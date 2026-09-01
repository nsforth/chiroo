use crate::domain::models::chat::ChatId;
use crate::domain::models::scenario::ScenarioId;
use crate::domain::models::votes::Vote;
use crate::errors::AppError;
use crate::infra::repositories::vote_repository::{self, NewVoteDb};
use crate::operation_result::OperationResult;
use deadpool_diesel::postgres::Pool;
use tracing::{info, instrument};

#[derive(Clone)]
pub struct VoteService {
    pool: Pool,
}

impl VoteService {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    #[instrument(skip(self))]
    pub async fn get_votes_for_chat(&self, chat_id: ChatId) -> Result<Vec<Vote>, AppError> {
        let result: Result<Vec<Vote>, AppError> =
            vote_repository::get_votes_for_chat(&self.pool, chat_id)
                .await
                .map(|votes| votes.iter().map(Into::into).collect())
                .map_err(|_| AppError::InternalServerError);
        match &result {
            Ok(votes) => info!(count = votes.len(), status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    #[instrument(skip(self))]
    pub async fn like(&self, chat_id: ChatId, scenario_id: ScenarioId) -> Result<Vote, AppError> {
        let result: Result<Vote, AppError> = vote_repository::upsert_vote(
            &self.pool,
            NewVoteDb {
                chat_id: chat_id.0,
                scenario_id: scenario_id.0,
                is_upvoted: true,
            },
        )
        .await
        .map(Into::into)
        .map_err(|_| AppError::InternalServerError);
        match &result {
            Ok(vote) => info!(vote_type = ?vote.vote_type, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    #[instrument(skip(self))]
    pub async fn dislike(
        &self,
        chat_id: ChatId,
        scenario_id: ScenarioId,
    ) -> Result<Vote, AppError> {
        let result: Result<Vote, AppError> = vote_repository::upsert_vote(
            &self.pool,
            NewVoteDb {
                chat_id: chat_id.0,
                scenario_id: scenario_id.0,
                is_upvoted: false,
            },
        )
        .await
        .map(Into::into)
        .map_err(|_| AppError::InternalServerError);
        match &result {
            Ok(vote) => info!(vote_type = ?vote.vote_type, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    #[instrument(skip(self))]
    pub async fn unvote(&self, chat_id: ChatId, scenario_id: ScenarioId) -> Result<bool, AppError> {
        let result = vote_repository::delete_vote(&self.pool, chat_id.0, scenario_id.0)
            .await
            .map_err(|_| AppError::InternalServerError);
        match &result {
            Ok(deleted) => info!(deleted = deleted, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }
}
