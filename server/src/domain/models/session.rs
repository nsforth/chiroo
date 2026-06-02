use chrono::NaiveDateTime;
use uuid::Uuid;

use crate::domain::models::user::UserId;

#[derive(Clone, Copy, Debug)]
pub struct SessionId(pub Uuid);

pub struct Session {
    pub id: SessionId,
    pub user_id: UserId,
    pub provider_sid: String,
    pub access_token: String,
    pub refresh_token: String,
    pub created_at: NaiveDateTime,
    pub expires_at: NaiveDateTime,
}
