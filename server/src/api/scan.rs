use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    api::rules::decrypt_pat,
    auth::session::CiRunner,
    github::status::{post_commit_status, CommitState},
    policy,
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct ScanRequest {
    /// Full SARIF 2.1.0 log produced by the scanner.
    pub sarif: scanner::output::SarifLog,
    /// `"owner/repo"` of the repository being scanned.
    pub repo: String,
    pub commit_sha: String,
    pub branch: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ScanResponse {
    pub scan_id: Uuid,
    pub passed: bool,
    pub findings_count: usize,
    pub status: String,
}

/// `POST /api/v1/scan`
///
/// Ingest a SARIF result, evaluate org policy, persist findings,
/// and optionally post a GitHub commit status.
pub async fn post_scan(
    State(state): State<AppState>,
    runner: CiRunner,
    Json(req): Json<ScanRequest>,
) -> Response {
    match handle_scan(&state, runner, req).await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(e) => {
            tracing::error!("Scan ingestion error: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

async fn handle_scan(
    state: &AppState,
    runner: CiRunner,
    req: ScanRequest,
) -> anyhow::Result<ScanResponse> {
    // 1. Evaluate policy
    let org_policy = policy::OrgPolicy::from_json(&runner.policy_json);
    let result = policy::evaluate(&org_policy, &req.sarif);

    // 2. Upsert repository
    let repo: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO repositories (org_id, github_repo)
        VALUES ($1, $2)
        ON CONFLICT (org_id, github_repo) DO UPDATE SET github_repo = EXCLUDED.github_repo
        RETURNING id
        "#,
    )
    .bind(runner.org_id)
    .bind(&req.repo)
    .fetch_one(&state.db)
    .await?;
    let repo_id = repo.0;

    // 3. Insert scan
    let sarif_value = serde_json::to_value(&req.sarif)?;
    let scan: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO scans (repo_id, commit_sha, branch, sarif_json, passed)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
    )
    .bind(repo_id)
    .bind(&req.commit_sha)
    .bind(&req.branch)
    .bind(sarif_value)
    .bind(result.passed)
    .fetch_one(&state.db)
    .await?;
    let scan_id = scan.0;

    // 4. Bulk-insert findings via UNNEST
    let findings_count = result.findings.len();
    if !result.findings.is_empty() {
        let rule_ids: Vec<String> = result.findings.iter().map(|f| f.rule_id.clone()).collect();
        let severities: Vec<String> = result.findings.iter().map(|f| f.severity.clone()).collect();
        let file_paths: Vec<String> = result.findings.iter().map(|f| f.file_path.clone()).collect();
        let line_numbers: Vec<Option<i32>> = result
            .findings
            .iter()
            .map(|f| f.line_number.map(|n| n as i32))
            .collect();
        let messages: Vec<String> = result.findings.iter().map(|f| f.message.clone()).collect();
        let scan_ids: Vec<Uuid> = vec![scan_id; findings_count];

        sqlx::query(
            r#"
            INSERT INTO findings (scan_id, rule_id, severity, file_path, line_number, message)
            SELECT unnest($1::uuid[]),
                   unnest($2::text[]),
                   unnest($3::text[]),
                   unnest($4::text[]),
                   unnest($5::int4[]),
                   unnest($6::text[])
            "#,
        )
        .bind(&scan_ids)
        .bind(&rule_ids)
        .bind(&severities)
        .bind(&file_paths)
        .bind(&line_numbers)
        .bind(&messages)
        .execute(&state.db)
        .await?;
    }

    // 5. Post GitHub commit status (best effort)
    if let Some(pat) = decrypt_pat(state, runner.github_pat_encrypted.as_deref()) {
        let commit_state = if result.passed {
            CommitState::Success
        } else {
            CommitState::Failure
        };
        let description = if result.passed {
            format!("Scan passed — {} finding(s)", findings_count)
        } else {
            format!("Scan failed — {} finding(s)", findings_count)
        };

        let http = state.http.clone();
        let repo = req.repo.clone();
        let sha = req.commit_sha.clone();
        tokio::spawn(async move {
            if let Err(e) = post_commit_status(
                &http,
                &repo,
                &sha,
                commit_state,
                &description,
                "scanner/policy",
                &pat,
            )
            .await
            {
                tracing::warn!("Failed to post commit status: {:?}", e);
            }
        });
    }

    let status = if result.passed { "passed" } else { "failed" }.to_string();
    Ok(ScanResponse {
        scan_id,
        passed: result.passed,
        findings_count,
        status,
    })
}
