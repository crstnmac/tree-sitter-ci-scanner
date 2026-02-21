use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct Organization {
    pub id: Uuid,
    pub github_org_login: String,
    pub rules_repo: String,
    pub rules_ref: String,
    pub policy_json: serde_json::Value,
    pub github_pat_encrypted: Option<Vec<u8>>,
    pub rules_cache_yaml: Option<String>,
    pub rules_cache_etag: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: Uuid,
    /// NULL for local (username/password) accounts.
    pub github_id: Option<i64>,
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub org_id: Uuid,
    pub is_admin: bool,
    /// Argon2id hash; NULL for OAuth-only accounts.
    pub password_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: Vec<u8>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct ApiKey {
    pub id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub created_by: Option<Uuid>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Repository {
    pub id: Uuid,
    pub org_id: Uuid,
    pub github_repo: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct Scan {
    pub id: Uuid,
    pub repo_id: Uuid,
    pub commit_sha: String,
    pub branch: Option<String>,
    pub sarif_json: serde_json::Value,
    pub passed: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Finding {
    pub id: Uuid,
    pub scan_id: Uuid,
    pub rule_id: String,
    pub severity: String,
    pub file_path: String,
    pub line_number: Option<i32>,
    pub message: String,
    pub created_at: DateTime<Utc>,
}

// ── View-model structs used in templates ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoRow {
    pub repo: String,
    pub last_scan_at: Option<DateTime<Utc>>,
    pub passed: Option<bool>,
    pub findings_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct FindingRow {
    pub id: Uuid,
    pub severity: String,
    pub rule_id: String,
    pub file_path: String,
    pub line_number: Option<i32>,
    pub message: String,
    pub repo: String,
    pub scan_at: DateTime<Utc>,
}
