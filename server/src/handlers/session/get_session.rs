use axum::extract::State;
use axum::response::ErrorResponse;
use axum::Json;

use crate::domain::models::auth_user::AuthUser;
use crate::handlers::session::SessionResponse;
use crate::handlers::user::PublicUserResponse;
use crate::AppState;

pub async fn get_session(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<SessionResponse>, ErrorResponse> {
    let user = state.user_service.get_user(auth_user.user_id).await?;

    let session = SessionResponse {
        created_at: user.created_at.and_utc().to_rfc3339(),
        id: auth_user.user_id.to_string(),
        user: Some(PublicUserResponse {
            id: auth_user.user_id.to_string(),
            name: user.name.unwrap_or_default(),
            username: user.username,
            avatar: user.avatar_url,
            admin: user.admin,
        }),
    };
    Ok(Json(session))
}
