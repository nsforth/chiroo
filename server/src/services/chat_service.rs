use crate::domain::models::chat::{Chat, ChatId};
use crate::domain::models::messages::Message;
use crate::domain::models::scenario::{Scenario, ScenarioStatus};
use crate::domain::models::user::UserId;
use crate::errors::AppError;
use crate::infra::repositories::chat_repository;
use crate::operation_result::OperationResult;
use crate::services::scenario_service::ScenarioService;
use deadpool_diesel::postgres::Pool;
use tracing::{info, instrument};

#[derive(Clone)]
pub struct ChatService {
    pool: Pool,
    scenario_service: ScenarioService,
}

impl ChatService {
    pub fn new(pool: Pool, scenario_service: ScenarioService) -> Self {
        Self {
            pool,
            scenario_service,
        }
    }

    /// Get all chats owned by the user.
    #[instrument(skip(self))]
    pub async fn get_chats(&self, user_id: UserId) -> Result<Vec<Chat>, AppError> {
        let r = chat_repository::get_all(&self.pool, user_id).await;
        let result = match r {
            Ok(chats) => Ok(chats.into_iter().map(Into::into).collect::<Vec<Chat>>()),
            Err(_) => Err(AppError::InternalServerError),
        };
        match &result {
            Ok(chats) => info!(chats_len = chats.len(), status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    /// Get a single chat by ID.
    #[instrument(skip(self))]
    pub async fn get_chat(&self, chat_id: ChatId, user_id: UserId) -> Result<Chat, AppError> {
        let result: Result<Chat, AppError> =
            match chat_repository::get_by_id(&self.pool, chat_id, user_id).await {
                Ok(Some(chat)) => Ok(chat.into()),
                Ok(None) => Err(AppError::NotFound("Chat not found".into())),
                Err(_) => Err(AppError::InternalServerError),
            };
        match &result {
            Ok(_) => info!(status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    /// Create a new chat with an optional first message.
    /// The chat title is derived from the first message (first 15 chars + "...").
    #[instrument(skip(self))]
    pub async fn create_chat(
        &self,
        first_message: Option<String>,
        model: String,
        user_id: UserId,
    ) -> Result<Chat, AppError> {
        let title = first_message.as_deref().map(Message::make_title);
        let new_chat = chat_repository::NewChatDb::new(user_id.0, title.clone());
        let result: Result<Chat, AppError> = chat_repository::insert(&self.pool, new_chat)
            .await
            .map(Into::into)
            .map_err(|_| AppError::InternalServerError);
        let result = match result {
            Ok(chat) => {
                if let Some(text) = first_message {
                    let user_message = Message::user(chat.id, text);
                    let scenario =
                        Scenario::new(chat.id, model, ScenarioStatus::Pending, user_message);
                    let _ = self.scenario_service.insert_scenario(&scenario).await;
                }
                Ok(chat)
            },
            Err(e) => Err(e),
        };
        match &result {
            Ok(chat) => info!(title = ?chat.title, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    /// Delete a chat by ID (only owner can delete).
    #[instrument(skip(self))]
    pub async fn delete_chat(&self, chat_id: &ChatId, user_id: &UserId) -> Result<(), AppError> {
        let result = match chat_repository::delete(&self.pool, *chat_id, *user_id).await {
            Ok(false) => Err(AppError::NotFound("Chat not found".into())),
            Ok(true) => Ok(()),
            Err(_) => Err(AppError::InternalServerError),
        };
        match &result {
            Ok(()) => info!(status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    /// Update chat title.
    #[instrument(skip(self))]
    pub async fn update_title(
        &self,
        chat_id: &ChatId,
        user_id: &UserId,
        title: String,
    ) -> Result<Chat, AppError> {
        let result: Result<Chat, AppError> = match chat_repository::update_title(
            &self.pool,
            *chat_id,
            *user_id,
            title.clone(),
        )
        .await
        {
            Ok(chat) => Ok(chat.into()),
            Err(crate::infra::errors::InfraError::NotFound) => {
                Err(AppError::NotFound("Chat not found".into()))
            },
            Err(_) => Err(AppError::InternalServerError),
        };
        match &result {
            Ok(chat) => info!(title = ?chat.title, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    /// Verify that the user owns the chat. Returns `NotFound` if not.
    #[instrument(skip(self))]
    pub async fn check_ownership(&self, chat_id: ChatId, user_id: UserId) -> Result<(), AppError> {
        let result = match chat_repository::is_owner(&self.pool, chat_id, user_id).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(AppError::NotFound("Chat not found".into())),
            Err(_) => Err(AppError::InternalServerError),
        };
        match &result {
            Ok(()) => info!(is_owner = true, status = result.status()),
            Err(_e) => info!(is_owner = false, status = result.status()),
        }
        result
    }

    /// Update chat visibility (public/private).
    #[instrument(skip(self))]
    pub async fn update_visibility(
        &self,
        chat_id: &ChatId,
        user_id: &UserId,
        is_public: bool,
    ) -> Result<Chat, AppError> {
        let result: Result<Chat, AppError> =
            match chat_repository::update_visibility(&self.pool, *chat_id, *user_id, is_public)
                .await
            {
                Ok(chat) => Ok(chat.into()),
                Err(crate::infra::errors::InfraError::NotFound) => {
                    Err(AppError::NotFound("Chat not found".into()))
                },
                Err(_) => Err(AppError::InternalServerError),
            };
        match &result {
            Ok(chat) => info!(is_public = chat.public, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }
}
