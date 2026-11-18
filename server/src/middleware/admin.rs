use axum::{
    extract::State,
    middleware::Next,
    response::{IntoResponse, Response},
};
use tracing::debug;

use crate::domain::models::auth_user::AuthUser;
use crate::errors::AppError;
use crate::AppState;

pub async fn admin_middleware(
    State(state): State<AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    let Some(user_id) = request
        .extensions()
        .get::<AuthUser>()
        .map(|auth_user| auth_user.user_id)
    else {
        return AppError::Unauthorized.into_response();
    };

    let is_admin = match state.user_service.get_user(user_id).await {
        Ok(user) => user.admin,
        Err(AppError::NotFound(_)) => false,
        Err(error) => {
            debug!(error = error.to_string(), status = "failed");
            return error.into_response();
        },
    };

    if !is_admin {
        debug!(status = "forbidden");
        return AppError::Forbidden.into_response();
    }

    next.run(request).await
}
