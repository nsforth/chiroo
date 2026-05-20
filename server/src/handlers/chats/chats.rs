use axum::extract::State;
use axum::http::StatusCode;
use axum::response::ErrorResponse;
use axum::Json;
use uuid::Uuid;

use crate::domain::models::user::UserId;
use crate::handlers::chats::ui_messages::{MessageRole, UIMessage, UIMessagePart};
use crate::handlers::chats::{ChatResponse, ChatsResponse, CreateChatRequest};
use crate::utils::PathExtractor;
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

/// Response for a single chat including messages.
#[derive(serde::Serialize)]
pub struct ChatDetailResponse {
    pub id: String,
    #[serde(rename = "userId")]
    pub user_id: String,
    pub title: Option<String>,
    pub visibility: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub messages: Vec<UIMessage>,
    #[serde(rename = "isOwner")]
    pub is_owner: bool,
}

/// Get a single chat by ID including its messages.
pub async fn get_chat(
    State(state): State<AppState>,
    PathExtractor(id): PathExtractor<Uuid>,
) -> Result<Json<ChatDetailResponse>, ErrorResponse> {
    let chat_id = crate::domain::models::chat::ChatId(id);

    // Get chat details
    let chat = match state.chat_service.get_chat(&chat_id).await? {
        Some(chat) => chat,
        None => return Err((StatusCode::NOT_FOUND, "Chat not found").into_response()),
    };

    // Get messages for this chat
    let db_messages = state.message_service.get_messages_for_chat(id).await?;

    let messages: Vec<UIMessage> = db_messages
        .into_iter()
        .map(|msg| {
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
        })
        .collect();

    // Check if current user is the owner (hardcoded for now)
    let hardcoded_user = Uuid::parse_str("b78f01ac-5225-11f1-a097-e71b54ed4087").unwrap();
    let is_owner = chat.owner.0 == hardcoded_user;

    let visibility = if chat.public {
        "public".to_string()
    } else {
        "private".to_string()
    };

    Ok(Json(ChatDetailResponse {
        id: chat.id.0.to_string(),
        user_id: chat.owner.0.to_string(),
        title: chat.title,
        visibility,
        created_at: chat.created_at.and_utc().to_rfc3339(),
        messages,
        is_owner,
    }))
}

/// Delete a chat by ID.
pub async fn delete_chat(
    State(state): State<AppState>,
    PathExtractor(id): PathExtractor<Uuid>,
) -> Result<StatusCode, ErrorResponse> {
    let user_id = UserId(Uuid::parse_str("b78f01ac-5225-11f1-a097-e71b54ed4087").unwrap());
    let chat_id = crate::domain::models::chat::ChatId(id);
    match state.chat_service.delete_chat(&chat_id, &user_id).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(_) => Err((StatusCode::NOT_FOUND, "Chat not found").into_response()),
    }
}

/// Update chat title.
pub async fn update_title(
    State(state): State<AppState>,
    PathExtractor(id): PathExtractor<Uuid>,
    JsonExtractor(body): JsonExtractor<UpdateTitleRequest>,
) -> Result<Json<ChatResponse>, ErrorResponse> {
    let user_id = UserId(Uuid::parse_str("b78f01ac-5225-11f1-a097-e71b54ed4087").unwrap());
    let chat_id = crate::domain::models::chat::ChatId(id);
    match state.chat_service.update_title(&chat_id, &user_id, body.title).await? {
        chat => Ok(Json(From::from(&chat))),
    }
}

/// Update chat visibility (public/private).
pub async fn update_visibility(
    State(state): State<AppState>,
    PathExtractor(id): PathExtractor<Uuid>,
    JsonExtractor(body): JsonExtractor<UpdateVisibilityRequest>,
) -> Result<Json<ChatResponse>, ErrorResponse> {
    let user_id = UserId(Uuid::parse_str("b78f01ac-5225-11f1-a097-e71b54ed4087").unwrap());
    let chat_id = crate::domain::models::chat::ChatId(id);
    match state.chat_service.update_visibility(&chat_id, &user_id, body.is_public).await? {
        chat => Ok(Json(From::from(&chat))),
    }
}

#[derive(serde::Deserialize)]
pub struct UpdateTitleRequest {
    pub title: String,
}

#[derive(serde::Deserialize)]
pub struct UpdateVisibilityRequest {
    #[serde(rename = "visibility")]
    pub is_public: bool,
}

/// Convert the JSONB parts payload to a Vec of UIMessagePart.
fn convert_parts_to_ui(parts: &serde_json::Value) -> Vec<UIMessagePart> {
    // Try deserializing as an array of UI parts first
    if let Ok(ui_parts) = serde_json::from_value::<Vec<UIMessagePart>>(parts.clone()) {
        return ui_parts;
    }

    // Try deserializing as a single object that could be a UI part
    if let Ok(single_part) = serde_json::from_value::<UIMessagePart>(parts.clone()) {
        return vec![single_part];
    }

    // Fallback: wrap the entire JSON value as a text part
    vec![crate::handlers::chats::ui_messages::TextUIPart {
        type_: "text".to_string(),
        text: serde_json::to_string(parts).unwrap_or_default(),
        state: Some(crate::handlers::chats::ui_messages::TextState::Done),
    }]
}
