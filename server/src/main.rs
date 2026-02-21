use std::sync::Arc;

use axum::extract::FromRef;
use axum::routing::{delete, get, post};
use axum::Router;
use axum_extra::extract::cookie::Key;
use tower_http::{compression::CompressionLayer, cors::CorsLayer, trace::TraceLayer};

mod api;
mod auth;
mod config;
mod dashboard;
mod db;
mod github;
mod policy;

pub use config::Config;

// ── Application state ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub config: Arc<Config>,
    /// Cookie signing/encryption key derived from `COOKIE_SECRET`.
    pub cookie_key: Key,
    /// AES-256-GCM key for encrypting GitHub PATs at rest.
    pub encryption_key: [u8; 32],
    /// Shared HTTP client for GitHub API calls.
    pub http: reqwest::Client,
}

/// `PrivateCookieJar` requires `Key: FromRef<S>` in axum-extra 0.9.
impl FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.cookie_key.clone()
    }
}

// ── Router ────────────────────────────────────────────────────────────────────

fn build_router(state: AppState) -> Router {
    Router::new()
        // ── Auth ────────────────────────────────────────────────────────────
        .route(
            "/auth/login",
            get(auth::local::login_get).post(auth::local::login_post),
        )
        .route(
            "/auth/register",
            get(auth::local::register_get).post(auth::local::register_post),
        )
        .route("/auth/github", get(auth::github_oauth::authorize))
        .route("/auth/github/callback", get(auth::github_oauth::callback))
        .route("/auth/logout", post(auth::github_oauth::logout))
        // ── Dashboard ───────────────────────────────────────────────────────
        .route("/", get(dashboard::routes::index))
        .route("/findings", get(dashboard::routes::findings))
        .route(
            "/settings",
            get(dashboard::routes::settings_get).post(dashboard::routes::settings_post),
        )
        .route(
            "/settings/keys",
            get(dashboard::routes::keys_list).post(dashboard::routes::keys_create),
        )
        .route("/settings/keys/:id", delete(dashboard::routes::keys_delete))
        .route("/docs/rules", get(dashboard::routes::docs_rules))
        // ── CI API ──────────────────────────────────────────────────────────
        .route("/api/v1/rules", get(api::rules::get_rules))
        .route("/api/v1/scan", post(api::scan::post_scan))
        // ── Middleware ──────────────────────────────────────────────────────
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .layer(CompressionLayer::new())
        .with_state(state)
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "server=info,tower_http=info".parse().unwrap()),
        )
        .init();

    let config = envy::from_env::<Config>().map_err(|e| {
        anyhow::anyhow!("Failed to load configuration from environment: {}", e)
    })?;

    let port = config.port;

    // Derive the cookie key from COOKIE_SECRET (must be valid hex, ≥32 bytes)
    let cookie_secret_bytes = hex::decode(&config.cookie_secret).map_err(|_| {
        anyhow::anyhow!("COOKIE_SECRET must be a hex-encoded byte string (≥64 hex chars)")
    })?;
    let cookie_key = Key::from(&cookie_secret_bytes);

    // Decode the 32-byte AES-256-GCM key from ENCRYPTION_KEY
    let enc_key_bytes = hex::decode(&config.encryption_key).map_err(|_| {
        anyhow::anyhow!("ENCRYPTION_KEY must be a 64-char hex string (32 bytes)")
    })?;
    anyhow::ensure!(
        enc_key_bytes.len() == 32,
        "ENCRYPTION_KEY must decode to exactly 32 bytes, got {}",
        enc_key_bytes.len()
    );
    let mut encryption_key = [0u8; 32];
    encryption_key.copy_from_slice(&enc_key_bytes);

    // Connect to Postgres and run embedded migrations
    tracing::info!("Connecting to database…");
    let db = db::create_pool(&config.database_url).await?;
    tracing::info!("Migrations applied.");

    let http = reqwest::Client::builder()
        .user_agent(concat!("scanner-server/", env!("CARGO_PKG_VERSION")))
        .build()?;

    let state = AppState {
        db,
        config: Arc::new(config),
        cookie_key,
        encryption_key,
        http,
    };

    let app = build_router(state);

    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("Listening on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
