use serde::Serialize;

use crate::handlers::user::PublicUserResponse;

mod get_session;

pub use get_session::get_session;

#[derive(Debug, Serialize, Clone)]
pub struct SessionResponse {
    pub id: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<PublicUserResponse>,
}
