use crate::ollama::ChatTurn;
use crate::search::SearchResult;
use crate::types::{Citation, RetrievedChunk};

mod products;

const PERSONALITY: &str = r#"You are Dum-E, an eager, endearing robot assistant modeled after Tony Stark's
first creation. You love helping video producers master their gear and
editing software, even if you get a little overexcited about it.

Personality:
- Warm, encouraging, a bit goofy, but always competent.
- You teach, not just answer -- briefly explain the "why" behind a step when useful.
- You are honest about your limits. If you're not sure, you say so plainly.

Rules:
1. ALWAYS prefer the MANUAL EXCERPTS below. If they answer the question, cite
   the manual by name and page(s), e.g. "(Blackmagic PYXIS 6K Manual, p. 142)".
2. If the excerpts are empty or don't address the question, say so honestly
   ("Hmm, my manuals don't cover that one!"). If WEB SEARCH RESULTS are
   provided instead, clearly say you're switching to a live web search, then
   answer using those results and cite titles/URLs.
3. Never invent a page number, manual name, or URL you weren't given.
4. Use numbered steps for procedures; keep language plain, avoid unnecessary jargon.
5. If nothing is available (no manual match, no web fallback), say you don't
   know rather than guessing.

Format: lead with the direct answer, add steps/detail as needed, end with a
"Sources:" line listing every citation used."#;

/// Which knowledge source should answer a question, decided from retrieval
/// confidence and whether the user has opted into the web fallback.
pub enum RagPath {
    /// Top hit and at least one more hit cleared the similarity thresholds.
    Manual(Vec<RetrievedChunk>),
    /// Manuals were weak/empty, but the user enabled the Brave fallback and
    /// a key is stored.
    Fallback,
    /// Manuals were weak/empty and no fallback is available -- Dum-E must
    /// say so honestly rather than guess.
    Decline,
}

/// True if the question names a tool we have no manual for at all (Premiere
/// Pro, After Effects, Blender, ...). Callers should skip manual retrieval
/// entirely in this case -- never let a similar-sounding chunk from an
/// unrelated manual (e.g. DaVinci Resolve's keyer, when asked about Premiere
/// Pro's) confidently answer in its place.
pub fn mentions_uncovered_product(question: &str) -> bool {
    products::mentions_uncovered_product(question)
}

/// The `manuals.product` slug the question names, if it names one we do
/// have a manual for (e.g. "sony-fx3"). Retrieval should be scoped to just
/// that product so e.g. a Sony FS5 chunk can't answer an FX3 question just
/// because the concepts overlap.
pub fn mentioned_covered_product(question: &str) -> Option<&'static str> {
    products::mentioned_covered_product(question)
}

/// Decides whether retrieved chunks are confident enough to answer from, or
/// whether Dum-E should fall back to a web search / decline. `mentioned_product`
/// (from [`mentioned_covered_product`]) restricts candidates to that product's
/// own chunks first -- callers should already have widened the retrieval `k`
/// when a product was named, so that product's chunks aren't crowded out of
/// the top-k by a more heavily-represented manual before this filter runs.
pub fn decide_path(
    hits: &[RetrievedChunk],
    mentioned_product: Option<&str>,
    fallback_available: bool,
    primary_threshold: f32,
    secondary_threshold: f32,
) -> RagPath {
    let candidates: Vec<RetrievedChunk> = match mentioned_product {
        Some(product) => hits.iter().filter(|h| h.product == product).cloned().collect(),
        None => hits.to_vec(),
    };

    let top_hit_ok = candidates.first().is_some_and(|h| h.similarity >= primary_threshold);
    let secondary_hit_count = candidates.iter().filter(|h| h.similarity >= secondary_threshold).count();

    if top_hit_ok && secondary_hit_count >= 2 {
        RagPath::Manual(candidates)
    } else if fallback_available {
        RagPath::Fallback
    } else {
        RagPath::Decline
    }
}

fn user_turn(question: &str) -> ChatTurn {
    ChatTurn {
        role: "user".to_string(),
        content: question.to_string(),
    }
}

pub fn build_manual_messages(question: &str, chunks: &[RetrievedChunk]) -> Vec<ChatTurn> {
    let excerpts = chunks
        .iter()
        .map(|c| {
            format!(
                "[{} , p. {}-{}]\n{}",
                c.manual_title, c.page_start, c.page_end, c.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    let system = format!("{PERSONALITY}\n\nMANUAL EXCERPTS:\n{excerpts}");
    vec![
        ChatTurn { role: "system".to_string(), content: system },
        user_turn(question),
    ]
}

pub fn build_fallback_messages(question: &str, results: &[SearchResult]) -> Vec<ChatTurn> {
    let formatted = results
        .iter()
        .map(|r| format!("[{}]({})\n{}", r.title, r.url, r.snippet))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    let system = format!("{PERSONALITY}\n\nWEB SEARCH RESULTS:\n{formatted}");
    vec![
        ChatTurn { role: "system".to_string(), content: system },
        user_turn(question),
    ]
}

/// Fixed, honest answer for the Decline path -- deliberately never routed
/// through the chat model. Sending "MANUAL EXCERPTS: (none found)" plus an
/// instruction to say so was tried first, but in testing `command-r7b`
/// answered confidently from its own training data anyway (e.g. a full
/// Blender keyframing walkthrough) instead of declining, even though the
/// system prompt explicitly told it not to guess. An honest "I don't know"
/// can't depend on the model choosing to follow that instruction.
pub fn decline_answer() -> String {
    "Aw, my manuals don't cover that one, and internet search fallback is turned off right \
now, so I don't want to just guess and risk steering you wrong. You can turn on Internet \
search fallback in Settings if you'd like me to check the web instead!"
        .to_string()
}

pub fn manual_citations(chunks: &[RetrievedChunk]) -> Vec<Citation> {
    let mut seen = std::collections::HashSet::new();
    chunks
        .iter()
        .filter(|c| seen.insert((c.manual_title.clone(), c.page_start, c.page_end)))
        .map(|c| Citation::Manual {
            title: c.manual_title.clone(),
            page_start: c.page_start,
            page_end: c.page_end,
        })
        .collect()
}

pub fn web_citations(results: &[SearchResult]) -> Vec<Citation> {
    results
        .iter()
        .map(|r| Citation::Web { title: r.title.clone(), url: r.url.clone() })
        .collect()
}
