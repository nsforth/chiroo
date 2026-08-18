use serde::{Deserialize, Serialize};

use crate::domain::models::document_chunk::SearchedChunk;

/// A source cited by the assistant, extracted from tool results.
///
/// `source_id` is the document id so the same document found via search and
/// extract is deduplicated downstream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub source_id: String,
    pub url: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub score: Option<f32>,
}

impl From<&SearchedChunk> for Source {
    fn from(chunk: &SearchedChunk) -> Self {
        Source {
            source_id: chunk.document_id.0.clone(),
            url: chunk.url.clone(),
            title: Some(chunk.title.clone()),
            score: Some(chunk.score),
        }
    }
}
