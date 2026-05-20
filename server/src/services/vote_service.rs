use crate::errors::AppError;
use crate::infra::repositories::vote_repository::{NewVoteDb, VoteDb};
use deadpool_diesel::postgres::Pool;

#[derive(Clone)]
pub struct VoteService {
    pool: Pool,
}

impl VoteService {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// Get all votes for a specific chat.
    pub async fn get_votes_for_chat(
        &self,
        chat_id: uuid::Uuid,
    ) -> Result<Vec<VoteDb>, AppError> {
        vote_repository::get_votes_for_chat(&self.pool, chat_id)
            .map_err(|_| AppError::InternalServerError)
    }

    /// Insert or update a vote for a message in a chat.
    pub async fn upsert_vote(
        &self,
        new_vote: NewVoteDb,
    ) -> Result<VoteDb, AppError> {
        vote_repository::upsert_vote(&self.pool, new_vote)
            .map_err(|_| AppError::InternalServerError)
    }

    /// Delete a vote for a specific message in a chat.
    pub async fn delete_vote(
        &self,
        chat_id: uuid::Uuid,
        message_id: uuid::Uuid,
    ) -> Result<usize, AppError> {
        vote_repository::delete_vote(&self.pool, chat_id, message_id)
            .map_err(|_| AppError::InternalServerError)
    }
}
