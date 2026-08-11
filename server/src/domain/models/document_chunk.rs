use serde_json::Value;

use crate::domain::models::document::DocumentId;
use crate::domain::models::knowledge_base::KnowledgeBaseId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentChunkId(pub String);

impl DocumentChunkId {
    pub fn from_doc_id_and_chunk_idx(document_id: &DocumentId, chunk_index: usize) -> Self {
        DocumentChunkId(format!("{}-chunk-{}", &document_id.0, chunk_index))
    }
}

pub struct DocumentChunk {
    pub id: DocumentChunkId,
    pub document_id: DocumentId,
    pub kb_id: KnowledgeBaseId,
    pub chunk_index: usize,
    pub chunk_total: usize,
    pub chunk_offset: usize,
    pub chunk_length: usize,
    pub document_total_length: usize,
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
    pub chunk_index: usize,
    pub chunk_total: usize,
    pub chunk_offset: usize,
    pub chunk_length: usize,
    pub document_total_length: usize,
    pub content: String,
    pub version: String,
    pub title: String,
    pub url: String,
    pub metadata: Value,
    pub score: f32,
}
