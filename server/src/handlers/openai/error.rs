use axum::http::header;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use crate::errors::AppError;

/// OpenAI error envelope: {"error": {message, type, param, code}}
#[derive(Debug)]
pub struct OpenAiApiError {
    status: StatusCode,
    message: String,
    err_type: &'static str,
    param: Option<&'static str>,
    code: Option<&'static str>,
}

impl OpenAiApiError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
            err_type: "invalid_request_error",
            param: None,
            code: None,
        }
    }

    pub fn unauthenticated(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
            err_type: "invalid_request_error",
            param: None,
            code: Some("invalid_api_key"),
        }
    }

    pub fn model_not_found(model: &str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: format!("Model `{model}` not found"),
            err_type: "invalid_request_error",
            param: Some("model"),
            code: Some("model_not_found"),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
            err_type: "server_error",
            param: None,
            code: None,
        }
    }
}

impl From<AppError> for OpenAiApiError {
    fn from(e: AppError) -> Self {
        match e {
            AppError::BadRequest(message) => Self::bad_request(message),
            AppError::NotFound(message) => Self {
                status: StatusCode::NOT_FOUND,
                message,
                err_type: "invalid_request_error",
                param: None,
                code: None,
            },
            // Never leak internal details (DB, OIDC, ...) to API clients;
            // the detail is logged server-side.
            other => {
                tracing::error!(
                    error = other.to_string(),
                    status = "openai_api_internal_error"
                );
                Self::internal("Internal server error")
            },
        }
    }
}

fn error_response(
    status: StatusCode,
    message: String,
    err_type: &'static str,
    param: Option<&'static str>,
    code: Option<&'static str>,
) -> Response {
    let body = Json(json!({
        "error": {
            "message": message,
            "type": err_type,
            "param": param,
            "code": code,
        }
    }));
    let mut response = (status, body).into_response();
    if status == StatusCode::UNAUTHORIZED {
        response.headers_mut().insert(
            header::WWW_AUTHENTICATE,
            header::HeaderValue::from_static("Bearer"),
        );
    }
    response
}

pub fn openai_error_response(
    status: StatusCode,
    message: impl Into<String>,
    err_type: &'static str,
    param: Option<&'static str>,
    code: Option<&'static str>,
) -> Response {
    error_response(status, message.into(), err_type, param, code)
}

impl IntoResponse for OpenAiApiError {
    fn into_response(self) -> Response {
        error_response(
            self.status,
            self.message,
            self.err_type,
            self.param,
            self.code,
        )
    }
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;

    use super::*;

    async fn error_json(error: OpenAiApiError) -> serde_json::Value {
        let response = error.into_response();
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        serde_json::from_slice(&bytes).expect("valid JSON")
    }

    #[tokio::test]
    async fn unauthenticated_has_401_and_www_authenticate() {
        let response = OpenAiApiError::unauthenticated("Nope").into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            response
                .headers()
                .get(header::WWW_AUTHENTICATE)
                .and_then(|v| v.to_str().ok()),
            Some("Bearer")
        );
        let value = error_json(OpenAiApiError::unauthenticated("Nope")).await;
        assert_eq!(value["error"]["message"], "Nope");
        assert_eq!(value["error"]["type"], "invalid_request_error");
        assert_eq!(value["error"]["code"], "invalid_api_key");
        assert_eq!(value["error"]["param"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn bad_request_has_400() {
        assert_eq!(
            OpenAiApiError::bad_request("Nope").into_response().status(),
            StatusCode::BAD_REQUEST
        );
        let value = error_json(OpenAiApiError::bad_request("Nope")).await;
        assert_eq!(value["error"]["type"], "invalid_request_error");
        assert_eq!(value["error"]["code"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn model_not_found_has_404_and_code() {
        assert_eq!(
            OpenAiApiError::model_not_found("gpt-4o")
                .into_response()
                .status(),
            StatusCode::NOT_FOUND
        );
        let value = error_json(OpenAiApiError::model_not_found("gpt-4o")).await;
        assert_eq!(value["error"]["message"], "Model `gpt-4o` not found");
        assert_eq!(value["error"]["type"], "invalid_request_error");
        assert_eq!(value["error"]["param"], "model");
        assert_eq!(value["error"]["code"], "model_not_found");
    }

    #[tokio::test]
    async fn internal_has_500_and_server_error_type() {
        assert_eq!(
            OpenAiApiError::internal("boom").into_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        let value = error_json(OpenAiApiError::internal("boom")).await;
        assert_eq!(value["error"]["type"], "server_error");
        assert_eq!(value["error"]["code"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn app_error_bad_request_maps_to_400() {
        let error = OpenAiApiError::from(AppError::BadRequest("bad input".into()));
        assert_eq!(error.into_response().status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn app_error_not_found_maps_to_404() {
        let error = OpenAiApiError::from(AppError::NotFound("gone".into()));
        assert_eq!(error.into_response().status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn app_error_not_found_type() {
        let error = OpenAiApiError::from(AppError::NotFound("gone".into()));
        let value = error_json(error).await;
        assert_eq!(value["error"]["type"], "invalid_request_error");
    }

    #[tokio::test]
    async fn app_error_internal_maps_to_500() {
        let error = OpenAiApiError::from(AppError::InternalServerError);
        assert_eq!(
            error.into_response().status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[tokio::test]
    async fn app_error_internal_message_does_not_leak_details() {
        let detailed = "Database error: password=hunter2 host=db.internal";
        let error = OpenAiApiError::from(AppError::DatabaseError(detailed.into()));
        let value = error_json(error).await;
        assert_eq!(value["error"]["message"], "Internal server error");
        let rendered = value["error"]["message"].to_string();
        assert!(!rendered.contains("hunter2"));
        assert!(!rendered.contains("db.internal"));
    }
}
