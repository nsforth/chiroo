use serde::Serialize;
mod get_chats;
pub use get_chats::get_chats;

#[derive(Debug, Clone, Serialize)]
pub struct ChatsResponse {
    chats: Vec<Chat>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Chat {
    pub id: String,
    pub title: Option<String>,
    #[serde(rename = "userId")]
    pub user_id: String,
    pub visibility: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}
