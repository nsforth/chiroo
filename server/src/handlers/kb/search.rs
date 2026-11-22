use std::collections::HashMap;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{ErrorResponse, IntoResponse};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::config::Config;
use crate::domain::models::document_chunk::SearchedChunk;
use crate::domain::models::knowledge_base::{KnowledgeBase, KnowledgeBaseId};
use crate::domain::models::search_filter::SearchFilter;
use crate::domain::models::search_params::SearchParams;
use crate::errors::AppError;
use crate::utils::JsonExtractor;
use crate::utils::PathExtractor;
use crate::AppState;

#[derive(Debug, Clone, Deserialize)]
pub struct SearchAllRequest {
    pub queries: Vec<String>,
    #[serde(rename = "kbIds")]
    pub kb_ids: Option<Vec<Uuid>>,
    pub limit: Option<usize>,
    #[serde(rename = "candidateLimit")]
    pub candidate_limit: Option<u64>,
    #[serde(rename = "rerankerLimit")]
    pub reranker_limit: Option<u32>,
    #[serde(rename = "rerankerThreshold")]
    pub reranker_threshold: Option<f32>,
    /// Run the reranker (default `true`). `false` skips it: results keep the
    /// hybrid score and are merged by dedup only.
    pub rerank: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchInKbRequest {
    pub queries: Vec<String>,
    pub filters: Option<Value>,
    pub limit: Option<usize>,
    #[serde(rename = "candidateLimit")]
    pub candidate_limit: Option<u64>,
    #[serde(rename = "rerankerLimit")]
    pub reranker_limit: Option<u32>,
    #[serde(rename = "rerankerThreshold")]
    pub reranker_threshold: Option<f32>,
    /// Run the reranker (default `true`). `false` skips it: results keep the
    /// hybrid score and are merged by dedup only.
    pub rerank: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    chunks: Vec<SearchedChunkResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchedChunkResponse {
    pub id: String,
    #[serde(rename = "kbId")]
    pub kb_id: String,
    #[serde(rename = "kbName")]
    pub kb_name: String,
    #[serde(rename = "documentId")]
    pub document_id: String,
    pub title: String,
    pub url: String,
    pub content: String,
    pub score: f32,
    pub metadata: Value,
    #[serde(rename = "chunkIndex")]
    pub chunk_index: usize,
    #[serde(rename = "chunkTotal")]
    pub chunk_total: usize,
    pub query: String,
}

impl SearchedChunkResponse {
    fn from_chunk(chunk: SearchedChunk, kb_names: &HashMap<Uuid, String>) -> Self {
        Self {
            id: chunk.id.0.to_string(),
            kb_id: chunk.kb_id.0.to_string(),
            kb_name: kb_names.get(&chunk.kb_id.0).cloned().unwrap_or_default(),
            document_id: chunk.document_id.0,
            title: chunk.title,
            url: chunk.url,
            content: chunk.content,
            score: chunk.score,
            metadata: chunk.metadata,
            chunk_index: chunk.chunk_index,
            chunk_total: chunk.chunk_total,
            query: chunk.query,
        }
    }
}

pub async fn search_all(
    State(state): State<AppState>,
    JsonExtractor(body): JsonExtractor<SearchAllRequest>,
) -> Result<Json<SearchResponse>, ErrorResponse> {
    let queries = non_empty_queries(&body.queries)?;
    let params = search_params_from_request(
        state.config,
        body.limit,
        body.candidate_limit,
        body.reranker_limit,
        body.reranker_threshold,
        body.rerank.unwrap_or(true),
    )?;

    let kb_ids = body
        .kb_ids
        .map(|ids| ids.into_iter().map(KnowledgeBaseId).collect());
    let kbs = state.knowledge_service.list_kbs_with_filter(kb_ids).await?;
    let chunks = state
        .search_service
        .search_all(queries, &kbs, &SearchFilter::default(), &params)
        .await?;

    Ok(Json(search_response(chunks, &kbs)))
}

pub async fn search_in_kb(
    State(state): State<AppState>,
    PathExtractor(id): PathExtractor<Uuid>,
    JsonExtractor(body): JsonExtractor<SearchInKbRequest>,
) -> Result<Json<SearchResponse>, ErrorResponse> {
    let queries = non_empty_queries(&body.queries)?;
    let params = search_params_from_request(
        state.config,
        body.limit,
        body.candidate_limit,
        body.reranker_limit,
        body.reranker_threshold,
        body.rerank.unwrap_or(true),
    )?;

    let kb = state
        .knowledge_service
        .get_kb(KnowledgeBaseId(id))
        .await?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Knowledge base not found").into_response())?;

    let empty_filters = json!({});
    let filters_value = body
        .filters
        .as_ref()
        .filter(|v| !v.is_null())
        .unwrap_or(&empty_filters);
    let mut filters = SearchFilter::from_json_value(filters_value)?;
    filters.validate(&kb.indexes)?;

    let chunks = state
        .search_service
        .search_in_kb(queries, &kb, &filters, &params)
        .await?;

    Ok(Json(search_response(chunks, std::slice::from_ref(&kb))))
}

fn non_empty_queries(queries: &[String]) -> Result<Vec<String>, AppError> {
    if queries.is_empty() {
        return Err(AppError::BadRequest("queries must not be empty".into()));
    }
    Ok(queries.to_vec())
}

fn search_params_from_request(
    config: &Config,
    limit: Option<usize>,
    candidate_limit: Option<u64>,
    reranker_limit: Option<u32>,
    reranker_threshold: Option<f32>,
    rerank: bool,
) -> Result<SearchParams, AppError> {
    SearchParams::try_new(
        limit.unwrap_or(config.search.limit),
        candidate_limit.unwrap_or(config.search.candidate_limit),
        reranker_limit.unwrap_or(config.search.reranker_limit),
        reranker_threshold.unwrap_or(config.search.score_threshold),
        rerank,
    )
}

fn search_response(chunks: Vec<SearchedChunk>, kbs: &[KnowledgeBase]) -> SearchResponse {
    let kb_names: HashMap<Uuid, String> = kbs.iter().map(|kb| (kb.id.0, kb.name.clone())).collect();
    SearchResponse {
        chunks: chunks
            .into_iter()
            .map(|chunk| SearchedChunkResponse::from_chunk(chunk, &kb_names))
            .collect(),
    }
}
