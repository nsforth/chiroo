use serde_json::Value;

use crate::domain::models::document::DocumentId;
use crate::domain::models::knowledge_base::KnowledgeBaseId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentChunkId(pub String);

pub struct DocumentChunk {
    pub id: DocumentChunkId,
    pub document_id: DocumentId,
    pub kb_id: KnowledgeBaseId,
    pub chunk_index: i32,
    pub chunk_total: i32,
    pub chunk_offset: i32,
    pub chunk_length: i32,
    pub document_total_length: i32,
    pub content: String,
    pub embedding: Vec<f32>,
    pub version: String,
    pub title: String,
    pub url: String,
    pub metadata: Value,
}

pub struct SearchedChunk {
    pub id: DocumentChunkId,
    pub document_id: DocumentId,
    pub chunk_index: i32,
    pub chunk_total: i32,
    pub chunk_offset: i32,
    pub chunk_length: i32,
    pub document_total_length: i32,
    pub content: String,
    pub version: String,
    pub title: String,
    pub url: String,
    pub metadata: Value,
    pub score: f32,
}
