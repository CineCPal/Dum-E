use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Debug, Deserialize)]
struct SearxngResponse {
    #[serde(default)]
    results: Vec<SearxngResult>,
}

#[derive(Debug, Deserialize)]
struct SearxngResult {
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    content: String,
}

pub async fn is_reachable(client: &reqwest::Client, base_url: &str) -> bool {
    client
        .get(base_url)
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

/// Sends only `query` (the user's raw question text -- never filenames, file
/// paths, or manual content) to the self-hosted SearXNG instance at
/// `base_url`. Only called when the user has explicitly enabled the
/// internet fallback. SearXNG's JSON output must be enabled in its
/// `settings.yml` (`search.formats: [html, json]`) -- see `searxng/README.md`.
pub async fn search(
    client: &reqwest::Client,
    base_url: &str,
    query: &str,
    count: usize,
) -> Result<Vec<SearchResult>, String> {
    let url = format!("{}/search", base_url.trim_end_matches('/'));
    let resp: SearxngResponse = client
        .get(url)
        .query(&[("q", query), ("format", "json")])
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("SearXNG request failed (is it running at {base_url}?): {e}"))?
        .error_for_status()
        .map_err(|e| format!("SearXNG request failed: {e}"))?
        .json()
        .await
        .map_err(|e| {
            format!(
                "SearXNG response was not valid JSON -- is JSON output enabled in its settings.yml? {e}"
            )
        })?;

    Ok(resp
        .results
        .into_iter()
        .take(count)
        .map(|r| SearchResult { title: r.title, url: r.url, snippet: r.content })
        .collect())
}
