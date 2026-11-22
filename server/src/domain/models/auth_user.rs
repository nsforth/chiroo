use crate::domain::models::user::UserId;

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user_id: UserId,
    /// The server session this request is attached to, if any. `None` for
    /// stateless requests (e.g. a Bearer token with no cookie session).
    pub session_id: Option<String>,
}
