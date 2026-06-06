use crate::domain::models::{chat::ChatId, messages::MessageId};

#[derive(Debug)]
pub struct Vote {
    pub chat_id: ChatId,
    pub message_id: MessageId,
    pub vote_type: VoteType,
}

#[derive(Debug)]
pub enum VoteType {
    Like,
    Dislike,
}
