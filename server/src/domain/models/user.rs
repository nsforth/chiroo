use std::fmt;

use chrono::NaiveDateTime;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UserId(pub Uuid);

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

pub struct User {
    pub id: UserId,
    pub sso_id: String,
    pub sso_provider: String,
    pub name: Option<String>,
    pub username: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: NaiveDateTime,
}
