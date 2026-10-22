// SPDX-License-Identifier: LicenseRef-Restricted
use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use pley_transcoder_api::ErrorBody;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Unauthorized,
    NotFound(String),
    /// The job does not exist (expired, deleted, transcoder restarted). Distinct
    /// from other 404s so clients only re-create jobs that are really gone.
    JobNotFound(String),
    /// Capacity exhausted (too many concurrent transcodes).
    Busy(String),
    Timeout(String),
    /// ffmpeg failed.
    Failed(String),
    Internal(anyhow::Error),
}

pub type ApiResult<T> = Result<T, ApiError>;

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::Internal(err)
    }
}

impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        if err.kind() == std::io::ErrorKind::NotFound {
            ApiError::NotFound(err.to_string())
        } else {
            ApiError::Internal(err.into())
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, "bad_request", m),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", "missing or invalid token".into()),
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, "not_found", m),
            ApiError::JobNotFound(m) => (StatusCode::NOT_FOUND, "job_not_found", m),
            ApiError::Busy(m) => (StatusCode::SERVICE_UNAVAILABLE, "busy", m),
            ApiError::Timeout(m) => (StatusCode::GATEWAY_TIMEOUT, "timeout", m),
            ApiError::Failed(m) => (StatusCode::INTERNAL_SERVER_ERROR, "ffmpeg_failed", m),
            ApiError::Internal(e) => {
                tracing::error!(error = ?e, "internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal", e.to_string())
            }
        };
        (status, Json(ErrorBody { code: code.into(), message })).into_response()
    }
}
