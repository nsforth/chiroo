use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{ErrorResponse, IntoResponse};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::models::chat::Chat;
use crate::handlers::chats::ui_messages::{MessageRole, UIMessage, UIMessagePart};
use crate::domain::models::auth_user::AuthUser;
use crate::utils::JsonExtractor;
use crate::utils::PathExtractor;
use crate::AppState;

#[derive(Debug, Clone, Deserialize)]
pub struct CreateChatRequest {
    #[serde(rename = "firstMessage")]
    pub first_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatsResponse {
    chats: Vec<ChatResponse>,
}

impl From<Vec<Chat>> for ChatsResponse {
    fn from(value: Vec<Chat>) -> Self {
        let chats_responses = value.iter().map(From::from).collect();
        ChatsResponse {
            chats: chats_responses,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatResponse {
    pub id: String,
    pub title: Option<String>,
    #[serde(rename = "userId")]
    pub user_id: String,
    pub visibility: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

impl From<&Chat> for ChatResponse {
    fn from(value: &Chat) -> Self {
        let visibility = if value.public {
            String::from("public")
        } else {
            String::from("private")
        };
        Self {
            id: value.id.0.to_string(),
            title: value.title.clone(),
            user_id: value.owner.to_string(),
            visibility,
            created_at: value.created_at.and_utc().to_rfc3339(),
        }
    }
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

pub async fn get_chats(
    State(state): State<AppState>,
    auth_user: AuthUser,
) -> Result<Json<ChatsResponse>, ErrorResponse> {
    let chats = state.chat_service.get_chats(auth_user.user_id).await?;
    Ok(Json(From::from(chats)))
}

pub async fn create_chat(
    State(state): State<AppState>,
    auth_user: AuthUser,
    JsonExtractor(new_chat): JsonExtractor<CreateChatRequest>,
) -> Result<Json<ChatResponse>, ErrorResponse> {
    let chat = state
        .chat_service
        .create_chat(new_chat.first_message, auth_user.user_id)
        .await?;
    Ok(Json(From::from(&chat)))
}

pub async fn get_chat(
    State(state): State<AppState>,
    auth_user: AuthUser,
    PathExtractor(id): PathExtractor<Uuid>,
) -> Result<Json<ChatDetailResponse>, ErrorResponse> {
    let chat_id = crate::domain::models::chat::ChatId(id);

    let chat = match state
        .chat_service
        .get_chat(chat_id, auth_user.user_id)
        .await?
    {
        Some(chat) => chat,
        None => {
            return Err((StatusCode::NOT_FOUND, "Chat not found")
                .into_response()
                .into())
        },
    };

    let db_messages = state.message_service.get_messages_for_chat(id).await?;

    // TODO: Make convenient converter
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

    let is_owner = chat.owner == auth_user.user_id;

    // TODO: Move to converter
    let visibility = if chat.public {
        "public".to_string()
    } else {
        "private".to_string()
    };

    Ok(Json(ChatDetailResponse {
        id: chat.id.0.to_string(),
        user_id: chat.owner.to_string(),
        title: chat.title,
        visibility,
        created_at: chat.created_at.and_utc().to_rfc3339(),
        messages,
        is_owner,
    }))
}

pub async fn delete_chat(
    State(state): State<AppState>,
    auth_user: AuthUser,
    PathExtractor(id): PathExtractor<Uuid>,
) -> Result<StatusCode, ErrorResponse> {
    let user_id = auth_user.user_id;
    let chat_id = crate::domain::models::chat::ChatId(id);
    match state.chat_service.delete_chat(&chat_id, &user_id).await {
        Ok(()) => Ok(StatusCode::NO_CONTENT),
        Err(_) => Err((StatusCode::NOT_FOUND, "Chat not found")
            .into_response()
            .into()),
    }
}

pub async fn update_title(
    State(state): State<AppState>,
    auth_user: AuthUser,
    PathExtractor(id): PathExtractor<Uuid>,
    JsonExtractor(body): JsonExtractor<UpdateTitleRequest>,
) -> Result<Json<ChatResponse>, ErrorResponse> {
    let user_id = auth_user.user_id;
    let chat_id = crate::domain::models::chat::ChatId(id);
    match state
        .chat_service
        .update_title(&chat_id, &user_id, body.title)
        .await?
    {
        chat => Ok(Json(From::from(&chat))),
    }
}

pub async fn update_visibility(
    State(state): State<AppState>,
    auth_user: AuthUser,
    PathExtractor(id): PathExtractor<Uuid>,
    JsonExtractor(body): JsonExtractor<UpdateVisibilityRequest>,
) -> Result<Json<ChatResponse>, ErrorResponse> {
    let user_id = auth_user.user_id;
    let chat_id = crate::domain::models::chat::ChatId(id);
    match state
        .chat_service
        .update_visibility(&chat_id, &user_id, body.is_public)
        .await?
    {
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

// TODO: Investigate what is this AI generated shit
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
    vec![UIMessagePart::Text(
        crate::handlers::chats::ui_messages::TextUIPart {
            text: serde_json::to_string(parts).unwrap_or_default(),
            state: Some(crate::handlers::chats::ui_messages::TextState::Done),
        },
    )]
}
