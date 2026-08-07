use crate::domain::models::chat::ChatId;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

const MAX_TITLE_CHARS: usize = 15;

#[derive(Clone, Copy, Debug)]
pub struct MessageId(pub Uuid);

impl MessageId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    pub id: MessageId,
    pub chat_id: ChatId,
    pub role: Role,
    pub created_at: NaiveDateTime,
    pub parts: Vec<MessagePart>,
}

impl Message {
    pub fn new(chat_id: ChatId, role: Role) -> Self {
        Self {
            id: MessageId::new(),
            chat_id,
            role,
            created_at: chrono::Utc::now().naive_utc(),
            parts: Vec::new(),
        }
    }

    pub fn make_title(text: &str) -> String {
        if text.chars().count() > MAX_TITLE_CHARS {
            format!(
                "{}...",
                text.chars().take(MAX_TITLE_CHARS).collect::<String>()
            )
        } else {
            text.to_string()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MessagePart {
    #[serde(rename = "text")]
    Text(TextBlock),
    #[serde(rename = "reasoning")]
    Reasoning(ReasoningBlock),
    ToolCall(ToolCallBlock),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningBlock {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextBlock {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallBlock {
    pub tool_call_id: String,
    pub tool_name: String,
    pub input: Value,
    pub output: Option<Value>,
    pub error: Option<String>,
    pub state: ToolCallState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolCallState {
    Invoked,
    Executing,
    Done,
    Error,
}
