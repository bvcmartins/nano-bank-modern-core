use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;
use thiserror::Error;

/// Errors surfaced by the ledger API, mapped to HTTP status codes.
#[derive(Error, Debug)]
pub enum AppError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),

    #[error("{0}")]
    BadRequest(String),

    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    Conflict(String),

    /// A business-rule rejection (does not balance, residual not zero, period closed).
    #[error("{0}")]
    Unprocessable(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            AppError::Database(e) => {
                tracing::error!(error = %e, "database error");
                (StatusCode::INTERNAL_SERVER_ERROR, "DATABASE_ERROR")
            }
            AppError::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            AppError::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            AppError::Conflict(_) => (StatusCode::CONFLICT, "CONFLICT"),
            AppError::Unprocessable(_) => (StatusCode::UNPROCESSABLE_ENTITY, "UNPROCESSABLE"),
        };
        let body = json!({ "error": { "code": code, "message": self.to_string() } });
        (status, axum::Json(body)).into_response()
    }
}
