use axum::{
    extract::{FromRequestParts, State},
    http::{header, request::Parts, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::cookie::CookieJar;
use axum_extra::headers::authorization::{Authorization, Bearer};
use axum_extra::typed_header::TypedHeaderRejection;
use axum_extra::TypedHeader;
use cookie::Cookie;
use secrecy::{ExposeSecret, SecretBox};
use serde_json::json;

use crate::domain::models::auth_user::AuthUser;
use crate::errors::AppError;
use crate::handlers::openai::error::openai_error_response;
use crate::services::auth_service::AuthResult;
use crate::AppState;

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<AuthUser>().cloned().ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(json!({"message": "Authentication required"})),
            )
        })
    }
}

fn build_set_cookie(token: &SecretBox<str>, secure: bool) -> String {
    Cookie::build(("access_token", token.expose_secret().to_string()))
        .http_only(true)
        .path("/api")
        .secure(secure)
        .same_site(cookie::SameSite::Lax)
        .to_string()
}

fn unauthorized_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({"message": "Unauthorized"})),
    )
        .into_response()
}

fn openai_unauthorized_response() -> Response {
    openai_error_response(
        StatusCode::UNAUTHORIZED,
        "Invalid or missing API token",
        "invalid_request_error",
        None,
        Some("invalid_api_key"),
    )
}

/// Verify a Bearer token (stateless path: scripts, API consumers, mobile app
/// sometime in future, who knows?) and insert the `AuthUser` into the request
/// extensions.
async fn authenticate_bearer(
    state: &AppState,
    token: &str,
    request: &mut axum::extract::Request,
) -> Result<(), AppError> {
    let auth_user = state
        .auth_service
        .verify_api_token(&SecretBox::from(token.to_owned().into_boxed_str()))
        .await?;
    request.extensions_mut().insert(auth_user);
    Ok(())
}

pub async fn auth_middleware(
    State(state): State<AppState>,
    auth: Result<Option<TypedHeader<Authorization<Bearer>>>, TypedHeaderRejection>,
    jar: CookieJar,
    mut request: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Bearer token first: if an `Authorization` header is presented it must be
    // a valid Bearer token — there is no fallback to cookies. Only requests
    // without the header at all use the cookie session.
    match auth {
        Err(_) => return Ok(unauthorized_response()),
        Ok(None) => {},
        Ok(Some(auth)) => {
            return match authenticate_bearer(&state, auth.token(), &mut request).await {
                Ok(()) => Ok(next.run(request).await),
                Err(_) => Ok(unauthorized_response()),
            };
        },
    }

    // Cookie session: verifies the token and, if it is expired, refreshes it
    // via the server session.
    let token = match jar.get("access_token").map(|c| c.value().to_string()) {
        Some(t) => SecretBox::from(t.into_boxed_str()),
        None => return Ok(unauthorized_response()),
    };

    let new_cookie = match state.auth_service.verify_or_refresh(&token).await {
        AuthResult::Valid(auth_user) => {
            request.extensions_mut().insert(auth_user);
            None
        },
        AuthResult::Refreshed(auth_user, access_token) => {
            request.extensions_mut().insert(auth_user);
            Some(build_set_cookie(
                &access_token,
                state.config.oidc_config().secure_cookie(),
            ))
        },
        AuthResult::Expired | AuthResult::Invalid => return Ok(unauthorized_response()),
    };

    let mut response = next.run(request).await;

    if let Some(cookie) = new_cookie {
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            response.headers_mut().insert(header::SET_COOKIE, value);
        }
    }

    Ok(response)
}

/// Authentication for the OpenAI-compatible API: Bearer token only (no cookie
/// path, no refresh), 401s use the OpenAI error envelope.
pub async fn openai_auth_middleware(
    State(state): State<AppState>,
    auth: Result<Option<TypedHeader<Authorization<Bearer>>>, TypedHeaderRejection>,
    mut request: axum::extract::Request,
    next: Next,
) -> Response {
    let Some(auth) = auth.ok().flatten() else {
        return openai_unauthorized_response();
    };
    match authenticate_bearer(&state, auth.token(), &mut request).await {
        Ok(()) => next.run(request).await,
        Err(_) => openai_unauthorized_response(),
    }
}
