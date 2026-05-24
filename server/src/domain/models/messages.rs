use crate::domain::models::chat::ChatId;
use crate::domain::models::votes::Vote;
use chrono::NaiveDateTime;
use serde::Deserialize;
use uuid::Uuid;

pub struct Messages {
    pub chat_id: ChatId,
    pub messages: Vec<Message>,
}

#[derive(Clone, Copy, Deserialize)]
pub struct MessageId(pub Uuid);

pub struct Message {
    message_id: MessageId,
    created_at: NaiveDateTime,
    message: MessageType,
}

pub enum MessageType {
    User(UserMessage),
    Assistant(AssistantMessage),
}

pub struct UserMessage {
    pub text: String,
}

pub struct AssistantMessage {
    pub parts: Vec<AssistantParts>,
    pub vote: Option<Vote>,
}

pub enum AssistantParts {
    AssistantText(AssistantText),
    AssistantReasoning(AssistantReasoning),
}

pub struct AssistantText {
    text: String,
}

pub struct AssistantReasoning {
    reasoning: String,
}
