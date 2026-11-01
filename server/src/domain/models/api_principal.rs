use crate::domain::models::user::UserId;

/// Authenticated principal for the OpenAI-compatible API, resolved strictly
/// from the OIDC access token's `sub` claim via `users.sso_id`.
#[derive(Clone, Debug)]
pub struct ApiPrincipal {
    pub user_id: UserId,
}
