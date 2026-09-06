use crate::ollama::ChatTurn;
use crate::types::{ChatDoneEvent, ChatErrorEvent, ChatMessage, ChatTokenEvent, DEFAULT_CHAT_ID};
use crate::AppState;
use crate::{db, ollama, rag, search};
use tauri::{AppHandle, Emitter, State};

/// What to answer with: either a prompt to send the chat model, or a fixed
/// string that skips the model entirely (see [`rag::decline_answer`] for why
/// the Decline path never trusts the model to produce it).
enum AnswerPlan {
    Generate(Vec<ChatTurn>),
    Fixed(String),
}

/// When a question names a specific covered product, retrieval is widened
/// to this many candidates before filtering to that product -- otherwise a
/// thin manual (e.g. Sony FS5, ~120 chunks) can get entirely crowded out of
/// a small top-k by a much larger one (e.g. DaVinci Resolve, ~4,700 chunks)
/// even when it has a good match.
const WIDE_RETRIEVAL_K: usize = 40;

#[tauri::command]
pub fn list_messages(state: State<AppState>) -> Result<Vec<ChatMessage>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::list_messages(&conn, DEFAULT_CHAT_ID).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_chat(state: State<AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::clear_messages(&conn, DEFAULT_CHAT_ID).map_err(|e| e.to_string())
}

/// Answers a question and streams the response via `chat://token`/`chat://done`/
/// `chat://error` events; the final persisted message is also returned so the
/// caller doesn't strictly need to listen for `chat://done`.
#[tauri::command]
pub async fn ask_question(app: AppHandle, state: State<'_, AppState>, text: String) -> Result<ChatMessage, String> {
    let question = text.trim().to_string();
    if question.is_empty() {
        return Err("Question cannot be empty".to_string());
    }

    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::insert_message(&conn, DEFAULT_CHAT_ID, "user", &question, false, &[])
            .map_err(|e| e.to_string())?;
    }

    let cfg = state.config.read().map_err(|e| e.to_string())?.clone();
    let http = state.http.clone();

    let emit_error = |err: &str| {
        let _ = app.emit(
            "chat://error",
            ChatErrorEvent { chat_id: DEFAULT_CHAT_ID, error: err.to_string() },
        );
    };

    // A question naming a tool we have no manual for (Premiere Pro, After
    // Effects, Blender, ...) skips manual retrieval entirely -- no chunk
    // from an unrelated manual gets a chance to confidently misanswer it.
    let (answer_plan, used_fallback, citations) = if rag::mentions_uncovered_product(&question) {
        if cfg.fallback_enabled {
            match search::search(&http, &cfg.searxng_url, &question, 5).await {
                Ok(results) => {
                    let citations = rag::web_citations(&results);
                    (AnswerPlan::Generate(rag::build_fallback_messages(&question, &results)), true, citations)
                }
                Err(err) => {
                    emit_error(&err);
                    return Err(err);
                }
            }
        } else {
            (AnswerPlan::Fixed(rag::decline_answer()), false, Vec::new())
        }
    } else {
        // Naming a product we DO have a manual for scopes retrieval to a much
        // wider candidate pool first, so that product's own chunks aren't
        // crowded out of a small top-k by a more heavily-represented manual
        // (e.g. a six-manual-strong DaVinci Resolve corpus vs. one Sony FS5
        // guide) before the product filter below ever runs.
        let mentioned_product = rag::mentioned_covered_product(&question);
        let retrieval_k = if mentioned_product.is_some() {
            cfg.top_k.max(WIDE_RETRIEVAL_K)
        } else {
            cfg.top_k
        };

        let query_embedding = match ollama::embed(&http, &cfg.embedding_model, &[question.clone()]).await {
            Ok(mut embeddings) if !embeddings.is_empty() => embeddings.remove(0),
            Ok(_) => {
                let err = "Ollama returned no embedding for the question".to_string();
                emit_error(&err);
                return Err(err);
            }
            Err(err) => {
                emit_error(&err);
                return Err(err);
            }
        };

        let hits = {
            let conn = state.db.lock().map_err(|e| e.to_string())?;
            db::top_k_chunks(&conn, &query_embedding, retrieval_k).map_err(|e| e.to_string())?
        };

        let path = rag::decide_path(
            &hits,
            mentioned_product,
            cfg.fallback_enabled,
            cfg.similarity_threshold_primary,
            cfg.similarity_threshold_secondary,
        );

        match path {
            rag::RagPath::Manual(chunks) => {
                // Retrieval may have been widened above; cap what actually
                // goes into the prompt/citations to the configured top_k.
                let chunks: Vec<_> = chunks.into_iter().take(cfg.top_k).collect();
                let citations = rag::manual_citations(&chunks);
                (AnswerPlan::Generate(rag::build_manual_messages(&question, &chunks)), false, citations)
            }
            rag::RagPath::Fallback => match search::search(&http, &cfg.searxng_url, &question, 5).await {
                Ok(results) => {
                    let citations = rag::web_citations(&results);
                    (AnswerPlan::Generate(rag::build_fallback_messages(&question, &results)), true, citations)
                }
                Err(err) => {
                    emit_error(&err);
                    return Err(err);
                }
            },
            rag::RagPath::Decline => (AnswerPlan::Fixed(rag::decline_answer()), false, Vec::new()),
        }
    };

    let answer = match answer_plan {
        AnswerPlan::Fixed(text) => {
            // Still emit a token event so the streaming UI has something to
            // show instead of sitting on "..." until chat://done.
            let _ = app.emit(
                "chat://token",
                ChatTokenEvent { chat_id: DEFAULT_CHAT_ID, delta: text.clone() },
            );
            text
        }
        AnswerPlan::Generate(messages) => {
            let app_for_stream = app.clone();
            let chat_result = ollama::chat_stream(&http, &cfg.chat_model, &messages, move |delta| {
                let _ = app_for_stream.emit(
                    "chat://token",
                    ChatTokenEvent { chat_id: DEFAULT_CHAT_ID, delta: delta.to_string() },
                );
            })
            .await;

            match chat_result {
                Ok(text) => text,
                Err(err) => {
                    emit_error(&err);
                    return Err(err);
                }
            }
        }
    };

    let saved = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::insert_message(&conn, DEFAULT_CHAT_ID, "assistant", &answer, used_fallback, &citations)
            .map_err(|e| e.to_string())?
    };

    let _ = app.emit("chat://done", ChatDoneEvent { message: saved.clone() });
    Ok(saved)
}
