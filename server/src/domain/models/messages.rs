use crate::domain::models::chat::ChatId;
use crate::domain::models::votes::Vote;
use chrono::NaiveDateTime;

pub struct Messages {
    pub chat_id: ChatId,
    pub messages: Vec<Message>,
}

pub enum Message {
    User(UserMessage),
    Assistant(AssistantMessage),
}

pub struct UserMessage {
    pub text: String,
    pub created_at: NaiveDateTime,
}

pub struct AssistantMessage {
    pub parts: Vec<AssistantParts>,
    pub vote: Option<Vote>,
    pub created_at: NaiveDateTime,
}

pub enum AssistantParts {
    AssistantText(AssistantText),
    AssistantReasoning(AssistantReasoning),
}

pub struct AssistantText {
    text: String,
    pub created_at: NaiveDateTime,
}

pub struct AssistantReasoning {
    reasoning: String,
    pub created_at: NaiveDateTime,
}
