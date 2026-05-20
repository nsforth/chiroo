use crate::domain::models::chat::{Chat, ChatId};
use crate::domain::models::user::UserId;
use crate::errors::AppError;
use crate::infra::repositories::chat_repository;
use deadpool_diesel::postgres::Pool;

#[derive(Clone)]
pub struct ChatService {
    pool: Pool,
}

impl ChatService {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Get all chats (public and owned by any user).
    pub async fn get_chats(&self) -> Result<Vec<Chat>, AppError> {
        let r = chat_repository::get_all(&self.pool).await;
        match r {
            Ok(chats) => Ok(chats),
            Err(_) => Err(AppError::InternalServerError),
        }
    }

    /// Get a single chat by ID.
    pub async fn get_chat(&self, chat_id: &ChatId) -> Result<Option<Chat>, AppError> {
        let r = chat_repository::get_by_id(&self.pool, chat_id).await;
        match r {
            Ok(chat) => Ok(chat),
            Err(_) => Err(AppError::InternalServerError),
        }
    }

    /// Create a new chat for a user.
    pub async fn create_chat(
        &self,
        title: Option<String>,
        user_id: UserId,
    ) -> Result<Chat, AppError> {
        let new_chat = chat_repository::NewChatDb {
            owner: user_id.0,
            title,
        };
        match chat_repository::insert(&self.pool, new_chat).await {
            Ok(chat) => Ok(chat),
            Err(_) => Err(AppError::InternalServerError),
        }
    }

    /// Delete a chat by ID (only owner can delete).
    pub async fn delete_chat(
        &self,
        chat_id: &ChatId,
        user_id: &UserId,
    ) -> Result<(), AppError> {
        match chat_repository::delete(&self.pool, chat_id, user_id).await {
            Ok(_) => Ok(()),
            Err(crate::infra::errors::InfraError::NotFound) => {
                Err(AppError::InternalServerError)
            }
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
        match chat_repository::update_title(&self.pool, chat_id, user_id, &title).await {
            Ok(chat) => Ok(chat),
            Err(crate::infra::errors::InfraError::NotFound) => {
                Err(AppError::InternalServerError)
            }
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
        match chat_repository::update_visibility(&self.pool, chat_id, user_id, is_public).await {
            Ok(chat) => Ok(chat),
            Err(crate::infra::errors::InfraError::NotFound) => {
                Err(AppError::InternalServerError)
            }
            Err(_) => Err(AppError::InternalServerError),
        }
    }
}
