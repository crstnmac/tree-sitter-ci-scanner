use anyhow::Context;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::Deserialize;

const GITHUB_API_BASE: &str = "https://api.github.com";

#[derive(Debug, Deserialize)]
struct ContentsResponse {
    content: String,   // base64-encoded
    encoding: String,
}

/// Fetch the raw YAML content of a file from a GitHub repository,
/// using ETag-based caching to avoid unnecessary downloads.
///
/// - `repo`     — `"owner/repo"` format
/// - `path`     — path within the repo (e.g. `"rules.yaml"`)
/// - `git_ref`  — branch, tag, or SHA
/// - `pat`      — optional GitHub PAT (falls back to unauthenticated)
/// - `cached_etag` — previous ETag from DB; returns `None` if not modified
///
/// Returns `Ok(Some((yaml_content, new_etag)))` on success,
/// `Ok(None)` when the server responded 304 (cached copy still valid).
pub async fn fetch_rules_yaml(
    http: &reqwest::Client,
    repo: &str,
    path: &str,
    git_ref: &str,
    pat: Option<&str>,
    cached_etag: Option<&str>,
) -> anyhow::Result<Option<(String, String)>> {
    let url = format!(
        "{}/repos/{}/contents/{}?ref={}",
        GITHUB_API_BASE, repo, path, git_ref,
    );

    let mut req = http
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");

    if let Some(token) = pat {
        req = req.bearer_auth(token);
    }
    if let Some(etag) = cached_etag {
        req = req.header("If-None-Match", etag);
    }

    let resp = req.send().await.context("GET GitHub contents")?;

    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(None); // cached copy is still valid
    }

    if !resp.status().is_success() {
        anyhow::bail!(
            "GitHub Contents API returned {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        );
    }

    let new_etag = resp
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let body: ContentsResponse = resp.json().await.context("parse contents JSON")?;

    anyhow::ensure!(
        body.encoding == "base64",
        "Unexpected encoding: {}",
        body.encoding
    );

    // GitHub wraps base64 in newlines — strip whitespace before decoding.
    let clean = body.content.replace(['\n', '\r', ' '], "");
    let bytes = B64.decode(clean).context("base64 decode rules YAML")?;
    let yaml = String::from_utf8(bytes).context("rules YAML is not valid UTF-8")?;

    Ok(Some((yaml, new_etag)))
}

/// Persist updated ETag and YAML cache back to the organizations table.
pub async fn update_cache(
    db: &sqlx::PgPool,
    org_id: uuid::Uuid,
    yaml: &str,
    etag: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE organizations SET rules_cache_yaml = $1, rules_cache_etag = $2, updated_at = NOW() WHERE id = $3",
    )
    .bind(yaml)
    .bind(etag)
    .bind(org_id)
    .execute(db)
    .await
    .context("update rules cache")?;
    Ok(())
}
