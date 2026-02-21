use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    Form,
};
use rand::RngCore;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::{middleware::require_admin, session::AuthenticatedUser},
    auth::session::sha256,
    db::models::{ApiKey, FindingRow, RepoRow},
    AppState,
};

// ── Template definitions ──────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    user: AuthenticatedUser,
    repos: Vec<RepoRow>,
}

#[derive(Template)]
#[template(path = "findings.html")]
struct FindingsTemplate {
    user: AuthenticatedUser,
    findings: Vec<FindingRow>,
    severity_filter: Option<String>,
}

#[derive(Template)]
#[template(path = "partials/findings_rows.html")]
struct FindingsRowsTemplate {
    findings: Vec<FindingRow>,
}

#[derive(Template)]
#[template(path = "settings.html")]
struct SettingsTemplate {
    user: AuthenticatedUser,
    org_login: String,
    rules_repo: String,
    rules_ref: String,
    policy_json: String,
    flash: Option<String>,
}

#[derive(Template)]
#[template(path = "keys.html")]
struct KeysTemplate {
    user: AuthenticatedUser,
    keys: Vec<ApiKey>,
    new_key: Option<String>,
}

// ── Route handlers ────────────────────────────────────────────────────────────

/// `GET /` — repo overview table.
pub async fn index(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Response {
    #[derive(sqlx::FromRow)]
    struct RepoStats {
        github_repo: String,
        last_scan_at: Option<chrono::DateTime<chrono::Utc>>,
        passed: Option<bool>,
        findings_count: Option<i64>,
    }

    let rows: Vec<RepoStats> = sqlx::query_as(
        r#"
        SELECT r.github_repo,
               MAX(s.created_at)  AS last_scan_at,
               (array_agg(s.passed ORDER BY s.created_at DESC))[1] AS passed,
               COALESCE(
                 (SELECT COUNT(*) FROM findings f
                  WHERE f.scan_id = (
                    SELECT id FROM scans WHERE repo_id = r.id ORDER BY created_at DESC LIMIT 1
                  )), 0
               ) AS findings_count
        FROM   repositories r
        LEFT JOIN scans s ON s.repo_id = r.id
        WHERE  r.org_id = $1
        GROUP BY r.id, r.github_repo
        ORDER BY last_scan_at DESC NULLS LAST
        "#,
    )
    .bind(user.org_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let repos = rows
        .into_iter()
        .map(|r| RepoRow {
            repo: r.github_repo,
            last_scan_at: r.last_scan_at,
            passed: r.passed,
            findings_count: r.findings_count.unwrap_or(0),
        })
        .collect();

    IndexTemplate { user, repos }.into_response()
}

#[derive(Debug, Deserialize)]
pub struct FindingsQuery {
    pub severity: Option<String>,
}

/// `GET /findings` — cross-repo findings table with optional severity filter.
/// Returns a partial (HTMX) when `HX-Request` header is present.
pub async fn findings(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    headers: HeaderMap,
    Query(params): Query<FindingsQuery>,
) -> Response {
    #[derive(sqlx::FromRow)]
    struct FindingDbRow {
        id: Uuid,
        severity: String,
        rule_id: String,
        file_path: String,
        line_number: Option<i32>,
        message: String,
        github_repo: String,
        scan_at: chrono::DateTime<chrono::Utc>,
    }

    let rows: Vec<FindingDbRow> = match &params.severity {
        Some(sev) => sqlx::query_as(
            r#"
            SELECT f.id, f.severity, f.rule_id, f.file_path, f.line_number, f.message,
                   r.github_repo, s.created_at AS scan_at
            FROM   findings f
            JOIN   scans s        ON s.id = f.scan_id
            JOIN   repositories r ON r.id = s.repo_id
            WHERE  r.org_id  = $1
              AND  f.severity = $2
            ORDER BY f.severity_order, f.id DESC
            LIMIT 500
            "#,
        )
        .bind(user.org_id)
        .bind(sev)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default(),

        None => sqlx::query_as(
            r#"
            SELECT f.id, f.severity, f.rule_id, f.file_path, f.line_number, f.message,
                   r.github_repo, s.created_at AS scan_at
            FROM   findings f
            JOIN   scans s        ON s.id = f.scan_id
            JOIN   repositories r ON r.id = s.repo_id
            WHERE  r.org_id = $1
            ORDER BY f.severity_order, f.id DESC
            LIMIT 500
            "#,
        )
        .bind(user.org_id)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default(),
    };

    let findings: Vec<FindingRow> = rows
        .into_iter()
        .map(|r| FindingRow {
            id: r.id,
            severity: r.severity,
            rule_id: r.rule_id,
            file_path: r.file_path,
            line_number: r.line_number,
            message: r.message,
            repo: r.github_repo,
            scan_at: r.scan_at,
        })
        .collect();

    let is_htmx = headers.contains_key("hx-request");
    if is_htmx {
        FindingsRowsTemplate { findings }.into_response()
    } else {
        FindingsTemplate {
            user,
            findings,
            severity_filter: params.severity,
        }
        .into_response()
    }
}

/// `GET /settings`
pub async fn settings_get(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Response {
    if let Err(r) = require_admin(&user) {
        return r;
    }

    match fetch_org_settings(&state, user.org_id).await {
        Ok((rules_repo, rules_ref, policy_json)) => SettingsTemplate {
            org_login: user.org_login.clone(),
            rules_repo,
            rules_ref,
            policy_json,
            user,
            flash: None,
        }
        .into_response(),
        Err(e) => {
            tracing::error!("settings_get DB error: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct SettingsForm {
    pub rules_repo: String,
    pub rules_ref: String,
    pub policy_json: String,
    #[serde(default)]
    pub github_pat: String,
}

/// `POST /settings`
pub async fn settings_post(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Form(form): Form<SettingsForm>,
) -> Response {
    if let Err(r) = require_admin(&user) {
        return r;
    }

    // Validate policy JSON
    if serde_json::from_str::<serde_json::Value>(&form.policy_json).is_err() {
        return (StatusCode::UNPROCESSABLE_ENTITY, "Invalid policy JSON").into_response();
    }
    let policy_value: serde_json::Value = serde_json::from_str(&form.policy_json).unwrap();

    // Optionally encrypt new PAT
    let pat_bytes: Option<Vec<u8>> = if form.github_pat.is_empty() {
        None
    } else {
        Some(encrypt_pat(&state.encryption_key, &form.github_pat))
    };

    let result = if let Some(enc_pat) = pat_bytes {
        sqlx::query(
            r#"
            UPDATE organizations
            SET rules_repo = $1, rules_ref = $2, policy_json = $3,
                github_pat_encrypted = $4, updated_at = NOW()
            WHERE id = $5
            "#,
        )
        .bind(&form.rules_repo)
        .bind(&form.rules_ref)
        .bind(policy_value)
        .bind(&enc_pat)
        .bind(user.org_id)
        .execute(&state.db)
        .await
    } else {
        sqlx::query(
            r#"
            UPDATE organizations
            SET rules_repo = $1, rules_ref = $2, policy_json = $3, updated_at = NOW()
            WHERE id = $4
            "#,
        )
        .bind(&form.rules_repo)
        .bind(&form.rules_ref)
        .bind(policy_value)
        .bind(user.org_id)
        .execute(&state.db)
        .await
    };

    if let Err(e) = result {
        tracing::error!("settings_post DB error: {:?}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to save settings").into_response();
    }

    Redirect::to("/settings").into_response()
}

/// `GET /settings/keys`
pub async fn keys_list(
    State(state): State<AppState>,
    user: AuthenticatedUser,
) -> Response {
    if let Err(r) = require_admin(&user) {
        return r;
    }

    let keys: Vec<ApiKey> = sqlx::query_as(
        "SELECT id, org_id, name, created_by, last_used_at, created_at FROM api_keys WHERE org_id = $1 ORDER BY created_at DESC",
    )
    .bind(user.org_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    KeysTemplate {
        user,
        keys,
        new_key: None,
    }
    .into_response()
}

#[derive(Debug, Deserialize)]
pub struct CreateKeyForm {
    pub name: String,
}

/// `POST /settings/keys` — generate a new API key; display it once.
pub async fn keys_create(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Form(form): Form<CreateKeyForm>,
) -> Response {
    if let Err(r) = require_admin(&user) {
        return r;
    }

    // Generate 32-byte random key and encode as hex (64 chars)
    let mut raw_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw_bytes);
    let raw_key = hex::encode(raw_bytes);
    let key_hash = sha256(raw_key.as_bytes());

    if let Err(e) = sqlx::query(
        "INSERT INTO api_keys (org_id, name, key_hash, created_by) VALUES ($1, $2, $3, $4)",
    )
    .bind(user.org_id)
    .bind(&form.name)
    .bind(&key_hash)
    .bind(user.id)
    .execute(&state.db)
    .await
    {
        tracing::error!("keys_create DB error: {:?}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create key").into_response();
    }

    let keys: Vec<ApiKey> = sqlx::query_as(
        "SELECT id, org_id, name, created_by, last_used_at, created_at FROM api_keys WHERE org_id = $1 ORDER BY created_at DESC",
    )
    .bind(user.org_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    KeysTemplate {
        user,
        keys,
        new_key: Some(raw_key),
    }
    .into_response()
}

/// `DELETE /settings/keys/:id`
pub async fn keys_delete(
    State(state): State<AppState>,
    user: AuthenticatedUser,
    Path(key_id): Path<Uuid>,
) -> Response {
    if let Err(r) = require_admin(&user) {
        return r;
    }

    // Ensure the key belongs to the user's org before deleting
    let _ = sqlx::query(
        "DELETE FROM api_keys WHERE id = $1 AND org_id = $2",
    )
    .bind(key_id)
    .bind(user.org_id)
    .execute(&state.db)
    .await;

    Redirect::to("/settings/keys").into_response()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn fetch_org_settings(
    state: &AppState,
    org_id: Uuid,
) -> anyhow::Result<(String, String, String)> {
    let row: (String, String, serde_json::Value) = sqlx::query_as(
        "SELECT rules_repo, rules_ref, policy_json FROM organizations WHERE id = $1",
    )
    .bind(org_id)
    .fetch_one(&state.db)
    .await?;

    let policy_str = serde_json::to_string_pretty(&row.2).unwrap_or_default();
    Ok((row.0, row.1, policy_str))
}

// ── Docs ──────────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "docs.html")]
struct DocsTemplate {
    user: AuthenticatedUser,
}

/// `GET /docs/rules`
pub async fn docs_rules(user: AuthenticatedUser) -> impl IntoResponse {
    DocsTemplate { user }
}

fn encrypt_pat(key: &[u8; 32], plaintext: &str) -> Vec<u8> {
    use aes_gcm::{aead::Aead, Aes256Gcm, Key, KeyInit, Nonce};

    let cipher_key = Key::<Aes256Gcm>::from_slice(key);
    let cipher = Aes256Gcm::new(cipher_key);

    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .expect("AES-GCM encryption failed");

    let mut result = nonce_bytes.to_vec();
    result.extend(ciphertext);
    result
}
