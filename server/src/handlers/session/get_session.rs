use axum::extract::State;
use axum::response::ErrorResponse;
use axum::Json;

use crate::handlers::session::{SessionResponse, UserResponse};
use crate::AppState;

pub async fn get_session(
    State(_): State<AppState>,
) -> Result<Json<SessionResponse>, ErrorResponse> {
    let session = SessionResponse {
        created_at: 0,
        id: "1234".to_string(),
        user: Some(UserResponse {
            id: "1".to_string(),
            name: "Ivan Ivanov".to_string(),
            username: "ivan_ivanov".to_string(),
            avatar: None,
        }),
    };
    Ok(Json(session))
}
