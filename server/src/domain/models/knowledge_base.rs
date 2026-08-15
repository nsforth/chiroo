use chrono::NaiveDateTime;
use serde::Deserialize;
use serde_json::Value;
use tera::Context;
use uuid::Uuid;

use crate::errors::AppError;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub struct KnowledgeBaseId(pub Uuid);

pub struct CreateKnowledgeBase {
    pub name: String,
    pub description: String,
    pub public: bool,
    pub metadata: Value,
    pub language: Option<String>,
}

#[derive(Clone)]
pub struct KnowledgeBase {
    pub id: KnowledgeBaseId,
    pub name: String,
    pub description: String,
    pub public: bool,
    pub metadata: Value,
    pub language: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl KnowledgeBase {
    pub fn render_description(&self) -> Result<String, AppError> {
        tera::Tera::one_off(
            &self.description,
            &Context::from_value(self.metadata.clone()).unwrap_or_default(),
            false,
        )
        .map_err(|_| AppError::InternalServerError)
    }
}
