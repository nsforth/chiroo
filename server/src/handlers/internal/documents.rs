use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{ErrorResponse, IntoResponse};
use axum::Json;
use axum_typed_multipart::{FieldData, TryFromMultipart, TypedMultipart};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::domain::models::document::{Document, DocumentId, MimeType, ReindexMode};
use crate::domain::models::knowledge_base::KnowledgeBaseId;
use crate::state::AppState;
use crate::utils::PathExtractor;

#[derive(TryFromMultipart)]
pub struct CreateDocForm {
    pub id: String,
    pub kb_id: String,
    pub version: String,
    #[form_data(default)]
    pub title: String,
    #[form_data(default)]
    pub url: String,
    #[form_data(default)]
    pub mime_type: String,
    #[form_data(default)]
    pub metadata: String,
    pub data: FieldData<Bytes>,
}

#[derive(TryFromMultipart)]
pub struct UpdateDocForm {
    pub version: String,
    pub title: String,
    pub url: String,
    pub mime_type: String,
    pub metadata: String,
    pub data: FieldData<Bytes>,
}

#[derive(Deserialize)]
pub struct ListDocsQuery {
    #[serde(rename = "kb_id")]
    pub kb_id: Uuid,
    #[serde(default)]
    pub filter: Option<String>,
}

#[derive(Serialize)]
pub struct DocListItem {
    pub id: String,
    #[serde(rename = "kbId")]
    pub kb_id: String,
    pub title: String,
    pub url: String,
    pub version: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub metadata: Value,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

impl From<Document> for DocListItem {
    fn from(doc: Document) -> Self {
        Self {
            id: doc.id.0,
            kb_id: doc.kb_id.0.to_string(),
            title: doc.title,
            url: doc.url,
            version: doc.version,
            mime_type: doc.mime_type.as_str().to_string(),
            metadata: doc.metadata,
            created_at: doc.created_at.and_utc().to_rfc3339(),
            updated_at: doc.updated_at.and_utc().to_rfc3339(),
        }
    }
}

fn bad_request(msg: &str) -> ErrorResponse {
    (StatusCode::BAD_REQUEST, msg.to_string())
        .into_response()
        .into()
}

fn parse_mime_type(s: &str) -> Result<MimeType, ErrorResponse> {
    MimeType::from_str(s).ok_or_else(|| bad_request(&format!("Unsupported mime type: {s}")))
}

fn parse_metadata(text: &str) -> Result<Value, ErrorResponse> {
    if text.is_empty() {
        return Ok(Value::default());
    }
    serde_json::from_str(text).map_err(|e| bad_request(&format!("Invalid metadata JSON: {e}")))
}

pub async fn create_document(
    State(state): State<AppState>,
    TypedMultipart(form): TypedMultipart<CreateDocForm>,
) -> Result<(StatusCode, Json<DocListItem>), ErrorResponse> {
    let kb_id = Uuid::parse_str(&form.kb_id).map_err(|_| bad_request("Invalid kb_id UUID"))?;
    let mime_type = parse_mime_type(&form.mime_type)?;
    let metadata = parse_metadata(&form.metadata)?;
    let doc = state
        .knowledge_service
        .create_document(
            form.id,
            KnowledgeBaseId(kb_id),
            form.title,
            form.url,
            form.version,
            mime_type,
            metadata,
            form.data.contents.to_vec(),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(doc.into())))
}

pub async fn list_documents(
    State(state): State<AppState>,
    Query(query): Query<ListDocsQuery>,
) -> Result<Json<Vec<DocListItem>>, ErrorResponse> {
    let docs = state
        .knowledge_service
        .list_documents(KnowledgeBaseId(query.kb_id), query.filter.as_deref())
        .await?;
    Ok(Json(docs.into_iter().map(Into::into).collect()))
}

pub async fn get_document(
    State(state): State<AppState>,
    PathExtractor(id): PathExtractor<String>,
) -> Result<Json<DocListItem>, ErrorResponse> {
    let doc = state
        .knowledge_service
        .get_document(&DocumentId(id))
        .await?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Document not found").into_response())?;
    Ok(Json(doc.into()))
}

pub async fn get_document_data(
    State(state): State<AppState>,
    PathExtractor(id): PathExtractor<String>,
) -> Result<impl IntoResponse, ErrorResponse> {
    let (doc, doc_data) = state
        .knowledge_service
        .get_document_with_data(&DocumentId(id))
        .await?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "Document not found").into_response())?;

    let content_type = doc.mime_type.as_str();
    Ok(([(header::CONTENT_TYPE, content_type)], doc_data.data))
}

pub async fn update_document(
    State(state): State<AppState>,
    PathExtractor(id): PathExtractor<String>,
    TypedMultipart(form): TypedMultipart<UpdateDocForm>,
) -> Result<Json<DocListItem>, ErrorResponse> {
    let mime_type = parse_mime_type(&form.mime_type)?;
    let metadata = parse_metadata(&form.metadata)?;
    let doc = state
        .knowledge_service
        .update_document(
            &DocumentId(id),
            form.title,
            form.url,
            form.version,
            mime_type,
            metadata,
            form.data.contents.to_vec(),
        )
        .await?;
    Ok(Json(doc.into()))
}

#[derive(Deserialize)]
pub struct UpdateDocMetadataRequest {
    pub title: String,
    pub url: String,
    pub metadata: Value,
}

pub async fn update_document_metadata(
    State(state): State<AppState>,
    PathExtractor(id): PathExtractor<String>,
    Json(req): Json<UpdateDocMetadataRequest>,
) -> Result<Json<DocListItem>, ErrorResponse> {
    let doc = state
        .knowledge_service
        .update_document_metadata(&DocumentId(id), req.title, req.url, req.metadata)
        .await?;
    Ok(Json(doc.into()))
}

pub async fn delete_document(
    State(state): State<AppState>,
    PathExtractor(id): PathExtractor<String>,
) -> Result<StatusCode, ErrorResponse> {
    state
        .knowledge_service
        .delete_document(&DocumentId(id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct ReindexDocRequest {
    #[serde(default)]
    pub mode: ReindexMode,
}

#[derive(Serialize)]
pub struct ReindexDocResponse {
    #[serde(rename = "reindexed")]
    pub reindexed: bool,
    pub chunks: usize,
}

pub async fn reindex_document(
    State(state): State<AppState>,
    PathExtractor(id): PathExtractor<String>,
    Json(req): Json<ReindexDocRequest>,
) -> Result<Json<ReindexDocResponse>, ErrorResponse> {
    let outcome = state
        .knowledge_service
        .reindex_document(&DocumentId(id), req.mode)
        .await?;
    Ok(Json(ReindexDocResponse {
        reindexed: outcome.reindexed,
        chunks: outcome.chunks,
    }))
}
