use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use cookie::Cookie;
use secrecy::ExposeSecret;
use serde::Deserialize;

use crate::middleware::csrf::create_csrf_token;
use crate::AppState;

#[derive(Deserialize)]
pub struct CallbackParams {
    pub code: String,
    pub state: String,
}

pub async fn oidc_callback(
    State(state): State<AppState>,
    Query(params): Query<CallbackParams>,
) -> Result<Response, StatusCode> {
    match state
        .auth_service
        .handle_callback(&params.code, &params.state)
        .await
    {
        Ok((redirect_url, access_token, auth_user)) => {
            let secure = state.config.oidc_config().secure_cookie();
            let access_cookie = Cookie::build(("access_token", access_token.expose_secret()))
                .http_only(true)
                .path("/api")
                .secure(secure)
                .same_site(cookie::SameSite::Lax)
                .to_string();

            let csrf_secret = state.config.oidc_config().state_secret().expose_secret();
            let csrf_jwt = create_csrf_token(csrf_secret, &auth_user.session_id)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let csrf_cookie = Cookie::build(("csrf-token", csrf_jwt))
                .http_only(false)
                .path("/")
                .secure(secure)
                .same_site(cookie::SameSite::Lax)
                .to_string();

            let mut response = Redirect::temporary(&redirect_url).into_response();
            response.headers_mut().insert(
                header::SET_COOKIE,
                header::HeaderValue::from_str(&access_cookie).unwrap(),
            );
            response.headers_mut().append(
                header::SET_COOKIE,
                header::HeaderValue::from_str(&csrf_cookie).unwrap(),
            );
            Ok(response)
        },
        Err(_) => Ok(Redirect::temporary("/auth/error").into_response()),
    }
}
