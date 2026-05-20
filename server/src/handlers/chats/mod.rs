use serde::Deserialize;
use serde::Serialize;

mod chats;
mod messages;
mod ui_messages;
mod votes;

pub use chats::{
    create_chat, delete_chat, get_chat, get_chats, update_title, update_visibility,
};
pub use messages::{delete_messages_after, get_messages};
pub use votes::{delete_vote, get_votes, upsert_vote};

use crate::domain::models::chat::Chat;

#[derive(Debug, Clone, Deserialize)]
pub struct CreateChatRequest {
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatsResponse {
    chats: Vec<ChatResponse>,
}

impl From<Vec<Chat>> for ChatsResponse {
    fn from(value: Vec<Chat>) -> Self {
        let chats_responses = value.iter().map(From::from).collect();
        ChatsResponse {
            chats: chats_responses,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatResponse {
    pub id: String,
    pub title: Option<String>,
    #[serde(rename = "userId")]
    pub user_id: String,
    pub visibility: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

impl From<&Chat> for ChatResponse {
    fn from(value: &Chat) -> Self {
        let visibility = if value.public {
            String::from("public")
        } else {
            String::from("private")
        };
        Self {
            id: value.id.0.to_string(),
            title: value.title.clone(),
            user_id: value.owner.0.to_string(),
            visibility,
            created_at: value.created_at.and_utc().to_rfc3339(),
        }
    }
}
