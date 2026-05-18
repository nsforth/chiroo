use crate::domain::models::user::UserId;
use chrono::NaiveDateTime;
use uuid::Uuid;

pub struct ChatId(pub Uuid);

pub struct Chat {
    pub id: ChatId,
    pub title: Option<String>,
    pub owner: UserId,
    pub public: bool,
    pub created_at: NaiveDateTime,
}
