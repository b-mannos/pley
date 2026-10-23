use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Error type returned by every HTTP handler. Serialises to
/// `{ "code": "...", "message": "..." }`.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("authentication required")]
    Unauthorized,
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Unavailable(String),
    #[error("{0}")]
    TooManyRequests(String),
    /// The second step of a sign-in is missing or wrong (see `crate::two_step`).
    #[error("{0}")]
    TwoStep(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub type ApiResult<T> = Result<T, AppError>;

impl AppError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest(msg.into())
    }

    pub fn not_found(msg: impl Into<String>) -> Self {
        Self::NotFound(msg.into())
    }

    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Conflict(msg.into())
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden(msg.into())
    }

    /// The same error again, for each of several requests that waited on one
    /// piece of work.
    pub fn again(&self) -> Self {
        match self {
            Self::BadRequest(m) => Self::BadRequest(m.clone()),
            Self::Unauthorized => Self::Unauthorized,
            Self::Forbidden(m) => Self::Forbidden(m.clone()),
            Self::NotFound(m) => Self::NotFound(m.clone()),
            Self::Conflict(m) => Self::Conflict(m.clone()),
            Self::Unavailable(m) => Self::Unavailable(m.clone()),
            Self::TooManyRequests(m) => Self::TooManyRequests(m.clone()),
            Self::TwoStep(m) => Self::TwoStep(m.clone()),
            Self::Internal(err) => Self::Internal(anyhow::anyhow!("{err:#}")),
        }
    }

    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::Forbidden(_) => (StatusCode::FORBIDDEN, "forbidden"),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            Self::Unavailable(_) => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
            Self::TooManyRequests(_) => (StatusCode::TOO_MANY_REQUESTS, "too_many_requests"),
            Self::TwoStep(_) => (StatusCode::UNAUTHORIZED, "two_step"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => Self::NotFound("not found".into()),
            other => Self::Internal(other.into()),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        match err.kind() {
            std::io::ErrorKind::NotFound => Self::NotFound("file not found".into()),
            _ => Self::Internal(err.into()),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = self.status_and_code();
        let message = match &self {
            Self::Internal(err) => {
                tracing::error!(error = ?err, "internal error");
                "internal server error".to_owned()
            }
            other => other.to_string(),
        };
        (status, Json(json!({ "code": code, "message": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn answer(err: AppError) -> (u16, serde_json::Value) {
        let response = err.into_response();
        let status = response.status().as_u16();
        let bytes = axum::body::to_bytes(response.into_body(), 1 << 16).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn errors_answer_with_a_status_code_and_message() {
        for (err, status, code) in [
            (AppError::bad_request("no"), 400, "bad_request"),
            (AppError::Unauthorized, 401, "unauthorized"),
            (AppError::forbidden("no"), 403, "forbidden"),
            (AppError::not_found("no"), 404, "not_found"),
            (AppError::conflict("no"), 409, "conflict"),
            (AppError::TooManyRequests("no".into()), 429, "too_many_requests"),
            (AppError::Unavailable("no".into()), 503, "unavailable"),
            (AppError::TwoStep("no".into()), 401, "two_step"),
        ] {
            let (got, body) = answer(err).await;
            assert_eq!((got, body["code"].as_str().unwrap()), (status, code));
        }
        assert_eq!(answer(AppError::not_found("item 42 not found")).await.1["message"], "item 42 not found");
    }

    #[tokio::test]
    async fn internal_errors_keep_their_details_out_of_the_answer() {
        let (status, body) = answer(AppError::Internal(anyhow::anyhow!("disk at /secret/path is full"))).await;
        assert_eq!(status, 500);
        assert_eq!(body, json!({"code": "internal", "message": "internal server error"}));
    }

    #[test]
    fn missing_rows_and_files_are_not_found() {
        assert!(matches!(AppError::from(sqlx::Error::RowNotFound), AppError::NotFound(_)));
        assert!(matches!(AppError::from(sqlx::Error::PoolTimedOut), AppError::Internal(_)));
        assert!(matches!(AppError::from(std::io::Error::from(std::io::ErrorKind::NotFound)), AppError::NotFound(_)));
        let denied = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert!(matches!(AppError::from(denied), AppError::Internal(_)));
    }
}
