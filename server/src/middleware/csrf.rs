use axum::{
    extract::State,
    http::{Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::domain::models::auth_user::AuthUser;
use crate::utils::extract_cookie;
use crate::AppState;

const CSRF_COOKIE_NAME: &str = "csrf-token";
const CSRF_HEADER_NAME: &str = "x-csrf-token";

#[derive(Serialize, Deserialize)]
struct CsrfClaims {
    jti: String,
    sid: String,
}

pub fn create_csrf_token(
    secret: &str,
    session_id: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    let claims = CsrfClaims {
        jti: Uuid::new_v4().to_string(),
        sid: session_id.to_string(),
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

fn verify_csrf_token(token: &str, secret: &str) -> Result<CsrfClaims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = false;
    validation.required_spec_claims.clear();
    decode::<CsrfClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
}

fn is_mutating(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

pub async fn csrf_middleware(
    State(state): State<AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if !is_mutating(request.method()) {
        return Ok(next.run(request).await);
    }

    let cookie_token = extract_cookie(request.headers(), CSRF_COOKIE_NAME);
    let header_token = request
        .headers()
        .get(CSRF_HEADER_NAME)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let cookie_token = match cookie_token {
        Some(t) => t,
        None => return Ok(unauthorized_response()),
    };

    let header_token = match header_token {
        Some(t) => t,
        None => return Ok(unauthorized_response()),
    };

    if cookie_token != header_token {
        return Ok(unauthorized_response());
    }

    let secret: &str = state.config.oidc_config().state_secret().expose_secret();
    let claims = match verify_csrf_token(&cookie_token, secret) {
        Ok(c) => c,
        Err(_) => return Ok(unauthorized_response()),
    };

    let auth_user = request.extensions().get::<AuthUser>();

    match auth_user {
        Some(user) if user.session_id == claims.sid => Ok(next.run(request).await),
        _ => return Ok(unauthorized_response()),
    }
}

fn unauthorized_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"message": "Unauthorized"})),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csrf_token_round_trips() {
        let token = create_csrf_token("secret", "session-1").unwrap();
        let claims = verify_csrf_token(&token, "secret").unwrap();
        assert_eq!(claims.sid, "session-1");
    }

    #[test]
    fn csrf_token_rejected_with_wrong_secret() {
        let token = create_csrf_token("secret", "session-1").unwrap();
        assert!(verify_csrf_token(&token, "other-secret").is_err());
    }

    #[test]
    fn csrf_token_rejected_when_tampered() {
        let token = create_csrf_token("secret", "session-1").unwrap();
        let parts: Vec<&str> = token.split('.').collect();
        assert_eq!(parts.len(), 3);
        let header = parts[0];
        let payload = parts[1];
        let forged = format!("{header}.{payload}.invalid-signature");
        assert!(verify_csrf_token(&forged, "secret").is_err());
    }

    #[test]
    fn csrf_token_rejected_for_garbage() {
        assert!(verify_csrf_token("not-a-jwt", "secret").is_err());
    }

    #[test]
    fn is_mutating_matches_only_write_methods() {
        for (method, expected) in [
            (axum::http::Method::POST, true),
            (axum::http::Method::PUT, true),
            (axum::http::Method::PATCH, true),
            (axum::http::Method::DELETE, true),
            (axum::http::Method::GET, false),
            (axum::http::Method::HEAD, false),
        ] {
            assert_eq!(is_mutating(&method), expected, "{method:?}");
        }
    }
}
