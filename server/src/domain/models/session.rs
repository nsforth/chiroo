use chrono::NaiveDateTime;
use secrecy::SecretBox;
use uuid::Uuid;

use crate::domain::models::user::UserId;

#[derive(Clone, Copy, Debug)]
pub struct SessionId(pub Uuid);

#[derive(Clone, Debug)]
pub struct ProviderSessionId(pub String);

pub struct Session {
    pub id: SessionId,
    pub user_id: UserId,
    pub provider_sid: ProviderSessionId,
    pub access_token: SecretBox<str>,
    pub refresh_token: SecretBox<str>,
    pub created_at: NaiveDateTime,
    pub expires_at: NaiveDateTime,
}
