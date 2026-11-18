use axum::extract::{Query, State};
use axum::response::ErrorResponse;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::domain::models::votes::VoteWithChat;
use crate::errors::AppError;
use crate::handlers::user::PublicUserResponse;
use crate::AppState;

fn default_page() -> u32 {
    1
}

fn default_page_size() -> u32 {
    25
}

#[derive(Deserialize)]
pub struct ListVotesQuery {
    #[serde(default = "default_page")]
    pub page: u32,
    #[serde(rename = "pageSize", default = "default_page_size")]
    pub page_size: u32,
}

#[derive(Serialize)]
pub struct VoteAdminResponse {
    pub user: PublicUserResponse,
    #[serde(rename = "chatId")]
    pub chat_id: String,
    #[serde(rename = "scenarioId")]
    pub scenario_id: String,
    #[serde(rename = "chatTitle")]
    pub chat_title: Option<String>,
    #[serde(rename = "votedAt")]
    pub voted_at: String,
    #[serde(rename = "chatCreatedAt")]
    pub chat_created_at: String,
    pub model: String,
    #[serde(rename = "isUpvoted")]
    pub is_upvoted: bool,
}

impl From<VoteWithChat> for VoteAdminResponse {
    fn from(vote: VoteWithChat) -> Self {
        let VoteWithChat {
            user,
            chat_id,
            scenario_id,
            is_upvoted,
            chat_title,
            chat_created_at,
            model,
            voted_at,
        } = vote;
        Self {
            user: PublicUserResponse {
                id: user.id.0.to_string(),
                name: user.name.unwrap_or_default(),
                username: user.username,
                avatar: user.avatar_url,
                admin: user.admin,
            },
            chat_id: chat_id.0.to_string(),
            scenario_id: scenario_id.0.to_string(),
            voted_at: voted_at.and_utc().to_rfc3339(),
            chat_title,
            chat_created_at: chat_created_at.and_utc().to_rfc3339(),
            model,
            is_upvoted,
        }
    }
}

#[derive(Serialize)]
pub struct VotePageResponse {
    pub items: Vec<VoteAdminResponse>,
    pub total: i64,
}

pub async fn list_votes(
    State(state): State<AppState>,
    Query(query): Query<ListVotesQuery>,
) -> Result<Json<VotePageResponse>, ErrorResponse> {
    if query.page < 1 {
        return Err(AppError::BadRequest("page must be >= 1".to_string()).into());
    }

    let (votes, total) = state
        .vote_service
        .list_all(query.page, query.page_size)
        .await?;
    let items = votes.into_iter().map(VoteAdminResponse::from).collect();
    Ok(Json(VotePageResponse { items, total }))
}
