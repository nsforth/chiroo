use axum::extract::State;
use axum::response::ErrorResponse;
use axum::Json;
use uuid::Uuid;

use crate::domain::models::user::UserId;
use crate::handlers::chats::{ChatResponse, ChatsResponse, CreateChatRequest};
use crate::utils::JsonExtractor;
use crate::AppState;

pub async fn get_chats(
    State(state): State<AppState>,
) -> Result<Json<ChatsResponse>, ErrorResponse> {
    let chats = state.chat_service.get_chats().await?;
    Ok(Json(From::from(chats)))
}

//#[axum::debug_handler]
pub async fn create_chat(
    State(state): State<AppState>,
    JsonExtractor(new_chat): JsonExtractor<CreateChatRequest>,
) -> Result<Json<ChatResponse>, ErrorResponse> {
    let owner = Uuid::parse_str("b78f01ac-5225-11f1-a097-e71b54ed4087").unwrap();
    let result = state
        .chat_service
        .create_chat(new_chat.title, UserId(owner))
        .await?;
    Ok(Json(From::from(&result)))
}
