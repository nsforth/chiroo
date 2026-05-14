use serde::Serialize;
mod get_session;

pub use get_session::get_session;

#[derive(Debug, Serialize, Clone)]
pub struct SessionResponse {
    pub id: String,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserResponse>,
}

#[derive(Debug, Serialize, Clone)]
pub struct UserResponse {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
    pub username: String,
}
