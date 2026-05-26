use serde::Serialize;
use std::fmt;
use worker::{Headers, Response};

/// Application-wide error type.
#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Internal(String),
    FormBricksError(String),
}

impl AppError {
    pub fn status_code(&self) -> u16 {
        match self {
            AppError::BadRequest(_) => 400,
            AppError::Unauthorized(_) => 401,
            AppError::Forbidden(_) => 403,
            AppError::NotFound(_) => 404,
            AppError::Internal(_) => 500,
            AppError::FormBricksError(_) => 502,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            AppError::BadRequest(m)
            | AppError::Unauthorized(m)
            | AppError::Forbidden(m)
            | AppError::NotFound(m)
            | AppError::Internal(m)
            | AppError::FormBricksError(m) => m,
        }
    }

    pub fn to_response(&self) -> worker::Result<Response> {
        let body = ErrorResponse {
            error: ErrorDetail {
                code: self.status_code(),
                message: self.message().to_string(),
            },
        };
        let mut headers = Headers::new();
        headers.set("Content-Type", "application/json")?;
        let response = Response::from_json(&body)?;
        Ok(response
            .with_headers(headers)
            .with_status(self.status_code()))
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}): {}",
            self.status_code(),
            self.label(),
            self.message()
        )
    }
}

impl AppError {
    fn label(&self) -> &'static str {
        match self {
            AppError::BadRequest(_) => "Bad Request",
            AppError::Unauthorized(_) => "Unauthorized",
            AppError::Forbidden(_) => "Forbidden",
            AppError::NotFound(_) => "Not Found",
            AppError::Internal(_) => "Internal Server Error",
            AppError::FormBricksError(_) => "Bad Gateway",
        }
    }
}

impl std::error::Error for AppError {}

impl From<worker::Error> for AppError {
    fn from(err: worker::Error) -> Self {
        AppError::Internal(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::BadRequest(err.to_string())
    }
}

impl From<AppError> for worker::Error {
    fn from(err: AppError) -> Self {
        // Convert to a JSON error response string so it's descriptive.
        worker::Error::RustError(err.to_string())
    }
}

#[derive(Serialize)]
struct ErrorDetail {
    code: u16,
    message: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: ErrorDetail,
}
