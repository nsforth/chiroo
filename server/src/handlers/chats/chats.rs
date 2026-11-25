use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{ErrorResponse, IntoResponse};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::models::auth_user::AuthUser;
use crate::domain::models::chat::{Chat, ChatId};
use crate::domain::models::chat_search_config::ChatSearchConfig;
use crate::domain::models::scenario::Scenario;
use crate::domain::models::user::UserId;
use crate::errors::AppError;
use crate::handlers::chats::ui_messages::{scenario_to_ui_messages, UIMessage};
use crate::utils::JsonExtractor;
use crate::utils::PathExtractor;
use crate::AppState;

#[derive(Debug, Clone, Deserialize)]
pub struct CreateChatRequest {
    #[serde(rename = "firstMessage")]
    pub first_message: Option<String>,
    pub model: String,
    #[serde(rename = "searchConfig")]
    pub search_config: Option<ChatSearchConfig>,
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
    pub model: String,
    pub messages: Vec<UIMessage>,
    #[serde(rename = "isOwner")]
    pub is_owner: bool,
}

impl ChatDetailResponse {
    fn new(chat: &Chat, viewer_id: UserId, scenarios: Vec<Scenario>) -> Result<Self, AppError> {
        let model = scenarios
            .last()
            .map(|s| s.name.clone())
            .ok_or(AppError::InternalServerError)?;

        let mut messages = Vec::new();
        for scenario in &scenarios {
            messages.extend(scenario_to_ui_messages(scenario)?);
        }

        Ok(Self {
            id: chat.id.0.to_string(),
            user_id: chat.owner.to_string(),
            title: chat.title.clone(),
            visibility: if chat.public { "public" } else { "private" }.to_string(),
            created_at: chat.created_at.and_utc().to_rfc3339(),
            model,
            messages,
            is_owner: chat.owner == viewer_id,
        })
    }
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
    // Validate the per-chat search overrides up front so an invalid value
    // (e.g. `limit: 0`, `rerankerThreshold: 1.5`) is rejected with 400 at
    // creation instead of failing on every subsequent message.
    if let Some(ref cfg) = new_chat.search_config {
        cfg.resolve(&state.ai_service.search_params())?;
    }
    let chat = state
        .chat_service
        .create_chat(
            new_chat.first_message,
            new_chat.model,
            auth_user.user_id,
            new_chat.search_config,
        )
        .await?;
    Ok(Json(From::from(&chat)))
}

pub async fn get_chat(
    State(state): State<AppState>,
    auth_user: AuthUser,
    PathExtractor(id): PathExtractor<Uuid>,
) -> Result<Json<ChatDetailResponse>, ErrorResponse> {
    let chat_id = ChatId(id);

    let chat = state
        .chat_service
        .get_chat(chat_id, auth_user.user_id)
        .await?;

    let scenarios = state
        .scenario_service
        .get_scenarios_for_chat(chat_id)
        .await?;

    Ok(Json(ChatDetailResponse::new(
        &chat,
        auth_user.user_id,
        scenarios,
    )?))
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
    let is_public = matches!(body.visibility, ChatVisibility::Public);
    match state
        .chat_service
        .update_visibility(&chat_id, &user_id, is_public)
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
#[serde(rename_all = "lowercase")]
pub enum ChatVisibility {
    Public,
    Private,
}

#[derive(serde::Deserialize)]
pub struct UpdateVisibilityRequest {
    pub visibility: ChatVisibility,
}
