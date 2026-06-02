use axum::{
    extract::{FromRequestParts, State},
    http::{header, request::Parts, HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use cookie::Cookie;
use serde_json::json;

use crate::domain::models::user::{User, UserId};
use crate::services::auth_service::AuthResult;
use crate::AppState;

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user_id: UserId,
    pub provider_sid: String,
    pub name: Option<String>,
    pub username: String,
    pub email: Option<String>,
}

impl AuthUser {
    fn from_sid_and_user(sid: String, user: User) -> Self {
        AuthUser {
            user_id: user.id,
            provider_sid: sid,
            name: user.name,
            username: user.username.unwrap_or_default(),
            email: user.email,
        }
    }
}

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

fn extract_access_token(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    Cookie::parse(cookie_header)
        .ok()
        .filter(|c| c.name() == "access_token")
        .map(|c| c.value_trimmed().to_string())
}

fn build_set_cookie(token: &str) -> String {
    Cookie::build(("access_token", token.to_string()))
        .http_only(true)
        .path("/")
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
    let token = match extract_access_token(request.headers()) {
        Some(t) => t,
        None => {
            tracing::debug!("Request without access token to protected route");
            return Ok(unauthorized_response());
        },
    };

    let (new_request, new_cookie) = match state.auth_service.verify_or_refresh(&token).await {
        AuthResult::Valid { claims, user } => {
            request.extensions_mut().insert(AuthUser::from_sid_and_user(
                claims.sid.unwrap_or_default(),
                user,
            ));
            (request, None)
        },
        AuthResult::Refreshed {
            access_token,
            user,
            provider_sid,
        } => {
            request
                .extensions_mut()
                .insert(AuthUser::from_sid_and_user(provider_sid, user));
            let cookie = build_set_cookie(&access_token);
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
