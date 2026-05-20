use axum::extract::State;
use axum::http::StatusCode;
use axum::response::ErrorResponse;
use axum::Json;
use uuid::Uuid;

use crate::utils::PathExtractor;
use crate::utils::JsonExtractor;
use crate::AppState;

/// Request body for upserting a vote.
/// Frontend sends: { messageId, isUpvoted? } where isUpvoted can be null to remove vote.
#[derive(serde::Deserialize)]
pub struct VoteUpsertRequest {
    #[serde(rename = "messageId")]
    pub message_id: String,
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

/// Get all votes for a specific chat.
/// Frontend expects: direct array of Vote objects.
pub async fn get_votes(
    State(state): State<AppState>,
    PathExtractor(chat_id): PathExtractor<Uuid>,
) -> Result<Json<Vec<VoteResponse>>, ErrorResponse> {
    let db_votes = state.vote_service.get_votes_for_chat(chat_id).await?;

    let votes: Vec<VoteResponse> = db_votes
        .into_iter()
        .map(|v| VoteResponse {
            chat_id: v.chat_id.to_string(),
            message_id: v.message_id.to_string(),
            is_upvoted: v.is_upvoted,
        })
        .collect();

    Ok(Json(votes))
}

/// Insert or update a vote for a message in a chat.
/// If isUpvoted is None/null, delete the vote.
pub async fn upsert_vote(
    State(state): State<AppState>,
    PathExtractor(chat_id): PathExtractor<Uuid>,
    JsonExtractor(body): JsonExtractor<VoteUpsertRequest>,
) -> Result<impl axum::response::IntoResponse, ErrorResponse> {
    let message_id = Uuid::parse_str(&body.message_id).map_err(|_| {
        (axum::http::StatusCode::BAD_REQUEST, "Invalid message ID").into_response()
    })?;

    // If is_upvoted is None/null, delete the vote
    if body.is_upvoted.is_none() {
        return match state.vote_service.delete_vote(chat_id, message_id).await {
            Ok(_) => Ok(axum::http::StatusCode::NO_CONTENT),
            Err(_) => Err((axum::http::StatusCode::NOT_FOUND, "Vote not found").into_response()),
        };
    }

    let new_vote = crate::infra::repositories::vote_repository::NewVoteDb {
        chat_id,
        message_id,
        is_upvoted: body.is_upvoted.unwrap(),
    };

    let result = state.vote_service.upsert_vote(new_vote).await?;

    Ok(Json(VoteResponse {
        chat_id: result.chat_id.to_string(),
        message_id: result.message_id.to_string(),
        is_upvoted: result.is_upvoted,
    }))
}
