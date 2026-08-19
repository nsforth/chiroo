use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::models::knowledge_base::KnowledgeBaseId;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub enum MimeType {
    #[serde(rename = "text/plain")]
    TextPlain,
}

impl MimeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TextPlain => "text/plain",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "text/plain" => Some(Self::TextPlain),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct DocumentId(pub String);

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReindexMode {
    #[default]
    Compare,
    Force,
    Full,
}

pub struct CreateDocument {
    pub id: String,
    pub kb_id: KnowledgeBaseId,
    pub title: String,
    pub url: String,
    pub version: String,
    pub mime_type: MimeType,
    pub metadata: Value,
}

#[derive(Debug)]
pub struct Document {
    pub id: DocumentId,
    pub kb_id: KnowledgeBaseId,
    pub title: String,
    pub url: String,
    pub version: String,
    pub mime_type: MimeType,
    pub metadata: Value,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl Document {
    pub fn with_meta(
        &self,
        title: String,
        url: String,
        version: String,
        mime_type: MimeType,
        metadata: Value,
    ) -> Self {
        Document {
            id: self.id.clone(),
            kb_id: self.kb_id,
            title,
            url,
            version,
            mime_type,
            metadata,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

pub struct DocumentData {
    pub document_id: DocumentId,
    pub data: Vec<u8>,
}
