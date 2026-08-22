use axum::extract::{Query, State};
use axum::response::ErrorResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::models::user::{User, UserId};
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
