use crate::domain::models::messages::Message;
use crate::errors::AppError;
use crate::infra::repositories::message_repository;
use crate::infra::repositories::message_repository::{
    message_role_to_db_role, MessageDb, NewMessageDb,
};
use crate::operation_result::OperationResult;
use deadpool_diesel::postgres::Pool;
use tracing::{info, instrument};

#[derive(Clone)]
pub struct MessageService {
    pool: Pool,
}

// TODO: Full review, has some shit
impl MessageService {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Get all messages for a specific chat, ordered by created_at ascending.
    pub async fn get_messages_for_chat(
        &self,
        chat_id: uuid::Uuid,
    ) -> Result<Vec<MessageDb>, AppError> {
        message_repository::get_messages_for_chat(&self.pool, chat_id)
            .await
            .map_err(|_| AppError::InternalServerError)
    }

    /// Insert a new message and return it.
    pub async fn insert(&self, new_message: NewMessageDb) -> Result<MessageDb, AppError> {
        message_repository::insert(&self.pool, new_message)
            .await
            .map_err(|_| AppError::InternalServerError)
    }

    /// Insert a domain Message and return the DB row.
    pub async fn insert_message(&self, message: &Message) -> Result<MessageDb, AppError> {
        let parts_json = serde_json::to_value(&message.parts).unwrap_or_default();
        let role = message_role_to_db_role(&message.role);

        self.insert(NewMessageDb {
            chat_id: message.chat_id.0,
            role: role.into(),
            parts: parts_json,
        })
        .await
    }

    /// Delete all messages created after the given message_id for a specific chat.
    /// Used for truncating history when user edits or regenerates a message.
    #[instrument(skip(self))]
    pub async fn delete_after_message(
        &self,
        chat_id: uuid::Uuid,
        target_message_id: uuid::Uuid,
    ) -> Result<usize, AppError> {
        let result =
            message_repository::delete_after_message(&self.pool, chat_id, target_message_id)
                .await
                .map_err(|_| AppError::InternalServerError);
        match &result {
            Ok(count) => info!(deleted = count, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }
}
