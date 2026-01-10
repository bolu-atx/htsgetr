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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_types() {
        assert_eq!(
            Error::InvalidAuthentication.error_type(),
            "InvalidAuthentication"
        );
        assert_eq!(Error::PermissionDenied.error_type(), "PermissionDenied");
        assert_eq!(Error::NotFound("test".into()).error_type(), "NotFound");
        assert_eq!(Error::PayloadTooLarge.error_type(), "PayloadTooLarge");
        assert_eq!(
            Error::UnsupportedFormat("BAM".into()).error_type(),
            "UnsupportedFormat"
        );
        assert_eq!(
            Error::InvalidInput("bad".into()).error_type(),
            "InvalidInput"
        );
        assert_eq!(
            Error::InvalidRange("0-100".into()).error_type(),
            "InvalidRange"
        );
        assert_eq!(Error::Internal("oops".into()).error_type(), "InternalError");
    }

    #[test]
    fn test_error_status_codes() {
        assert_eq!(
            Error::InvalidAuthentication.status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(Error::PermissionDenied.status_code(), StatusCode::FORBIDDEN);
        assert_eq!(
            Error::NotFound("x".into()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            Error::PayloadTooLarge.status_code(),
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert_eq!(
            Error::UnsupportedFormat("x".into()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            Error::InvalidInput("x".into()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            Error::InvalidRange("x".into()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            Error::Internal("x".into()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn test_error_display() {
        assert_eq!(
            Error::NotFound("sample1".into()).to_string(),
            "not found: sample1"
        );
        assert_eq!(
            Error::UnsupportedFormat("XYZ".into()).to_string(),
            "unsupported format: XYZ"
        );
    }

    #[test]
    fn test_htsget_error_serialization() {
        let error = HtsgetError {
            htsget: HtsgetErrorBody {
                error: "NotFound",
                message: "not found: sample1".to_string(),
            },
        };
        let json = serde_json::to_string(&error).unwrap();
        assert!(json.contains("\"error\":\"NotFound\""));
        assert!(json.contains("\"message\":\"not found: sample1\""));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        assert_eq!(err.error_type(), "InternalError");
        assert_eq!(err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
