use crate::domain::models::{chat::ChatId, scenario::ScenarioId};

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
