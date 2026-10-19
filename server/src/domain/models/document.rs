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

#[derive(Debug)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_type_round_trips() {
        assert_eq!(MimeType::from_str("text/plain"), Some(MimeType::TextPlain));
        assert_eq!(MimeType::TextPlain.as_str(), "text/plain");
    }

    #[test]
    fn mime_type_unknown_is_none() {
        assert_eq!(MimeType::from_str("application/pdf"), None);
        assert_eq!(MimeType::from_str("text/plain;charset=utf-8"), None);
    }

    #[test]
    fn mime_type_serde_rename() {
        assert_eq!(
            serde_json::to_string(&MimeType::TextPlain).unwrap(),
            "\"text/plain\""
        );
        let mime: MimeType = serde_json::from_str("\"text/plain\"").unwrap();
        assert_eq!(mime, MimeType::TextPlain);
    }

    #[test]
    fn reindex_mode_defaults_to_compare() {
        assert_eq!(ReindexMode::default(), ReindexMode::Compare);
    }

    #[test]
    fn reindex_mode_serde_is_lowercase() {
        for (raw, mode) in [
            ("\"compare\"", ReindexMode::Compare),
            ("\"force\"", ReindexMode::Force),
            ("\"full\"", ReindexMode::Full),
        ] {
            assert_eq!(serde_json::from_str::<ReindexMode>(raw).unwrap(), mode);
            assert_eq!(serde_json::to_string(&mode).unwrap(), raw);
        }
    }

    #[test]
    fn with_meta_preserves_identity_and_timestamps() {
        let doc = Document {
            id: DocumentId("d1".into()),
            kb_id: KnowledgeBaseId(uuid::Uuid::new_v4()),
            title: "old".into(),
            url: "http://old".into(),
            version: "1".into(),
            mime_type: MimeType::TextPlain,
            metadata: serde_json::Value::Null,
            created_at: chrono::NaiveDateTime::MIN,
            updated_at: chrono::NaiveDateTime::MIN,
        };
        let updated = doc.with_meta(
            "new".into(),
            "http://new".into(),
            "2".into(),
            MimeType::TextPlain,
            serde_json::Value::Null,
        );
        assert_eq!(updated.id, doc.id);
        assert_eq!(updated.kb_id, doc.kb_id);
        assert_eq!(updated.title, "new");
        assert_eq!(updated.created_at, doc.created_at);
    }
}
