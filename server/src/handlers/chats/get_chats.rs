use axum::extract::State;
use axum::response::ErrorResponse;
use axum::Json;

use crate::handlers::chats::{Chat, ChatsResponse};
use crate::AppState;

pub async fn get_chats(State(_): State<AppState>) -> Result<Json<ChatsResponse>, ErrorResponse> {
    let mut chats = Vec::new();
    /*chats.push(Chat {
        id: "1".to_string(),
        title: Some("First".to_string()),
        user_id: "1".to_string(),
        visibility: "private".to_string(),
        created_at: "2026-05-12T10:00:00".to_string(),
    });*/
    Ok(Json(ChatsResponse { chats }))
}
