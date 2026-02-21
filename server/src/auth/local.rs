use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use askama::Template;
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
    Form,
};
use axum_extra::extract::cookie::{Cookie, Key, PrivateCookieJar, SameSite};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth::session::{create_session, SESSION_COOKIE},
    AppState,
};

// ── Templates ─────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub error: Option<String>,
}

#[derive(Template)]
#[template(path = "register.html")]
pub struct RegisterTemplate {
    pub error: Option<String>,
}

// ── Query params ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AuthQuery {
    error: Option<String>,
}

// ── GET handlers ──────────────────────────────────────────────────────────────

/// `GET /auth/login`
pub async fn login_get(Query(q): Query<AuthQuery>) -> impl IntoResponse {
    LoginTemplate {
        error: q.error.map(|e| error_message_login(&e)),
    }
}

/// `GET /auth/register`
pub async fn register_get(Query(q): Query<AuthQuery>) -> impl IntoResponse {
    RegisterTemplate {
        error: q.error.map(|e| error_message_register(&e)),
    }
}

// ── POST handlers ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
}

/// `POST /auth/login`
pub async fn login_post(
    State(state): State<AppState>,
    jar: PrivateCookieJar<Key>,
    Form(form): Form<LoginForm>,
) -> Response {
    match try_login(&state, &form.username, &form.password).await {
        Ok(user_id) => {
            match create_session(&state.db, user_id, state.config.session_hours).await {
                Ok(raw_token) => finish_login(jar, raw_token, state.config.session_hours),
                Err(e) => {
                    tracing::error!("Session creation failed: {:?}", e);
                    Redirect::to("/auth/login?error=server").into_response()
                }
            }
        }
        Err(e) => {
            tracing::debug!("Login failed for '{}': {}", form.username, e);
            Redirect::to("/auth/login?error=invalid").into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RegisterForm {
    username: String,
    password: String,
}

/// `POST /auth/register`
pub async fn register_post(
    State(state): State<AppState>,
    jar: PrivateCookieJar<Key>,
    Form(form): Form<RegisterForm>,
) -> Response {
    // ── Validate inputs ──────────────────────────────────────────────────────
    let username = form.username.trim().to_string();

    if username.len() < 3 || username.len() > 32 {
        return Redirect::to("/auth/register?error=username_length").into_response();
    }
    if !username
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Redirect::to("/auth/register?error=username_invalid").into_response();
    }
    if form.password.len() < 8 {
        return Redirect::to("/auth/register?error=password_short").into_response();
    }

    match try_register(&state, &username, &form.password).await {
        Ok(user_id) => {
            match create_session(&state.db, user_id, state.config.session_hours).await {
                Ok(raw_token) => finish_login(jar, raw_token, state.config.session_hours),
                Err(e) => {
                    tracing::error!("Session creation failed: {:?}", e);
                    Redirect::to("/auth/register?error=server").into_response()
                }
            }
        }
        Err(e) => {
            tracing::debug!("Registration failed for '{}': {}", username, e);
            let code = if e.to_string().contains("taken") {
                "taken"
            } else {
                "server"
            };
            Redirect::to(&format!("/auth/register?error={}", code)).into_response()
        }
    }
}

// ── Core logic ────────────────────────────────────────────────────────────────

/// Verify username + password and return the user's UUID on success.
async fn try_login(state: &AppState, username: &str, password: &str) -> anyhow::Result<Uuid> {
    let row: Option<(Uuid, Option<String>)> = sqlx::query_as(
        "SELECT id, password_hash FROM users WHERE login = $1 AND password_hash IS NOT NULL",
    )
    .bind(username)
    .fetch_optional(&state.db)
    .await?;

    let (user_id, stored_hash) =
        row.ok_or_else(|| anyhow::anyhow!("user not found"))?;
    let stored_hash = stored_hash.ok_or_else(|| anyhow::anyhow!("no password set"))?;

    let parsed = PasswordHash::new(&stored_hash)
        .map_err(|e| anyhow::anyhow!("invalid stored hash: {}", e))?;

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| anyhow::anyhow!("wrong password"))?;

    Ok(user_id)
}

/// Create a new local account and return the new user's UUID.
async fn try_register(state: &AppState, username: &str, password: &str) -> anyhow::Result<Uuid> {
    // Check for duplicate username
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE login = $1)")
        .bind(username)
        .fetch_one(&state.db)
        .await?;

    if exists {
        anyhow::bail!("username taken");
    }

    // Hash password with Argon2id
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("hash failed: {}", e))?
        .to_string();

    // Upsert org — personal org named after the user
    let org: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO organizations (github_org_login)
        VALUES ($1)
        ON CONFLICT (github_org_login) DO UPDATE SET updated_at = NOW()
        RETURNING id
        "#,
    )
    .bind(username)
    .fetch_one(&state.db)
    .await?;
    let org_id = org.0;

    // First user in this org becomes admin
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE org_id = $1")
        .bind(org_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

    let user: (Uuid,) = sqlx::query_as(
        r#"
        INSERT INTO users (login, org_id, is_admin, password_hash)
        VALUES ($1, $2, $3, $4)
        RETURNING id
        "#,
    )
    .bind(username)
    .bind(org_id)
    .bind(count == 0)
    .bind(&hash)
    .fetch_one(&state.db)
    .await?;

    Ok(user.0)
}

// ── Helper ────────────────────────────────────────────────────────────────────

fn finish_login(jar: PrivateCookieJar<Key>, raw_token: String, session_hours: i64) -> Response {
    let cookie = Cookie::build((SESSION_COOKIE, raw_token))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::hours(session_hours))
        .build();
    (jar.add(cookie), Redirect::to("/")).into_response()
}

fn error_message_login(code: &str) -> String {
    match code {
        "invalid" => "Invalid username or password.".to_string(),
        "server" => "Server error — please try again.".to_string(),
        other => other.to_string(),
    }
}

fn error_message_register(code: &str) -> String {
    match code {
        "taken" => "That username is already taken.".to_string(),
        "username_length" => "Username must be 3–32 characters.".to_string(),
        "username_invalid" => "Username may only contain letters, numbers, hyphens and underscores.".to_string(),
        "password_short" => "Password must be at least 8 characters.".to_string(),
        "server" => "Server error — please try again.".to_string(),
        other => other.to_string(),
    }
}
