use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::ErrorResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::models::user::{User, UserId};
use crate::errors::AppError;
use crate::state::AppState;
use crate::utils::{JsonExtractor, PathExtractor};

#[derive(Deserialize)]
pub struct GetUserQuery {
    pub username: String,
}

#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub admin: bool,
}

#[derive(Deserialize)]
pub struct CreateUserRequest {
    #[serde(rename = "ssoId")]
    pub sso_id: String,
    #[serde(rename = "ssoProvider")]
    pub sso_provider: Option<String>,
    pub username: String,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Deserialize)]
pub struct DeleteUserQuery {
    #[serde(rename = "purgeChats", default)]
    pub purge_chats: bool,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: String,
    pub name: Option<String>,
    pub username: String,
    pub email: Option<String>,
    pub admin: bool,
    #[serde(rename = "createdAt")]
    pub created_at: String,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id.0.to_string(),
            name: user.name,
            username: user.username,
            email: user.email,
            admin: user.admin,
            created_at: user.created_at.and_utc().to_rfc3339(),
        }
    }
}

pub async fn get_user_by_username(
    State(state): State<AppState>,
    Query(query): Query<GetUserQuery>,
) -> Result<Json<UserResponse>, ErrorResponse> {
    let user = state
        .user_service
        .get_user_by_username(&query.username)
        .await?;
    Ok(Json(user.into()))
}

pub async fn update_user_admin(
    State(state): State<AppState>,
    PathExtractor(id): PathExtractor<Uuid>,
    JsonExtractor(body): JsonExtractor<UpdateUserRequest>,
) -> Result<Json<UserResponse>, ErrorResponse> {
    let user = state.user_service.set_admin(UserId(id), body.admin).await?;
    Ok(Json(user.into()))
}

pub async fn create_user(
    State(state): State<AppState>,
    JsonExtractor(body): JsonExtractor<CreateUserRequest>,
) -> Result<Json<UserResponse>, ErrorResponse> {
    let user = state
        .user_service
        .create_user(
            &body.sso_id,
            body.sso_provider
                .as_deref()
                .ok_or(AppError::BadRequest("SSO Provider required".to_string()))?,
            &body.username,
            body.name.as_deref(),
            body.email.as_deref(),
        )
        .await?;
    Ok(Json(user.into()))
}

pub async fn delete_user(
    State(state): State<AppState>,
    PathExtractor(id): PathExtractor<Uuid>,
    Query(query): Query<DeleteUserQuery>,
) -> Result<(StatusCode,), ErrorResponse> {
    state
        .user_service
        .delete_user(UserId(id), query.purge_chats)
        .await?;
    Ok((StatusCode::NO_CONTENT,))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn create_user_request_deserializes_camel_case_json() {
        let body: CreateUserRequest = serde_json::from_value(json!({
            "ssoId": "sso-123",
            "ssoProvider": "okta",
            "username": "jane",
            "name": "Jane Doe",
            "email": "jane@example.com",
        }))
        .expect("deserialize");

        assert_eq!(body.sso_id, "sso-123");
        assert_eq!(body.sso_provider.as_deref(), Some("okta"));
        assert_eq!(body.username, "jane");
        assert_eq!(body.name.as_deref(), Some("Jane Doe"));
        assert_eq!(body.email.as_deref(), Some("jane@example.com"));
    }

    #[test]
    fn create_user_request_sso_provider_is_optional() {
        let body: CreateUserRequest = serde_json::from_value(json!({
            "ssoId": "sso-456",
            "username": "joe",
        }))
        .expect("deserialize without ssoProvider");
        assert_eq!(body.sso_provider, None);

        let body: CreateUserRequest = serde_json::from_value(
            json!({ "ssoId": "sso-789", "ssoProvider": null, "username": "joe" }),
        )
        .expect("deserialize with null ssoProvider");
        assert_eq!(body.sso_provider, None);
    }

    #[test]
    fn delete_user_query_defaults_purge_chats_to_false() {
        let query: DeleteUserQuery = serde_json::from_value(json!({})).expect("deserialize");
        assert!(!query.purge_chats);

        let query: DeleteUserQuery =
            serde_json::from_value(json!({ "purgeChats": true })).expect("deserialize");
        assert!(query.purge_chats);
    }
}
