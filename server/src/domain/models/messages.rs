use crate::domain::models::chat::ChatId;
use crate::domain::models::source::Source;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

const MAX_TITLE_CHARS: usize = 50;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct MessageId(pub Uuid);

impl MessageId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub fn user(chat_id: ChatId, text: String) -> Self {
        let mut msg = Self::new(chat_id, Role::User);
        msg.parts.push(MessagePart::Text(TextBlock { text }));
        msg
    }

    pub fn user_with_id(chat_id: ChatId, id: MessageId, text: String) -> Self {
        let mut msg = Self::user(chat_id, text);
        msg.id = id;
        msg
    }

    pub fn text(&self) -> String {
        self.parts
            .iter()
            .filter_map(|p| match p {
                MessagePart::Text(t) => Some(t.text.as_str()),
                _ => None,
            })
            .collect()
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
    #[serde(rename = "source-url")]
    SourceUrl(Source),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_title_short_unchanged() {
        assert_eq!(Message::make_title("Hello"), "Hello");
    }

    #[test]
    fn make_title_exactly_50_chars_unchanged() {
        let text: String = std::iter::repeat('a').take(50).collect();
        assert_eq!(Message::make_title(&text), text);
    }

    #[test]
    fn make_title_truncates_at_50_chars_with_ellipsis() {
        let text: String = std::iter::repeat('a').take(51).collect();
        let title = Message::make_title(&text);
        assert_eq!(title.chars().count(), 53);
        assert!(title.ends_with("..."));
    }

    #[test]
    fn make_title_empty() {
        assert_eq!(Message::make_title(""), "");
    }

    #[test]
    fn make_title_multibyte_chars_are_not_mangled() {
        let text = "Привет, мир! Это заголовок длиннее пятидесяти символов в чате";
        let title = Message::make_title(text);
        assert_eq!(title.chars().count(), 53);
        assert!(title.ends_with("..."));
    }

    #[test]
    fn user_message_has_user_role_and_text_part() {
        let msg = Message::user(ChatId(Uuid::new_v4()), "hi there".into());
        assert!(matches!(msg.role, Role::User));
        assert_eq!(msg.text(), "hi there");
    }

    #[test]
    fn text_joins_all_text_parts_and_ignores_others() {
        let chat_id = ChatId(Uuid::new_v4());
        let mut msg = Message::user(chat_id, "one".into());
        msg.parts
            .push(MessagePart::Text(TextBlock { text: "two".into() }));
        msg.parts.push(MessagePart::Reasoning(ReasoningBlock {
            text: "secret thinking".into(),
        }));
        assert_eq!(msg.text(), "onetwo");
    }
}
