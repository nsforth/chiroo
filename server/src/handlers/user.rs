use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct PublicUserResponse {
    pub id: String,
    pub name: String,
    pub avatar: Option<String>,
    pub username: String,
    pub admin: bool,
}
