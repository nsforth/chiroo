use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{ErrorResponse, IntoResponse};
use axum::Json;
use uuid::Uuid;

use crate::domain::models::chat::ChatId;
use crate::domain::models::messages::MessageId;
use crate::domain::models::votes::VoteType;
use crate::utils::JsonExtractor;
use crate::utils::PathExtractor;
use crate::AppState;

/// Request body for upserting a vote.
/// Frontend sends: { messageId, isUpvoted? } where isUpvoted can be null to remove vote.
#[derive(serde::Deserialize)]
pub struct VoteRequest {
    #[serde(rename = "messageId")]
    pub message_id: Uuid,
    #[serde(rename = "isUpvoted", default)]
    pub is_upvoted: Option<bool>,
}

/// Response for a single vote.
#[derive(serde::Serialize)]
pub struct VoteResponse {
    #[serde(rename = "chatId")]
    pub chat_id: String,
    #[serde(rename = "messageId")]
    pub message_id: String,
    #[serde(rename = "isUpvoted")]
    pub is_upvoted: bool,
}

pub async fn get_votes(
    State(state): State<AppState>,
    PathExtractor(chat_id): PathExtractor<Uuid>,
) -> Result<Json<Vec<VoteResponse>>, ErrorResponse> {
    let db_votes = state
        .vote_service
        .get_votes_for_chat(ChatId(chat_id))
        .await?;

    let votes: Vec<VoteResponse> = db_votes
        .into_iter()
        .map(|v| VoteResponse {
            chat_id: v.chat_id.0.to_string(),
            message_id: v.message_id.0.to_string(),
            is_upvoted: match v.vote_type {
                VoteType::Like => true,
                VoteType::Dislike => false,
            },
        })
        .collect();

    Ok(Json(votes))
}

pub async fn vote(
    State(state): State<AppState>,
    PathExtractor(chat_id): PathExtractor<ChatId>,
    JsonExtractor(body): JsonExtractor<VoteRequest>,
) -> Result<StatusCode, ErrorResponse> {
    let message_id = MessageId(body.message_id.clone());

    if let Some(is_upvoted) = body.is_upvoted {
        if is_upvoted {
            state.vote_service.like(chat_id, message_id).await?;
        } else {
            state.vote_service.dislike(chat_id, message_id).await?;
        }
    } else {
        state.vote_service.unvote(chat_id, message_id).await?;
    }

    Ok(StatusCode::OK)
}
