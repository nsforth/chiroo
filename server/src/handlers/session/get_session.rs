use axum::extract::State;
use axum::response::ErrorResponse;
use axum::Json;

use crate::handlers::session::{SessionResponse, UserResponse};
use crate::middleware::auth::AuthUser;
use crate::AppState;

pub async fn get_session(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<SessionResponse>, ErrorResponse> {
    let user = state.auth_service.get_user(auth_user.user_id).await?;

    let session = SessionResponse {
        created_at: user.created_at.and_utc().timestamp(),
        id: auth_user.user_id.to_string(),
        user: Some(UserResponse {
            id: auth_user.user_id.to_string(),
            name: user.name.unwrap_or_default(),
            username: user.username.unwrap_or_default(),
            avatar: user.avatar_url,
        }),
    };
    Ok(Json(session))
}
