use std::fmt::Display;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

use crate::infra::errors::InfraError;
use crate::operation_result::OperationResult;

#[derive(Debug)]
pub enum AppError {
    InternalServerError,
    DatabaseError(String),
    NotFound(String),
    BodyParsingError(String),
    Unauthorized,
    OidcError(String),
    Conflict,
}

impl Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::InternalServerError => write!(f, "Internal server error"),
            AppError::DatabaseError(e) => write!(f, "Database error: {}", e),
            AppError::NotFound(e) => write!(f, "Not found: {}", e),
            AppError::BodyParsingError(e) => write!(f, "Body parsing error: {}", e),
            AppError::Unauthorized => write!(f, "Unathorized"),
            AppError::OidcError(e) => write!(f, "OIDC Error: {}", e),
            AppError::Conflict => write!(f, "Conflict"),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, err_msg) = match self {
            Self::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                String::from("Internal Server Error"),
            ),
            Self::DatabaseError(e) => (StatusCode::INTERNAL_SERVER_ERROR, String::from(e)),
            Self::NotFound(message) => (StatusCode::NOT_FOUND, message),
            Self::BodyParsingError(message) => (
                StatusCode::BAD_REQUEST,
                format!("Bad request error: {message}"),
            ),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, String::from("Unauthorized")),
            Self::OidcError(message) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("OIDC error: {message}"),
            ),
            Self::Conflict => (StatusCode::CONFLICT, String::from("Conflict")),
        };
        (status, Json(json!({ "message": err_msg }))).into_response()
    }
}

impl From<url::ParseError> for AppError {
    fn from(e: url::ParseError) -> Self {
        Self::OidcError(e.to_string())
    }
}

impl From<openidconnect::ConfigurationError> for AppError {
    fn from(e: openidconnect::ConfigurationError) -> Self {
        Self::OidcError(e.to_string())
    }
}

impl From<openidconnect::DiscoveryError<openidconnect::reqwest::Error>> for AppError {
    fn from(e: openidconnect::DiscoveryError<openidconnect::reqwest::Error>) -> Self {
        Self::OidcError(format!("{e:?}"))
    }
}

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(e: jsonwebtoken::errors::Error) -> Self {
        Self::OidcError(e.to_string())
    }
}

impl From<InfraError> for AppError {
    fn from(e: InfraError) -> Self {
        match e {
            InfraError::InternalServerError => Self::InternalServerError,
            InfraError::NotFound => Self::NotFound("Not found".into()),
            InfraError::Conflict => Self::Conflict,
        }
    }
}

impl OperationResult for AppError {
    fn is_success(&self) -> bool {
        false
    }
}

impl<T> OperationResult for Result<T, AppError> {
    fn is_success(&self) -> bool {
        self.is_ok()
    }
}
