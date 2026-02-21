use anyhow::Context;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::cookie::{Cookie, Key, PrivateCookieJar, SameSite};
use chrono::Utc;
use rand::RngCore;
use serde::Deserialize;

use crate::{
    auth::session::{sha256, SESSION_COOKIE},
    AppState,
};

const OAUTH_STATE_COOKIE: &str = "oauth_state";
const GITHUB_AUTH_URL: &str = "https://github.com/login/oauth/authorize";
const GITHUB_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_API_BASE: &str = "https://api.github.com";

// ── GitHub API response types ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GitHubUser {
    pub id: i64,
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubOrg {
    pub login: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// `GET /auth/github` — redirect user to GitHub OAuth consent screen.
pub async fn authorize(
    State(state): State<AppState>,
    jar: PrivateCookieJar<Key>,
) -> impl IntoResponse {
    let mut rng_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut rng_bytes);
    let oauth_state = hex::encode(rng_bytes);

    let url = format!(
        "{}?client_id={}&redirect_uri={}&scope=read:org,read:user&state={}",
        GITHUB_AUTH_URL,
        state.config.github_client_id,
        urlencoding::encode(&state.config.github_callback_url),
        oauth_state,
    );

    let state_cookie = Cookie::build((OAUTH_STATE_COOKIE, oauth_state))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::minutes(10))
        .build();

    (jar.add(state_cookie), Redirect::to(&url))
}

#[derive(Debug, Deserialize)]
pub struct CallbackParams {
    code: String,
    state: String,
}

/// `GET /auth/github/callback` — exchange code, upsert user, create session.
pub async fn callback(
    State(state): State<AppState>,
    Query(params): Query<CallbackParams>,
    jar: PrivateCookieJar<Key>,
) -> Response {
    // Verify CSRF state
    let stored_state = match jar.get(OAUTH_STATE_COOKIE) {
        Some(c) => c.value().to_string(),
        None => {
            return (StatusCode::BAD_REQUEST, "Missing oauth state cookie").into_response();
        }
    };
    if stored_state != params.state {
        return (StatusCode::BAD_REQUEST, "OAuth state mismatch").into_response();
    }

    match handle_callback_inner(&state, &params.code).await {
        Ok(session_token) => {
            let session_cookie = Cookie::build((SESSION_COOKIE, session_token))
                .http_only(true)
                .same_site(SameSite::Lax)
                .path("/")
                .max_age(time::Duration::hours(state.config.session_hours))
                .build();

            let jar = jar
                .remove(Cookie::build(OAUTH_STATE_COOKIE).path("/").build())
                .add(session_cookie);

            (jar, Redirect::to("/")).into_response()
        }
        Err(e) => {
            tracing::error!("OAuth callback error: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Authentication failed").into_response()
        }
    }
}

/// Returns the raw session token on success.
async fn handle_callback_inner(state: &AppState, code: &str) -> anyhow::Result<String> {
    let access_token = exchange_code(state, code).await?;
    let gh_user = get_github_user(state, &access_token).await?;
    let gh_orgs = get_github_orgs(state, &access_token).await?;

    let org_login = gh_orgs
        .into_iter()
        .next()
        .map(|o| o.login)
        .unwrap_or_else(|| gh_user.login.clone());

    // Upsert organization
    let org: crate::db::models::Organization = sqlx::query_as(
        r#"
        INSERT INTO organizations (github_org_login)
        VALUES ($1)
        ON CONFLICT (github_org_login) DO UPDATE
            SET updated_at = NOW()
        RETURNING *
        "#,
    )
    .bind(&org_login)
    .fetch_one(&state.db)
    .await
    .context("upsert organization")?;

    // First user in the org becomes admin
    let existing_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE org_id = $1")
            .bind(org.id)
            .fetch_one(&state.db)
            .await
            .unwrap_or(0);

    let is_admin = existing_count == 0;

    sqlx::query_as::<_, crate::db::models::User>(
        r#"
        INSERT INTO users (github_id, login, name, avatar_url, org_id, is_admin)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (github_id) DO UPDATE
            SET login      = EXCLUDED.login,
                name       = EXCLUDED.name,
                avatar_url = EXCLUDED.avatar_url,
                updated_at = NOW()
        RETURNING *
        "#,
    )
    .bind(gh_user.id)
    .bind(&gh_user.login)
    .bind(&gh_user.name)
    .bind(&gh_user.avatar_url)
    .bind(org.id)
    .bind(is_admin)
    .fetch_one(&state.db)
    .await
    .context("upsert user")?;

    // Create session: store SHA-256 hash in DB, return raw token for cookie
    let mut raw_token_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut raw_token_bytes);
    let raw_token = hex::encode(raw_token_bytes);
    let token_hash = sha256(raw_token.as_bytes());

    let expires_at = Utc::now() + chrono::Duration::hours(state.config.session_hours);

    sqlx::query("INSERT INTO sessions (user_id, token_hash, expires_at) SELECT id, $1, $2 FROM users WHERE github_id = $3")
        .bind(&token_hash)
        .bind(expires_at)
        .bind(gh_user.id)
        .execute(&state.db)
        .await
        .context("insert session")?;

    Ok(raw_token)
}

/// `POST /auth/logout` — delete session and clear cookie.
pub async fn logout(
    State(state): State<AppState>,
    jar: PrivateCookieJar<Key>,
) -> impl IntoResponse {
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        let token_hash = sha256(cookie.value().as_bytes());
        let _ = sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
            .bind(&token_hash)
            .execute(&state.db)
            .await;
    }
    let jar = jar.remove(Cookie::build(SESSION_COOKIE).path("/").build());
    (jar, Redirect::to("/auth/github"))
}

// ── GitHub API helpers ────────────────────────────────────────────────────────

async fn exchange_code(state: &AppState, code: &str) -> anyhow::Result<String> {
    let resp = state
        .http
        .post(GITHUB_TOKEN_URL)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", state.config.github_client_id.as_str()),
            ("client_secret", state.config.github_client_secret.as_str()),
            ("code", code),
        ])
        .send()
        .await
        .context("POST token exchange")?
        .json::<TokenResponse>()
        .await
        .context("parse token response")?;

    Ok(resp.access_token)
}

pub async fn get_github_user(state: &AppState, token: &str) -> anyhow::Result<GitHubUser> {
    state
        .http
        .get(format!("{}/user", GITHUB_API_BASE))
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .context("GET /user")?
        .json::<GitHubUser>()
        .await
        .context("parse user")
}

async fn get_github_orgs(state: &AppState, token: &str) -> anyhow::Result<Vec<GitHubOrg>> {
    state
        .http
        .get(format!("{}/user/orgs", GITHUB_API_BASE))
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .context("GET /user/orgs")?
        .json::<Vec<GitHubOrg>>()
        .await
        .context("parse orgs")
}
