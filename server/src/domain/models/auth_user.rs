use crate::domain::models::user::UserId;

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user_id: UserId,
    pub session_id: String,
}
