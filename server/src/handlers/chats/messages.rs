use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{ErrorResponse, IntoResponse};
use axum::Json;
use uuid::Uuid;

use crate::domain::models::auth_user::AuthUser;
use crate::domain::models::chat::ChatId;
use crate::handlers::chats::ui_messages::{convert_parts_to_ui, MessageRole, UIMessage};
use crate::utils::JsonExtractor;
use crate::utils::PathExtractor;
use crate::AppState;

/// Response for a list of messages in a chat.
#[derive(serde::Serialize)]
pub struct MessagesResponse {
    pub messages: Vec<UIMessage>,
}

/// Get all messages for a specific chat.
pub async fn get_messages(
    State(state): State<AppState>,
    PathExtractor(chat_id): PathExtractor<Uuid>,
) -> Result<Json<MessagesResponse>, ErrorResponse> {
    let db_messages = state.message_service.get_messages_for_chat(chat_id).await?;

    let messages: Vec<UIMessage> = db_messages
        .into_iter()
        .map(|msg| message_db_to_ui_message(msg))
        .collect();

    Ok(Json(MessagesResponse { messages }))
}

/// Request body for deleting messages after a target message.
/// Frontend sends: { messageId, type: 'edit'|'regenerate' }
#[derive(serde::Deserialize)]
pub struct DeleteMessagesAfterRequest {
    #[serde(rename = "messageId")]
    pub message_id: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub r#type: Option<String>,
}

/// Delete all messages after a specific message in a chat.
/// Used for truncating history when user edits or regenerates a message.
/// Frontend calls DELETE /api/chats/messages/{chat_id} with body { messageId, type }.
pub async fn delete_messages_after(
    State(state): State<AppState>,
    auth_user: AuthUser,
    PathExtractor(chat_id): PathExtractor<Uuid>,
    JsonExtractor(body): JsonExtractor<DeleteMessagesAfterRequest>,
) -> Result<StatusCode, ErrorResponse> {
    let chat_id = ChatId(chat_id);
    state
        .chat_service
        .check_ownership(chat_id, auth_user.user_id)
        .await
        .map_err(IntoResponse::into_response)?;

    let message_id = Uuid::parse_str(&body.message_id)
        .map_err(|_| (axum::http::StatusCode::BAD_REQUEST, "Invalid message ID").into_response())?;

    match state
        .message_service
        .delete_after_message(chat_id.0, message_id)
        .await
    {
        Ok(_) => Ok(StatusCode::NO_CONTENT),
        Err(_) => Err((StatusCode::NOT_FOUND, "Message not found")
            .into_response()
            .into()),
    }
}

/// Convert a database MessageDb to the UIMessage format expected by the frontend.
fn message_db_to_ui_message(
    msg: crate::infra::repositories::message_repository::MessageDb,
) -> UIMessage {
    let role = match msg.role.as_str() {
        "user" => MessageRole::User,
        "assistant" => MessageRole::Assistant,
        _ => MessageRole::System,
    };

    let parts = convert_parts_to_ui(&msg.parts);

    UIMessage {
        id: msg.id.to_string(),
        role,
        metadata: None,
        parts,
    }
}
