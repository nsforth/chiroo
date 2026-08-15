use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{ErrorResponse, IntoResponse};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::domain::models::knowledge_base::{KnowledgeBase, KnowledgeBaseId};
use crate::state::AppState;
use crate::utils::JsonExtractor;
use crate::utils::PathExtractor;

#[derive(Deserialize)]
pub struct CreateKbRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub language: Option<String>,
}

#[derive(Serialize)]
pub struct KbResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub public: bool,
    pub metadata: Value,
    pub language: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

impl From<KnowledgeBase> for KbResponse {
    fn from(kb: KnowledgeBase) -> Self {
        Self {
            id: kb.id.0.to_string(),
            name: kb.name,
            description: kb.description,
            public: kb.public,
            metadata: kb.metadata,
            language: kb.language,
            created_at: kb.created_at.and_utc().to_rfc3339(),
            updated_at: kb.updated_at.and_utc().to_rfc3339(),
        }
    }
}

#[derive(Deserialize)]
pub struct UpdateKbRequest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub public: bool,
    #[serde(default)]
    pub metadata: Value,
}

pub async fn create_kb(
    State(state): State<AppState>,
    JsonExtractor(body): JsonExtractor<CreateKbRequest>,
) -> Result<(StatusCode, Json<KbResponse>), ErrorResponse> {
    let kb = state
        .knowledge_service
        .create_kb(
            body.name,
            body.description,
            body.public,
            body.metadata,
            body.language,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(kb.into())))
}

pub async fn list_kbs(
    State(state): State<AppState>,
) -> Result<Json<Vec<KbResponse>>, ErrorResponse> {
    let kbs = state.knowledge_service.list_kbs().await?;
    Ok(Json(kbs.into_iter().map(Into::into).collect()))
}

pub async fn get_kb(
    State(state): State<AppState>,
    PathExtractor(id): PathExtractor<Uuid>,
) -> Result<Json<KbResponse>, ErrorResponse> {
    let kb = state
        .knowledge_service
        .get_kb(KnowledgeBaseId(id))
        .await?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "KB not found").into_response())?;
    Ok(Json(kb.into()))
}

pub async fn update_kb(
    State(state): State<AppState>,
    PathExtractor(id): PathExtractor<Uuid>,
    JsonExtractor(body): JsonExtractor<UpdateKbRequest>,
) -> Result<Json<KbResponse>, ErrorResponse> {
    let kb = state
        .knowledge_service
        .update_kb(
            KnowledgeBaseId(id),
            body.name,
            body.description,
            body.public,
            body.metadata,
        )
        .await?;
    Ok(Json(kb.into()))
}

pub async fn delete_kb(
    State(state): State<AppState>,
    PathExtractor(id): PathExtractor<Uuid>,
) -> Result<StatusCode, ErrorResponse> {
    state
        .knowledge_service
        .delete_kb(KnowledgeBaseId(id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
