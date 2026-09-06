use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const BASE_URL: &str = "http://127.0.0.1:11434";

#[derive(Debug, Clone, Deserialize)]
pub struct TagModel {
    pub name: String,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagModel>,
}

#[derive(Debug, Deserialize)]
struct PsResponse {
    #[serde(default)]
    models: Vec<TagModel>,
}

#[derive(Debug, Serialize)]
struct EmbedRequest<'a> {
    model: &'a str,
    input: &'a [String],
}

#[derive(Debug, Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [ChatTurn],
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ChatStreamChunk {
    #[serde(default)]
    message: Option<ChatStreamMessage>,
    #[serde(default)]
    done: bool,
}

#[derive(Debug, Deserialize)]
struct ChatStreamMessage {
    #[serde(default)]
    content: String,
}

pub async fn is_reachable(client: &reqwest::Client) -> bool {
    client
        .get(format!("{BASE_URL}/api/tags"))
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

pub async fn list_tags(client: &reqwest::Client) -> Result<Vec<String>, String> {
    let resp: TagsResponse = client
        .get(format!("{BASE_URL}/api/tags"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.models.into_iter().map(|m| m.name).collect())
}

pub async fn list_running(client: &reqwest::Client) -> Result<Vec<String>, String> {
    let resp: PsResponse = client
        .get(format!("{BASE_URL}/api/ps"))
        .timeout(Duration::from_secs(3))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    Ok(resp.models.into_iter().map(|m| m.name).collect())
}

pub async fn embed(
    client: &reqwest::Client,
    model: &str,
    inputs: &[String],
) -> Result<Vec<Vec<f32>>, String> {
    let body = EmbedRequest { model, input: inputs };
    let resp: EmbedResponse = client
        .post(format!("{BASE_URL}/api/embed"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Ollama embedding request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Ollama embedding request failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Ollama embedding response was not valid JSON: {e}"))?;
    Ok(resp.embeddings)
}

/// Streams a chat completion, invoking `on_delta` for every incoming token
/// chunk, and returns the fully assembled response text once the model
/// reports `done`.
pub async fn chat_stream<F: FnMut(&str)>(
    client: &reqwest::Client,
    model: &str,
    messages: &[ChatTurn],
    mut on_delta: F,
) -> Result<String, String> {
    let body = ChatRequest { model, messages, stream: true };
    let response = client
        .post(format!("{BASE_URL}/api/chat"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Ollama chat request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("Ollama chat request failed: {e}"))?;

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut full_text = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Ollama stream error: {e}"))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer.drain(..=newline_pos);
            if line.is_empty() {
                continue;
            }
            let parsed: ChatStreamChunk = serde_json::from_str(&line)
                .map_err(|e| format!("Ollama stream returned malformed JSON: {e}"))?;
            if let Some(msg) = parsed.message {
                if !msg.content.is_empty() {
                    on_delta(&msg.content);
                    full_text.push_str(&msg.content);
                }
            }
            if parsed.done {
                return Ok(full_text);
            }
        }
    }

    Ok(full_text)
}
