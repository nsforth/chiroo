use std::collections::HashSet;

use axum::{
    extract::State,
    http::{header, HeaderMap, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use cookie::Cookie;
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

fn extract_csrf_cookie(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    Cookie::parse(cookie_header)
        .ok()
        .filter(|c| c.name() == CSRF_COOKIE_NAME)
        .map(|c| c.value().to_string())
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

    let cookie_token = extract_csrf_cookie(request.headers());
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
    tracing::info!("CSFR: chech AuthUser {:?}", auth_user);
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
