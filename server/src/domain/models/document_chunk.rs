use serde_json::Value;
use uuid::Uuid;

use crate::domain::models::document::DocumentId;
use crate::domain::models::knowledge_base::KnowledgeBaseId;

const CHUNK_NS: Uuid = Uuid::from_bytes([
    0x3f, 0x2c, 0x14, 0x86, 0x95, 0xbc, 0x11, 0xf1, 0xaa, 0x1a, 0x87, 0x18, 0x66, 0x0f, 0x1f, 0x8c,
]);

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct DocumentChunkId(pub Uuid);

impl DocumentChunkId {
    pub fn from_doc_id_and_chunk_idx(document_id: &DocumentId, chunk_index: usize) -> Self {
        let name = format!("{}:{}", &document_id.0, chunk_index);
        let uuid = Uuid::new_v5(&CHUNK_NS, name.as_bytes());
        DocumentChunkId(uuid)
    }
}

#[derive(Clone)]
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

impl DocumentChunk {
    pub fn with_embedding(&self, embedding: Vec<f32>) -> Self {
        DocumentChunk {
            embedding,
            ..self.clone()
        }
    }
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
