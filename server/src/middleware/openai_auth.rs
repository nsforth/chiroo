use axum::{extract::State, http::StatusCode, middleware::Next, response::Response};
use secrecy::SecretBox;

use crate::handlers::openai::error::openai_error_response;
use crate::AppState;

/// Extract the token from `Authorization: Bearer <token>`; scheme match is
/// case-insensitive, the token must be non-empty.
pub fn extract_bearer(headers: &axum::http::HeaderMap) -> Option<String> {
    let value = headers.get("authorization")?.to_str().ok()?.trim_start();
    let (scheme, token) = value.split_once(char::is_whitespace)?;
    let token = token.trim();
    if scheme.eq_ignore_ascii_case("bearer") && !token.is_empty() {
        Some(token.to_string())
    } else {
        None
    }
}

pub async fn openai_auth_middleware(
    State(state): State<AppState>,
    mut request: axum::extract::Request,
    next: Next,
) -> Response {
    let token = match extract_bearer(request.headers()) {
        Some(token) => SecretBox::from(token.into_boxed_str()),
        None => {
            return openai_error_response(
                StatusCode::UNAUTHORIZED,
                "Invalid or missing API token",
                "invalid_request_error",
                None,
                Some("invalid_api_key"),
            )
        },
    };

    match state.auth_service.verify_api_token(&token).await {
        Ok(principal) => {
            request.extensions_mut().insert(principal);
            next.run(request).await
        },
        Err(_) => openai_error_response(
            StatusCode::UNAUTHORIZED,
            "Invalid or missing API token",
            "invalid_request_error",
            None,
            Some("invalid_api_key"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    fn headers_with_auth(value: &str) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert("authorization", HeaderValue::from_str(value).unwrap());
        map
    }

    #[test]
    fn extract_bearer_standard() {
        assert_eq!(
            extract_bearer(&headers_with_auth("Bearer abc")),
            Some("abc".into())
        );
    }

    #[test]
    fn extract_bearer_lowercase_scheme() {
        assert_eq!(
            extract_bearer(&headers_with_auth("bearer abc")),
            Some("abc".into())
        );
    }

    #[test]
    fn extract_bearer_empty_token() {
        assert_eq!(extract_bearer(&headers_with_auth("Bearer ")), None);
    }

    #[test]
    fn extract_bearer_other_scheme() {
        assert_eq!(extract_bearer(&headers_with_auth("Basic abc")), None);
    }

    #[test]
    fn extract_bearer_missing_header() {
        assert_eq!(extract_bearer(&HeaderMap::new()), None);
    }

    #[test]
    fn extract_bearer_no_space() {
        assert_eq!(extract_bearer(&headers_with_auth("Bearer")), None);
    }
}
