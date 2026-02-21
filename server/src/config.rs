/// Server configuration loaded from environment variables via `envy`.
///
/// All env vars are uppercase (e.g. `DATABASE_URL`, `PORT`).
#[derive(Debug, serde::Deserialize)]
pub struct Config {
    pub database_url: String,

    #[serde(default = "default_port")]
    pub port: u16,

    pub github_client_id: String,
    pub github_client_secret: String,
    pub github_callback_url: String,

    /// Public base URL of this server (e.g. `https://scanner.example.com`)
    pub base_url: String,

    /// 64+ bytes hex-encoded secret for signing/encrypting session cookies
    pub cookie_secret: String,

    /// 32 bytes hex-encoded key for AES-256-GCM (PAT encryption at rest)
    pub encryption_key: String,

    #[serde(default = "default_session_hours")]
    pub session_hours: i64,
}

fn default_port() -> u16 {
    3000
}

fn default_session_hours() -> i64 {
    24
}
