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
        id: "c6f591e2-5225-11f1-a32d-1715edb436ac".to_string(),
        user: Some(UserResponse {
            id: "b78f01ac-5225-11f1-a097-e71b54ed4087".to_string(),
            name: "Ivan Ivanov".to_string(),
            username: "ivan_ivanov".to_string(),
            avatar: None,
        }),
    };
    Ok(Json(session))
}
