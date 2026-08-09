use deadpool_diesel::postgres::Pool;
use serde_json::Value;
use tracing::{info, instrument};
use uuid::Uuid;

use crate::domain::models::document::{
    CreateDocument, Document, DocumentData, DocumentId, MimeType,
};
use crate::domain::models::knowledge_base::{CreateKnowledgeBase, KnowledgeBase, KnowledgeBaseId};
use crate::errors::AppError;
use crate::infra::repositories::document_repository;
use crate::infra::repositories::knowledge_base_repository;

#[derive(Clone)]
pub struct KnowledgeService {
    pool: Pool,
}

impl KnowledgeService {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    #[instrument(skip(self, metadata))]
    pub async fn create_kb(
        &self,
        name: String,
        description: String,
        public: bool,
        metadata: Value,
    ) -> Result<KnowledgeBase, AppError> {
        let cmd = CreateKnowledgeBase {
            name,
            description,
            public,
            metadata,
        };
        let result = knowledge_base_repository::insert(&self.pool, &cmd).await;
        match &result {
            Ok(_) => info!(status = "success"),
            Err(e) => info!(error = e.to_string(), status = "failed"),
        }
        let kb = result?;
        kb.render_description()
            .map_err(|e| AppError::BodyParsingError(e.to_string()))?;
        Ok(kb)
    }

    #[instrument(skip(self))]
    pub async fn list_kbs(&self) -> Result<Vec<KnowledgeBase>, AppError> {
        let result = knowledge_base_repository::list_all(&self.pool).await;
        match &result {
            Ok(kbs) => info!(kb_count = kbs.len(), status = "success"),
            Err(e) => info!(error = e.to_string(), status = "failed"),
        }
        Ok(result?)
    }

    #[instrument(skip(self))]
    pub async fn get_kb(&self, id: KnowledgeBaseId) -> Result<Option<KnowledgeBase>, AppError> {
        let result = knowledge_base_repository::get_by_id(&self.pool, id).await;
        match &result {
            Ok(Some(_)) => info!(status = "success"),
            Ok(None) => info!(status = "not_found"),
            Err(e) => info!(error = e.to_string(), status = "failed"),
        }
        Ok(result?)
    }

    #[instrument(skip(self, metadata))]
    pub async fn update_kb(
        &self,
        id: KnowledgeBaseId,
        name: String,
        description: String,
        public: bool,
        metadata: Value,
    ) -> Result<KnowledgeBase, AppError> {
        let kb = KnowledgeBase {
            id,
            name,
            description,
            public,
            metadata,
            created_at: chrono::NaiveDateTime::MIN,
            updated_at: chrono::NaiveDateTime::MIN,
        };
        kb.render_description()
            .map_err(|e| AppError::BodyParsingError(e.to_string()))?;

        let result = knowledge_base_repository::update(&self.pool, &kb).await;
        match &result {
            Ok(_) => info!(status = "success"),
            Err(e) => info!(error = e.to_string(), status = "failed"),
        }
        Ok(result?)
    }

    #[instrument(skip(self))]
    pub async fn delete_kb(&self, id: KnowledgeBaseId) -> Result<(), AppError> {
        let result = knowledge_base_repository::delete(&self.pool, id).await;
        match &result {
            Ok(true) => info!(status = "success"),
            Ok(false) => info!(status = "not_found"),
            Err(e) => info!(error = e.to_string(), status = "failed"),
        }
        if !result? {
            return Err(AppError::NotFound("KB not found".into()));
        }
        Ok(())
    }

    #[instrument(skip(self, metadata, data))]
    pub async fn create_document(
        &self,
        id: String,
        kb_id: KnowledgeBaseId,
        title: String,
        url: String,
        version: String,
        mime_type: MimeType,
        metadata: Value,
        data: Vec<u8>,
    ) -> Result<Document, AppError> {
        let cmd = CreateDocument {
            id: id.clone(),
            kb_id,
            title,
            url,
            version,
            mime_type,
            metadata,
        };
        let result = document_repository::insert(&self.pool, &cmd, &data).await;
        match &result {
            Ok(_) => info!(doc_id = id, status = "success"),
            Err(e) => info!(doc_id = id, error = e.to_string(), status = "failed"),
        }
        Ok(result?)
    }

    #[instrument(skip(self))]
    pub async fn list_documents(&self, kb_id: KnowledgeBaseId) -> Result<Vec<Document>, AppError> {
        let result = document_repository::list_by_kb(&self.pool, kb_id).await;
        match &result {
            Ok(docs) => info!(doc_count = docs.len(), status = "success"),
            Err(e) => info!(error = e.to_string(), status = "failed"),
        }
        Ok(result?)
    }

    #[instrument(skip(self))]
    pub async fn get_document(&self, id: &DocumentId) -> Result<Option<Document>, AppError> {
        let result = document_repository::get_by_id(&self.pool, id).await;
        match &result {
            Ok(Some(_)) => info!(status = "success"),
            Ok(None) => info!(status = "not_found"),
            Err(e) => info!(error = e.to_string(), status = "failed"),
        }
        Ok(result?)
    }

    #[instrument(skip(self))]
    pub async fn get_document_data(
        &self,
        id: &DocumentId,
    ) -> Result<Option<DocumentData>, AppError> {
        let result = document_repository::get_data_by_id(&self.pool, id).await;
        match &result {
            Ok(Some(_)) => info!(status = "success"),
            Ok(None) => info!(status = "not_found"),
            Err(e) => info!(error = e.to_string(), status = "failed"),
        }
        Ok(result?)
    }

    #[instrument(skip(self))]
    pub async fn get_document_with_data(
        &self,
        id: &DocumentId,
    ) -> Result<Option<(Document, DocumentData)>, AppError> {
        let result = document_repository::get_with_data(&self.pool, id).await;
        match &result {
            Ok(Some(_)) => info!(status = "success"),
            Ok(None) => info!(status = "not_found"),
            Err(e) => info!(error = e.to_string(), status = "failed"),
        }
        Ok(result?.map(|(doc, data)| {
            let doc_data = DocumentData {
                document_id: DocumentId(doc.id.0.clone()),
                data,
            };
            (doc, doc_data)
        }))
    }

    #[instrument(skip(self, metadata, data))]
    pub async fn update_document(
        &self,
        id: &DocumentId,
        title: String,
        url: String,
        version: String,
        mime_type: MimeType,
        metadata: Value,
        data: Vec<u8>,
    ) -> Result<Document, AppError> {
        let doc = Document {
            id: id.clone(),
            kb_id: KnowledgeBaseId(Uuid::nil()),
            title,
            url,
            version,
            mime_type,
            metadata,
            created_at: chrono::NaiveDateTime::MIN,
            updated_at: chrono::NaiveDateTime::MIN,
        };
        let result = document_repository::update(&self.pool, &doc, &data).await;
        match &result {
            Ok(_) => info!(status = "success"),
            Err(e) => info!(error = e.to_string(), status = "failed"),
        }
        Ok(result?)
    }

    #[instrument(skip(self))]
    pub async fn delete_document(&self, id: &DocumentId) -> Result<(), AppError> {
        let result = document_repository::delete(&self.pool, id).await;
        match &result {
            Ok(true) => info!(status = "success"),
            Ok(false) => info!(status = "not_found"),
            Err(e) => info!(error = e.to_string(), status = "failed"),
        }
        if !result? {
            return Err(AppError::NotFound("Document not found".into()));
        }
        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn delete_documents_by_kb(&self, kb_id: KnowledgeBaseId) -> Result<u64, AppError> {
        let result = document_repository::delete_by_kb(&self.pool, kb_id).await;
        match &result {
            Ok(count) => info!(deleted = count, status = "success"),
            Err(e) => info!(error = e.to_string(), status = "failed"),
        }
        Ok(result?)
    }
}
