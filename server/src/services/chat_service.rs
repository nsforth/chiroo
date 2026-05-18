use crate::domain::models::chat::Chat;
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

    pub async fn get_chats(&self) -> Result<Vec<Chat>, AppError> {
        let r = chat_repository::get_all(&self.pool).await;
        match r {
            Ok(chats) => Ok(chats),
            Err(_) => Err(AppError::InternalServerError),
        }
    }

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
}
