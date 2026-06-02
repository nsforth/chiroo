use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use cookie::Cookie;
use serde::Deserialize;

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
        Ok((redirect_url, access_token)) => {
            let cookie = Cookie::build(("access_token", access_token))
                .http_only(true)
                .path("/")
                .secure(state.config.oidc_config().secure_cookie())
                .same_site(cookie::SameSite::Lax)
                .to_string();

            let mut response = Redirect::temporary(&redirect_url).into_response();
            response.headers_mut().insert(
                header::SET_COOKIE,
                header::HeaderValue::from_str(&cookie).unwrap(),
            );
            Ok(response)
        },
        Err(_) => Ok(Redirect::temporary("/auth/error").into_response()),
    }
}
