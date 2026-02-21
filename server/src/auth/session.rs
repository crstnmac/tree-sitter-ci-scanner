use async_trait::async_trait;
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Key, PrivateCookieJar};
use rand::RngCore;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::AppState;

pub const SESSION_COOKIE: &str = "session";

/// SHA-256 hash of `data`.
pub fn sha256(data: &[u8]) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().to_vec()
}

// ── AuthenticatedUser extractor ──────────────────────────────────────────────

/// Extractor for dashboard routes requiring a valid browser session.
/// Redirects to `/auth/github` when the session is missing or expired.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub id: Uuid,
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub org_id: Uuid,
    pub is_admin: bool,
    pub org_login: String,
}

#[derive(sqlx::FromRow)]
struct SessionUserRow {
    id: Uuid,
    login: String,
    name: Option<String>,
    avatar_url: Option<String>,
    org_id: Uuid,
    is_admin: bool,
    github_org_login: String,
}

#[async_trait]
impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = PrivateCookieJar::<Key>::from_request_parts(parts, state)
            .await
            .map_err(|e| e.into_response())?;

        let token = jar
            .get(SESSION_COOKIE)
            .map(|c| c.value().to_string())
            .ok_or_else(|| Redirect::to("/auth/login").into_response())?;

        let token_hash = sha256(token.as_bytes());

        let row: Option<SessionUserRow> = sqlx::query_as(
            r#"
            SELECT u.id, u.login, u.name, u.avatar_url, u.org_id, u.is_admin,
                   o.github_org_login
            FROM   sessions s
            JOIN   users u         ON u.id = s.user_id
            JOIN   organizations o ON o.id = u.org_id
            WHERE  s.token_hash = $1 AND s.expires_at > NOW()
            "#,
        )
        .bind(&token_hash)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

        let row = row.ok_or_else(|| Redirect::to("/auth/login").into_response())?;

        Ok(AuthenticatedUser {
            id: row.id,
            login: row.login,
            name: row.name,
            avatar_url: row.avatar_url,
            org_id: row.org_id,
            is_admin: row.is_admin,
            org_login: row.github_org_login,
        })
    }
}

// ── CiRunner extractor ───────────────────────────────────────────────────────

/// Extractor for CI API routes requiring a valid API key.
/// Returns 401 when the key is missing or unknown.
#[derive(Debug, Clone)]
pub struct CiRunner {
    pub org_id: Uuid,
    pub org_login: String,
    pub api_key_id: Uuid,
    pub rules_repo: String,
    pub rules_ref: String,
    pub policy_json: serde_json::Value,
    pub github_pat_encrypted: Option<Vec<u8>>,
    pub rules_cache_yaml: Option<String>,
    pub rules_cache_etag: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ApiKeyOrgRow {
    api_key_id: Uuid,
    org_id: Uuid,
    github_org_login: String,
    rules_repo: String,
    rules_ref: String,
    policy_json: serde_json::Value,
    github_pat_encrypted: Option<Vec<u8>>,
    rules_cache_yaml: Option<String>,
    rules_cache_etag: Option<String>,
}

#[async_trait]
impl FromRequestParts<AppState> for CiRunner {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                (StatusCode::UNAUTHORIZED, "Missing Authorization header").into_response()
            })?;

        let raw_key = auth.strip_prefix("Bearer ").ok_or_else(|| {
            (StatusCode::UNAUTHORIZED, "Authorization must be Bearer <key>").into_response()
        })?;

        let key_hash = sha256(raw_key.as_bytes());

        let row: Option<ApiKeyOrgRow> = sqlx::query_as(
            r#"
            SELECT k.id AS api_key_id,
                   o.id AS org_id,
                   o.github_org_login,
                   o.rules_repo,
                   o.rules_ref,
                   o.policy_json,
                   o.github_pat_encrypted,
                   o.rules_cache_yaml,
                   o.rules_cache_etag
            FROM   api_keys k
            JOIN   organizations o ON o.id = k.org_id
            WHERE  k.key_hash = $1
            "#,
        )
        .bind(&key_hash)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;

        let row = row.ok_or_else(|| {
            (StatusCode::UNAUTHORIZED, "Invalid API key").into_response()
        })?;

        // Fire-and-forget: update last_used_at
        let db = state.db.clone();
        let key_id = row.api_key_id;
        tokio::spawn(async move {
            let _ = sqlx::query("UPDATE api_keys SET last_used_at = NOW() WHERE id = $1")
                .bind(key_id)
                .execute(&db)
                .await;
        });

        Ok(CiRunner {
            org_id: row.org_id,
            org_login: row.github_org_login,
            api_key_id: row.api_key_id,
            rules_repo: row.rules_repo,
            rules_ref: row.rules_ref,
            policy_json: row.policy_json,
            github_pat_encrypted: row.github_pat_encrypted,
            rules_cache_yaml: row.rules_cache_yaml,
            rules_cache_etag: row.rules_cache_etag,
        })
    }
}

// ── Shared session creation ───────────────────────────────────────────────────

/// Create a new session for `user_id`, returning the raw cookie token.
///
/// Stores only `SHA-256(raw_token)` in the DB — the raw token is set as the
/// (encrypted) cookie value and never persisted.
pub async fn create_session(
    db: &sqlx::PgPool,
    user_id: Uuid,
    session_hours: i64,
) -> anyhow::Result<String> {
    let mut raw_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw_bytes);
    let raw_token = hex::encode(raw_bytes);
    let token_hash = sha256(raw_token.as_bytes());
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(session_hours);

    sqlx::query(
        "INSERT INTO sessions (user_id, token_hash, expires_at) VALUES ($1, $2, $3)",
    )
    .bind(user_id)
    .bind(&token_hash)
    .bind(expires_at)
    .execute(db)
    .await?;

    Ok(raw_token)
}
