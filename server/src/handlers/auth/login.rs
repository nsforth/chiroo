use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Deserialize)]
pub struct LoginRequest {
    pub redirect: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    #[serde(rename = "loginUrl")]
    pub login_url: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, crate::errors::AppError> {
    let login_url = state.auth_service.build_login_url(body.redirect).await?;
    Ok(Json(LoginResponse { login_url }))
}
