use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::{auth::session::CiRunner, github::rules_fetcher, AppState};

const DEFAULT_RULES_PATH: &str = "rules.yaml";

/// `GET /api/v1/rules`
///
/// Returns the merged rules YAML for the organisation.
/// Uses ETag caching so repeated calls don't hammer the GitHub API.
pub async fn get_rules(
    State(state): State<AppState>,
    runner: CiRunner,
) -> Response {
    if runner.rules_repo.is_empty() {
        return (
            StatusCode::NOT_FOUND,
            "No rules_repo configured for this organisation",
        )
            .into_response();
    }

    // Decrypt PAT if present
    let pat_opt = decrypt_pat(&state, runner.github_pat_encrypted.as_deref());

    match rules_fetcher::fetch_rules_yaml(
        &state.http,
        &runner.rules_repo,
        DEFAULT_RULES_PATH,
        &runner.rules_ref,
        pat_opt.as_deref(),
        runner.rules_cache_etag.as_deref(),
    )
    .await
    {
        Ok(Some((yaml, etag))) => {
            // Persist updated cache in background
            let db = state.db.clone();
            let org_id = runner.org_id;
            let yaml2 = yaml.clone();
            let etag2 = etag.clone();
            tokio::spawn(async move {
                if let Err(e) = rules_fetcher::update_cache(&db, org_id, &yaml2, &etag2).await {
                    tracing::warn!("Failed to update rules cache: {:?}", e);
                }
            });

            axum::response::Response::builder()
                .status(200)
                .header("Content-Type", "application/x-yaml")
                .header("ETag", etag)
                .body(axum::body::Body::from(yaml))
                .unwrap()
        }

        Ok(None) => {
            // 304 from GitHub — serve cached copy
            match runner.rules_cache_yaml {
                Some(yaml) => axum::response::Response::builder()
                    .status(200)
                    .header("Content-Type", "application/x-yaml")
                    .body(axum::body::Body::from(yaml))
                    .unwrap(),
                None => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Cache inconsistency: no stored rules YAML",
                )
                    .into_response(),
            }
        }

        Err(e) => {
            tracing::error!("Failed to fetch rules: {:?}", e);
            // Fall back to cache if available
            if let Some(yaml) = runner.rules_cache_yaml {
                axum::response::Response::builder()
                    .status(200)
                    .header("Content-Type", "application/x-yaml")
                    .body(axum::body::Body::from(yaml))
                    .unwrap()
            } else {
                (
                    StatusCode::BAD_GATEWAY,
                    format!("Failed to fetch rules from GitHub: {}", e),
                )
                    .into_response()
            }
        }
    }
}

/// Decrypt the stored PAT using AES-256-GCM.
/// Returns `None` if there's no stored PAT or decryption fails.
pub fn decrypt_pat(state: &AppState, encrypted: Option<&[u8]>) -> Option<String> {
    use aes_gcm::{aead::Aead, Aes256Gcm, Key, KeyInit, Nonce};

    let enc = encrypted?;
    if enc.len() < 12 {
        return None;
    }
    let (nonce_bytes, ciphertext) = enc.split_at(12);
    let key = Key::<Aes256Gcm>::from_slice(&state.encryption_key);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);
    let plaintext = cipher.decrypt(nonce, ciphertext).ok()?;
    String::from_utf8(plaintext).ok()
}
