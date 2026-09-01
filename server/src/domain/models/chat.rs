use crate::domain::models::user::UserId;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct ChatId(pub Uuid);

pub struct Chat {
    pub id: ChatId,
    pub title: Option<String>,
    pub owner: UserId,
    pub public: bool,
    pub created_at: NaiveDateTime,
}
