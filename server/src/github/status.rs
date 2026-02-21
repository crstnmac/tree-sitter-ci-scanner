use anyhow::Context;
use serde::Serialize;

const GITHUB_API_BASE: &str = "https://api.github.com";

/// GitHub commit status state.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CommitState {
    Success,
    Failure,
    Pending,
    Error,
}

/// Post a commit status to the GitHub Statuses API.
///
/// - `repo`       — `"owner/repo"` format
/// - `sha`        — full commit SHA
/// - `state`      — success / failure / error / pending
/// - `description` — human-readable summary (max 140 chars)
/// - `context`    — status check name shown in GitHub UI
/// - `pat`        — GitHub PAT with `repo:status` scope
pub async fn post_commit_status(
    http: &reqwest::Client,
    repo: &str,
    sha: &str,
    state: CommitState,
    description: &str,
    context: &str,
    pat: &str,
) -> anyhow::Result<()> {
    #[derive(Serialize)]
    struct Body<'a> {
        state: CommitState,
        description: &'a str,
        context: &'a str,
    }

    let url = format!("{}/repos/{}/statuses/{}", GITHUB_API_BASE, repo, sha);

    // GitHub limits description to 140 chars
    let description = if description.len() > 140 {
        &description[..140]
    } else {
        description
    };

    let resp = http
        .post(&url)
        .bearer_auth(pat)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .json(&Body {
            state,
            description,
            context,
        })
        .send()
        .await
        .context("POST commit status")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("GitHub Statuses API returned {}: {}", status, body);
    }

    Ok(())
}
