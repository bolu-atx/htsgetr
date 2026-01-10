use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid authentication")]
    InvalidAuthentication,

    #[error("permission denied")]
    PermissionDenied,

    #[error("not found: {0}")]
    NotFound(String),

    #[error("payload too large")]
    PayloadTooLarge,

    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("invalid range: {0}")]
    InvalidRange(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Debug, Serialize)]
pub struct HtsgetError {
    pub htsget: HtsgetErrorBody,
}

#[derive(Debug, Serialize)]
pub struct HtsgetErrorBody {
    pub error: &'static str,
    pub message: String,
}

impl Error {
    fn error_type(&self) -> &'static str {
        match self {
            Error::InvalidAuthentication => "InvalidAuthentication",
            Error::PermissionDenied => "PermissionDenied",
            Error::NotFound(_) => "NotFound",
            Error::PayloadTooLarge => "PayloadTooLarge",
            Error::UnsupportedFormat(_) => "UnsupportedFormat",
            Error::InvalidInput(_) => "InvalidInput",
            Error::InvalidRange(_) => "InvalidRange",
            Error::Io(_) | Error::Internal(_) => "InternalError",
        }
    }

    fn status_code(&self) -> StatusCode {
        match self {
            Error::InvalidAuthentication => StatusCode::UNAUTHORIZED,
            Error::PermissionDenied => StatusCode::FORBIDDEN,
            Error::NotFound(_) => StatusCode::NOT_FOUND,
            Error::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Error::UnsupportedFormat(_) => StatusCode::BAD_REQUEST,
            Error::InvalidInput(_) => StatusCode::BAD_REQUEST,
            Error::InvalidRange(_) => StatusCode::BAD_REQUEST,
            Error::Io(_) | Error::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let body = HtsgetError {
            htsget: HtsgetErrorBody {
                error: self.error_type(),
                message: self.to_string(),
            },
        };
        (self.status_code(), axum::Json(body)).into_response()
    }
}
