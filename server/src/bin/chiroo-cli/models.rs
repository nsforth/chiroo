use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Kb {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) public: bool,
    pub(crate) metadata: Value,
    #[serde(default)]
    pub(crate) language: Option<String>,
    #[serde(rename = "createdAt")]
    pub(crate) created_at: String,
    #[serde(rename = "updatedAt")]
    pub(crate) updated_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct KbRequest {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) public: bool,
    pub(crate) metadata: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) language: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Doc {
    pub(crate) id: String,
    #[serde(rename = "kbId")]
    pub(crate) kb_id: String,
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) version: String,
    #[serde(rename = "mimeType")]
    pub(crate) mime_type: String,
    pub(crate) metadata: Value,
    #[serde(rename = "createdAt")]
    pub(crate) created_at: String,
    #[serde(rename = "updatedAt")]
    pub(crate) updated_at: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ReindexRequest {
    pub(crate) mode: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReindexResponse {
    #[serde(rename = "reindexed")]
    pub(crate) reindexed: bool,
    pub(crate) chunks: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct User {
    pub(crate) id: String,
    pub(crate) admin: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct UpdateUserRequest {
    pub(crate) admin: bool,
}
