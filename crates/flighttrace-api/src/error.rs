//! API error type and `IntoResponse` mapping.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;
use tracing::error;

use flighttrace_assertions::AssertionError;
use flighttrace_storage::StorageError;

/// Errors that can be returned from API handlers.
#[derive(Debug, Error)]
pub enum ApiError {
    /// Requested entity does not exist.
    #[error("resource not found")]
    NotFound,

    /// Storage layer error.
    #[error(transparent)]
    Storage(#[from] StorageError),

    /// A request body could not be parsed.
    #[error("invalid JSON: {0}")]
    BadJson(#[from] serde_json::Error),

    /// A 400-class problem with the request: invalid YAML, missing mission
    /// file, missing reference, etc.
    #[error("{0}")]
    BadRequest(String),

    /// The assertion engine rejected the supplied mission spec.
    #[error("invalid mission spec: {0}")]
    Assertion(#[from] AssertionError),
}

impl ApiError {
    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            ApiError::Storage(StorageError::NotFound) => (StatusCode::NOT_FOUND, "not_found"),
            ApiError::Storage(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
            ApiError::BadJson(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            ApiError::Assertion(_) => (StatusCode::BAD_REQUEST, "bad_request"),
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorBody<'a> {
    error: ErrorPayload<'a>,
}

#[derive(Debug, Serialize)]
struct ErrorPayload<'a> {
    code: &'a str,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = self.status_and_code();
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            error!(error = %self, "request failed");
        }
        let body = ErrorBody {
            error: ErrorPayload {
                code,
                message: self.to_string(),
            },
        };
        (status, Json(body)).into_response()
    }
}

/// Convenience alias used in handlers.
pub type ApiResult<T> = Result<T, ApiError>;
