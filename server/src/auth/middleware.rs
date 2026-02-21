use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::auth::session::AuthenticatedUser;

/// Guard that rejects non-admin users with 403.
pub fn require_admin(user: &AuthenticatedUser) -> Result<(), Response> {
    if user.is_admin {
        Ok(())
    } else {
        Err((StatusCode::FORBIDDEN, "Admin access required").into_response())
    }
}
