use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{ErrorResponse, IntoResponse};
use axum::Json;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::domain::models::knowledge_base::{KnowledgeBase, KnowledgeBaseId};
use crate::errors::AppError;
use crate::utils::PathExtractor;
use crate::AppState;

#[derive(Debug, Serialize)]
pub struct KbListResponse {
    kbs: Vec<KbListItemResponse>,
}

impl From<Vec<KnowledgeBase>> for KbListResponse {
    fn from(kbs: Vec<KnowledgeBase>) -> Self {
        Self {
            kbs: kbs
                .into_iter()
                .map(|kb| KbListItemResponse {
                    id: kb.id.0.to_string(),
                    name: kb.name,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct KbListItemResponse {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct KbDetailResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "renderedDescription")]
    pub rendered_description: String,
    pub public: bool,
    pub metadata: Value,
    pub language: Option<String>,
    pub indexes: Value,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

impl KbDetailResponse {
    fn new(kb: &KnowledgeBase) -> Result<Self, AppError> {
        let rendered_description = kb.render_description()?;
        Ok(Self {
            id: kb.id.0.to_string(),
            name: kb.name.clone(),
            description: kb.description.clone(),
            rendered_description,
            public: kb.public,
            metadata: kb.metadata.clone(),
            language: kb.language.clone(),
            indexes: kb
                .indexes
                .to_value()
                .unwrap_or_else(|| Value::Object(serde_json::Map::new())),
            created_at: kb.created_at.and_utc().to_rfc3339(),
            updated_at: kb.updated_at.and_utc().to_rfc3339(),
        })
    }
}

pub async fn list_kbs(
    State(state): State<AppState>,
) -> Result<Json<KbListResponse>, ErrorResponse> {
    let kbs = state.knowledge_service.list_kbs().await?;
    Ok(Json(KbListResponse::from(kbs)))
}

pub async fn get_kb(
    State(state): State<AppState>,
    PathExtractor(id): PathExtractor<Uuid>,
) -> Result<Json<KbDetailResponse>, ErrorResponse> {
    let kb = state
        .knowledge_service
        .get_kb(KnowledgeBaseId(id))
        .await?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Knowledge base not found").into_response())?;
    Ok(Json(KbDetailResponse::new(&kb)?))
}
