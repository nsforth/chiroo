use crate::domain::models::{chat::ChatId, messages::MessageId};

pub struct Vote {
    pub chat_id: ChatId,
    pub message_id: MessageId,
    pub vote_type: VoteType,
}

pub enum VoteType {
    Like,
    Dislike,
}
