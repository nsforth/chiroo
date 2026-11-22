use axum::{
    extract::State,
    http::{Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::cookie::CookieJar;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::domain::models::auth_user::AuthUser;
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

/// Returns `true` when the request passes the CSRF check: the `csrf-token`
/// cookie and the `x-csrf-token` header carry the same JWT, the JWT verifies
/// with the OIDC state secret, and its `sid` matches the `AuthUser` session.
fn check_csrf(state: &AppState, jar: &CookieJar, request: &axum::extract::Request) -> bool {
    let cookie_token = match jar.get(CSRF_COOKIE_NAME) {
        Some(c) => c.value().to_string(),
        None => return false,
    };

    let header_token = match request
        .headers()
        .get(CSRF_HEADER_NAME)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
    {
        Some(t) => t,
        None => return false,
    };

    if cookie_token != header_token {
        return false;
    }

    let secret: &str = state.config.oidc_config().state_secret().expose_secret();
    let claims = match verify_csrf_token(&cookie_token, secret) {
        Ok(c) => c,
        Err(_) => return false,
    };

    request
        .extensions()
        .get::<AuthUser>()
        .is_some_and(|user| user.session_id.as_deref() == Some(claims.sid.as_str()))
}

/// CSRF protection, applied after authentication. Only requests attached to
/// a server session (the cookie path) are enforced: stateless requests
/// (Bearer tokens) skip the check entirely, since a cross-site origin cannot
/// forge the `Authorization` header, so there is nothing to protect against.
pub async fn csrf_middleware(
    State(state): State<AppState>,
    jar: CookieJar,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let bound_to_session = request
        .extensions()
        .get::<AuthUser>()
        .is_some_and(|user| user.session_id.is_some());
    if bound_to_session && is_mutating(request.method()) && !check_csrf(&state, &jar, &request) {
        return unauthorized_response();
    }
    next.run(request).await
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
