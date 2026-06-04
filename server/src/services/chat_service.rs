use crate::domain::models::chat::{Chat, ChatId};
use crate::domain::models::messages::{Message, MessagePart, Role, TextBlock};
use crate::domain::models::user::UserId;
use crate::errors::AppError;
use crate::infra::repositories::chat_repository;
use crate::services::message_service::MessageService;
use deadpool_diesel::postgres::Pool;

#[derive(Clone)]
pub struct ChatService {
    pool: Pool,
    message_service: MessageService,
}

// TODO: Full review, has some shit
impl ChatService {
    pub fn new(pool: Pool, message_service: MessageService) -> Self {
        Self {
            pool,
            message_service,
        }
    }

    /// Get all chats (public and owned by any user).
    pub async fn get_chats(&self, user_id: UserId) -> Result<Vec<Chat>, AppError> {
        let r = chat_repository::get_all(&self.pool, user_id).await;
        match r {
            Ok(chats) => Ok(chats.into_iter().map(Into::into).collect()),
            Err(_) => Err(AppError::InternalServerError),
        }
    }

    /// Get a single chat by ID.
    pub async fn get_chat(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<Option<Chat>, AppError> {
        let r = chat_repository::get_by_id(&self.pool, chat_id, user_id).await;
        match r {
            Ok(chat) => Ok(chat.map(Into::into)),
            Err(_) => Err(AppError::InternalServerError),
        }
    }

    /// Create a new chat with an optional first message.
    /// The chat title is derived from the first message (first 15 chars + "...").
    pub async fn create_chat(
        &self,
        first_message: Option<String>,
        user_id: UserId,
    ) -> Result<Chat, AppError> {
        let title = first_message.as_deref().map(Message::make_title);

        let new_chat = chat_repository::NewChatDb {
            owner: user_id.0,
            title,
        };

        let chat: Chat = chat_repository::insert(&self.pool, new_chat)
            .await
            .map(Into::into)
            .map_err(|_| AppError::InternalServerError)?;

        if let Some(text) = first_message {
            let mut message = Message::new(ChatId(chat.id.0), Role::User);
            message.parts.push(MessagePart::Text(TextBlock { text }));
            let _ = self.message_service.insert_message(&message).await;
        }

        Ok(chat)
    }

    /// Delete a chat by ID (only owner can delete).
    pub async fn delete_chat(&self, chat_id: &ChatId, user_id: &UserId) -> Result<(), AppError> {
        match chat_repository::delete(&self.pool, *chat_id, *user_id).await {
            Ok(false) => Err(AppError::NotFound("Chat not found".into())),
            Ok(true) => Ok(()),
            Err(_) => Err(AppError::InternalServerError),
        }
    }

    /// Update chat title.
    pub async fn update_title(
        &self,
        chat_id: &ChatId,
        user_id: &UserId,
        title: String,
    ) -> Result<Chat, AppError> {
        match chat_repository::update_title(&self.pool, *chat_id, *user_id, title).await {
            Ok(chat) => Ok(chat.into()),
            Err(crate::infra::errors::InfraError::NotFound) => {
                Err(AppError::NotFound("Chat not found".into()))
            },
            Err(_) => Err(AppError::InternalServerError),
        }
    }

    /// Verify that the user owns the chat. Returns `NotFound` if not.
    pub async fn check_ownership(
        &self,
        chat_id: ChatId,
        user_id: UserId,
    ) -> Result<(), AppError> {
        match chat_repository::is_owner(&self.pool, chat_id, user_id).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(AppError::NotFound("Chat not found".into())),
            Err(_) => Err(AppError::InternalServerError),
        }
    }

    /// Update chat visibility (public/private).
    pub async fn update_visibility(
        &self,
        chat_id: &ChatId,
        user_id: &UserId,
        is_public: bool,
    ) -> Result<Chat, AppError> {
        match chat_repository::update_visibility(&self.pool, *chat_id, *user_id, is_public).await {
            Ok(chat) => Ok(chat.into()),
            Err(crate::infra::errors::InfraError::NotFound) => {
                Err(AppError::NotFound("Chat not found".into()))
            },
            Err(_) => Err(AppError::InternalServerError),
        }
    }
}
