use axum::{
    extract::{FromRequestParts, State},
    http::{header, request::Parts, HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use cookie::Cookie;
use secrecy::{ExposeSecret, SecretBox};
use serde_json::json;

use crate::domain::models::auth_user::AuthUser;
use crate::services::auth_service::AuthResult;
use crate::utils::extract_cookie;
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
        .path("/")
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

pub async fn auth_middleware(
    State(state): State<AppState>,
    mut request: axum::extract::Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = match extract_cookie(request.headers(), "access_token")
        .map(|t| SecretBox::from(t.into_boxed_str()))
    {
        Some(t) => t,
        None => {
            tracing::debug!("Request without access token to protected route");
            return Ok(unauthorized_response());
        },
    };

    let (new_request, new_cookie) = match state.auth_service.verify_or_refresh(&token).await {
        AuthResult::Valid(auth_user) => {
            request.extensions_mut().insert(auth_user);
            (request, None)
        },
        AuthResult::Refreshed(auth_user, access_token) => {
            tracing::info!("Refreshing token for user: {}", auth_user.user_id);
            request.extensions_mut().insert(auth_user);
            let cookie =
                build_set_cookie(&access_token, state.config.oidc_config().secure_cookie());
            (request, Some(cookie))
        },
        AuthResult::Expired => {
            tracing::debug!("Access token is expired");
            return Ok(unauthorized_response());
        },
        AuthResult::Invalid => {
            tracing::debug!("Access token is invalid");
            return Ok(unauthorized_response());
        },
    };

    let mut response = next.run(new_request).await;

    if let Some(cookie) = new_cookie {
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            response.headers_mut().insert(header::SET_COOKIE, value);
        }
    }

    Ok(response)
}
