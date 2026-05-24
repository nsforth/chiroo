use crate::domain::models::chat::ChatId;
use crate::domain::models::messages::MessageId;
use crate::domain::models::votes::Vote;
use crate::errors::AppError;
use crate::infra::repositories::vote_repository::{self, NewVoteDb};
use deadpool_diesel::postgres::Pool;

#[derive(Clone)]
pub struct VoteService {
    pool: Pool,
}

impl VoteService {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    pub async fn get_votes_for_chat(&self, chat_id: ChatId) -> Result<Vec<Vote>, AppError> {
        vote_repository::get_votes_for_chat(&self.pool, chat_id)
            .await
            .map(|votes| votes.iter().map(Into::into).collect())
            .map_err(|_| AppError::InternalServerError)
    }

    pub async fn like(&self, chat_id: ChatId, message_id: MessageId) -> Result<Vote, AppError> {
        vote_repository::upsert_vote(
            &self.pool,
            NewVoteDb {
                chat_id: chat_id.0,
                message_id: message_id.0,
                is_upvoted: true,
            },
        )
        .await
        .map(Into::into)
        .map_err(|_| AppError::InternalServerError)
    }

    pub async fn dislike(&self, chat_id: ChatId, message_id: MessageId) -> Result<Vote, AppError> {
        vote_repository::upsert_vote(
            &self.pool,
            NewVoteDb {
                chat_id: chat_id.0,
                message_id: message_id.0,
                is_upvoted: false,
            },
        )
        .await
        .map(Into::into)
        .map_err(|_| AppError::InternalServerError)
    }

    pub async fn unvote(&self, chat_id: ChatId, message_id: MessageId) -> Result<bool, AppError> {
        vote_repository::delete_vote(&self.pool, chat_id.0, message_id.0)
            .await
            .map_err(|_| AppError::InternalServerError)
    }
}
