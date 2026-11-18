use chrono::NaiveDateTime;

use crate::domain::models::chat::ChatId;
use crate::domain::models::scenario::ScenarioId;
use crate::domain::models::user::User;

#[derive(Debug)]
pub struct Vote {
    pub chat_id: ChatId,
    pub scenario_id: ScenarioId,
    pub vote_type: VoteType,
}

#[derive(Debug)]
pub enum VoteType {
    Like,
    Dislike,
}

/// A vote with surrounding user/chat/scenario context for the admin view.
#[derive(Debug)]
pub struct VoteWithChat {
    pub user: User,
    pub chat_id: ChatId,
    pub scenario_id: ScenarioId,
    pub is_upvoted: bool,
    pub voted_at: NaiveDateTime,
    pub chat_title: Option<String>,
    pub chat_created_at: NaiveDateTime,
    pub model: String,
}
