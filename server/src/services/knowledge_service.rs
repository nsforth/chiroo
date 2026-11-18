use std::collections::HashMap;

use deadpool_diesel::postgres::Pool;
use serde_json::Value;
use tracing::{info, instrument};

use crate::domain::models::document::{
    CreateDocument, Document, DocumentData, DocumentId, MimeType, ReindexMode,
};
use crate::domain::models::document_chunk::{DocumentChunk, DocumentChunkId};
use crate::domain::models::kb_index::KbIndexes;
use crate::domain::models::knowledge_base::{CreateKnowledgeBase, KnowledgeBase, KnowledgeBaseId};
use crate::errors::AppError;
use crate::infra::clients::embedding_client::EmbeddingClient;
use crate::infra::clients::qdrant_client::QdrantClient;
use crate::infra::repositories::document_repository;
use crate::infra::repositories::knowledge_base_repository;
use crate::services::chunking_service::ChunkingService;

#[derive(Clone)]
pub struct KnowledgeService {
    pool: Pool,
    chunking_service: ChunkingService,
    qdrant_client: QdrantClient,
    embedding_client: EmbeddingClient,
    embedding_dimensions: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct ReindexOutcome {
    pub reindexed: bool,
    pub chunks: usize,
}

impl KnowledgeService {
    pub fn new(
        pool: Pool,
        chunking_service: ChunkingService,
        qdrant_client: QdrantClient,
        embedding_client: EmbeddingClient,
        embedding_dimensions: u32,
    ) -> Self {
        Self {
            pool,
            chunking_service,
            qdrant_client,
            embedding_client,
            embedding_dimensions,
        }
    }

    fn validate_document_params<'a>(
        mime_type: &MimeType,
        data: &'a [u8],
    ) -> Result<&'a str, AppError> {
        if *mime_type != MimeType::TextPlain {
            return Err(AppError::BodyParsingError(format!(
                "unsupported mime type: {}",
                mime_type.as_str()
            )));
        }

        let text = std::str::from_utf8(data)
            .map_err(|e| AppError::BodyParsingError(format!("Invalid UTF-8: {e}")))?;
        if text.trim().is_empty() {
            return Err(AppError::BadRequest("document content is empty".into()));
        }

        Ok(text)
    }

    #[instrument(skip(self, metadata))]
    pub async fn create_kb(
        &self,
        name: String,
        description: String,
        public: bool,
        metadata: Value,
        language: Option<String>,
        indexes: Option<Value>,
    ) -> Result<KnowledgeBase, AppError> {
        let indexes = indexes.filter(|v| !v.is_null());
        let parsed_indexes = match &indexes {
            Some(v) => Some(KbIndexes::parse(v)?),
            None => None,
        };
        let cmd = CreateKnowledgeBase {
            name,
            description,
            public,
            metadata,
            language,
            indexes,
        };
        let result = knowledge_base_repository::insert(&self.pool, &cmd).await;
        if let Err(e) = &result {
            info!(error = e.to_string(), status = "failed");
        }
        let kb = result?;
        if let Err(e) = kb.render_description() {
            info!(error = e.to_string(), status = "failed");
            return Err(AppError::BodyParsingError(e.to_string()));
        }

        if let Err(e) = self
            .qdrant_client
            .create_collection(&kb.id, self.embedding_dimensions as u64)
            .await
        {
            info!(
                kb_id = kb.id.0.to_string(),
                error = e.to_string(),
                status = "failed"
            );
            return Err(e);
        }
        info!(
            kb_id = kb.id.0.to_string(),
            dimensions = self.embedding_dimensions,
            status = "collection_created"
        );

        if let Err(e) = self
            .qdrant_client
            .sync_field_indexes(&kb.id, &parsed_indexes.unwrap_or_default())
            .await
        {
            info!(
                kb_id = kb.id.0.to_string(),
                error = e.to_string(),
                status = "failed"
            );
            return Err(e);
        }

        info!(kb_id = kb.id.0.to_string(), status = "success");
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
        indexes: Option<Value>,
    ) -> Result<KnowledgeBase, AppError> {
        let indexes = indexes.filter(|v| !v.is_null());
        let parsed_indexes = match &indexes {
            Some(v) => Some(KbIndexes::parse(v)?),
            None => None,
        };
        let kb = KnowledgeBase {
            id,
            name,
            description,
            public,
            metadata,
            language: None,
            indexes: parsed_indexes.clone().unwrap_or_default(),
            created_at: chrono::NaiveDateTime::MIN,
            updated_at: chrono::NaiveDateTime::MIN,
        };
        kb.render_description()
            .map_err(|e| AppError::BodyParsingError(e.to_string()))?;

        let result = knowledge_base_repository::update(&self.pool, &kb).await;
        if let Err(e) = &result {
            info!(
                kb_id = kb.id.0.to_string(),
                error = e.to_string(),
                status = "failed"
            );
        }
        let updated = result?;

        if let Err(e) = self
            .qdrant_client
            .sync_field_indexes(&kb.id, &kb.indexes)
            .await
        {
            info!(
                kb_id = kb.id.0.to_string(),
                error = e.to_string(),
                status = "failed"
            );
            return Err(e);
        }

        info!(kb_id = kb.id.0.to_string(), status = "success");
        Ok(updated)
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

        match self.qdrant_client.delete_collection(&id).await {
            Ok(()) => info!(kb_id = id.0.to_string(), status = "collection_deleted"),
            Err(e) => info!(
                kb_id = id.0.to_string(),
                error = e.to_string(),
                status = "collection_delete_failed"
            ),
        }

        Ok(())
    }

    #[instrument(skip(self, metadata, data), fields(data_len = data.len()))]
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
        let text = Self::validate_document_params(&mime_type, &data)?;

        let kb = knowledge_base_repository::get_by_id(&self.pool, kb_id)
            .await?
            .ok_or_else(|| AppError::NotFound("KB not found".into()))?;

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
        if let Err(e) = &result {
            info!(doc_id = id, error = e.to_string(), status = "failed");
        }
        let doc = result?;

        if let Err(e) = self
            .index_document_text(&doc, text, kb.language.as_deref())
            .await
        {
            info!(doc_id = id, error = e.to_string(), status = "index_failed");
            return Err(e);
        }

        info!(doc_id = id, status = "success");
        Ok(doc)
    }

    #[instrument(skip(self))]
    pub async fn list_documents(
        &self,
        kb_id: KnowledgeBaseId,
        filter: Option<&str>,
        cursor: Option<&str>,
        limit: i64,
    ) -> Result<(Vec<Document>, Option<String>), AppError> {
        let result =
            document_repository::list_by_kb_page(&self.pool, kb_id, filter, cursor, limit).await;
        match &result {
            Ok((docs, _)) => info!(doc_count = docs.len(), status = "success"),
            Err(e) => info!(error = e.to_string(), status = "failed"),
        }
        Ok(result?)
    }

    #[instrument(skip(self))]
    pub async fn list_document_versions(
        &self,
        kb_id: KnowledgeBaseId,
        cursor: Option<&str>,
        limit: i64,
    ) -> Result<(Vec<(String, String)>, Option<String>), AppError> {
        let result =
            document_repository::list_versions_page_by_kb(&self.pool, kb_id, cursor, limit).await;
        match &result {
            Ok((items, _)) => info!(version_count = items.len(), status = "success"),
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

    #[instrument(skip(self, metadata, data), fields(data_len = data.len()))]
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
        let text = Self::validate_document_params(&mime_type, &data)?;

        let existing = self
            .get_document(id)
            .await?
            .ok_or_else(|| AppError::NotFound("Document not found".into()))?;

        let kb = knowledge_base_repository::get_by_id(&self.pool, existing.kb_id)
            .await?
            .ok_or_else(|| AppError::NotFound("KB not found".into()))?;

        let doc = existing.with_meta(title, url, version, mime_type, metadata);

        let result = document_repository::update(&self.pool, &doc, &data).await;
        if let Err(e) = &result {
            info!(error = e.to_string(), status = "failed");
        }
        let updated_doc = result?;

        if let Err(e) = self
            .reindex_document_text(&updated_doc, text, kb.language.as_deref())
            .await
        {
            info!(
                doc_id = updated_doc.id.0,
                error = e.to_string(),
                status = "index_failed"
            );
            return Err(e);
        }

        info!(doc_id = updated_doc.id.0, status = "success");
        Ok(updated_doc)
    }

    #[instrument(skip(self, metadata))]
    pub async fn update_document_metadata(
        &self,
        id: &DocumentId,
        title: String,
        url: String,
        metadata: Value,
    ) -> Result<Document, AppError> {
        let doc = self
            .get_document(id)
            .await?
            .ok_or_else(|| AppError::NotFound("Document not found".into()))?;

        let result =
            document_repository::update_metadata(&self.pool, id, &title, &url, &metadata).await;
        if let Err(e) = &result {
            info!(error = e.to_string(), status = "failed");
        }
        let updated_doc = result?;

        if let Err(e) = self
            .qdrant_client
            .update_chunk_metadata(&doc.kb_id, id, &title, &url, &metadata)
            .await
        {
            info!(doc_id = id.0, error = e.to_string(), status = "failed");
            return Err(e);
        }

        info!(doc_id = id.0, status = "success");
        Ok(updated_doc)
    }

    #[instrument(skip(self))]
    pub async fn delete_document(&self, id: &DocumentId) -> Result<(), AppError> {
        let doc = self
            .get_document(id)
            .await?
            .ok_or_else(|| AppError::NotFound("Document not found".into()))?;

        let result = document_repository::delete(&self.pool, id).await;
        match &result {
            Ok(true) => info!(status = "success"),
            Ok(false) => info!(status = "not_found"),
            Err(e) => info!(error = e.to_string(), status = "failed"),
        }
        if !result? {
            return Err(AppError::NotFound("Document not found".into()));
        }

        match self.qdrant_client.delete_by_document(&doc.kb_id, id).await {
            Ok(()) => info!(status = "qdrant_cleanup_success"),
            Err(e) => info!(error = e.to_string(), status = "qdrant_cleanup_failed"),
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

    async fn index_document_text(
        &self,
        document: &Document,
        text: &str,
        language: Option<&str>,
    ) -> Result<usize, AppError> {
        let chunks = self
            .chunking_service
            .chunk_markdown(document.kb_id, document, text);

        let to_embed: HashMap<DocumentChunkId, String> = chunks
            .iter()
            .map(|c| (c.id.clone(), c.content.clone()))
            .collect();

        let embeddings = self.embedding_client.embed_map(to_embed, None).await?;

        let to_upsert: Vec<DocumentChunk> = chunks
            .iter()
            .map(|c| c.with_embedding(embeddings[&c.id].clone()))
            .collect();

        self.qdrant_client
            .upsert_chunks(&document.kb_id, &to_upsert, language)
            .await?;

        info!(
            doc_id = document.id.0,
            chunks_total = to_upsert.len(),
            status = "success"
        );
        Ok(to_upsert.len())
    }

    async fn reindex_document_text(
        &self,
        document: &Document,
        text: &str,
        language: Option<&str>,
    ) -> Result<usize, AppError> {
        let chunks = self
            .chunking_service
            .chunk_markdown(document.kb_id, document, text);

        let old_chunks = self
            .qdrant_client
            .scroll_by_document(&document.kb_id, &document.id)
            .await?;

        let embedding_by_content: HashMap<String, Vec<f32>> = old_chunks
            .into_iter()
            .map(|c| (c.content, c.embedding))
            .collect();

        let to_embed: HashMap<DocumentChunkId, String> = chunks
            .iter()
            .filter(|c| !embedding_by_content.contains_key(&c.content))
            .map(|c| (c.id.clone(), c.content.clone()))
            .collect();

        let new_embeddings = self.embedding_client.embed_map(to_embed, None).await?;

        let to_upsert: Vec<DocumentChunk> = chunks
            .iter()
            .map(|c| {
                let emb = new_embeddings
                    .get(&c.id)
                    .or_else(|| embedding_by_content.get(&c.content))
                    .expect("chunk embedding must be available");
                c.with_embedding(emb.clone())
            })
            .collect();

        self.qdrant_client
            .delete_by_document(&document.kb_id, &document.id)
            .await?;

        self.qdrant_client
            .upsert_chunks(&document.kb_id, &to_upsert, language)
            .await?;

        info!(
            doc_id = document.id.0,
            chunks_total = to_upsert.len(),
            status = "success"
        );
        Ok(to_upsert.len())
    }

    #[instrument(skip(self))]
    pub async fn reindex_document(
        &self,
        id: &DocumentId,
        mode: ReindexMode,
    ) -> Result<ReindexOutcome, AppError> {
        let doc = self
            .get_document(id)
            .await?
            .ok_or_else(|| AppError::NotFound("Document not found".into()))?;

        let kb = knowledge_base_repository::get_by_id(&self.pool, doc.kb_id)
            .await?
            .ok_or_else(|| AppError::NotFound("KB not found".into()))?;

        let language = kb.language.as_deref();

        let (reindexed, chunks) = match mode {
            ReindexMode::Full => {
                let data = self
                    .get_document_data(id)
                    .await?
                    .ok_or_else(|| AppError::NotFound("Document data not found".into()))?;
                let text = Self::validate_document_params(&doc.mime_type, &data.data)?;
                self.qdrant_client
                    .delete_by_document(&doc.kb_id, id)
                    .await?;
                let chunks = self.index_document_text(&doc, text, language).await?;
                (true, chunks)
            },
            ReindexMode::Force => {
                let data = self
                    .get_document_data(id)
                    .await?
                    .ok_or_else(|| AppError::NotFound("Document data not found".into()))?;
                let text = Self::validate_document_params(&doc.mime_type, &data.data)?;
                let chunks = self.reindex_document_text(&doc, text, language).await?;
                (true, chunks)
            },
            ReindexMode::Compare => {
                let actual = self
                    .qdrant_client
                    .scroll_payload_by_document(&doc.kb_id, id)
                    .await?;
                if chunks_up_to_date(&actual, &doc.version) {
                    (false, actual.len())
                } else {
                    let data = self
                        .get_document_data(id)
                        .await?
                        .ok_or_else(|| AppError::NotFound("Document data not found".into()))?;
                    let text = Self::validate_document_params(&doc.mime_type, &data.data)?;
                    let chunks = self.reindex_document_text(&doc, text, language).await?;
                    (true, chunks)
                }
            },
        };

        info!(
            doc_id = id.0,
            mode = ?mode,
            reindexed,
            chunks,
            status = "success"
        );
        Ok(ReindexOutcome { reindexed, chunks })
    }
}

fn chunks_up_to_date(actual: &[DocumentChunk], version: &str) -> bool {
    if actual.is_empty() {
        return false;
    }

    let n = actual.len();
    let mut indices: Vec<usize> = actual.iter().map(|c| c.chunk_index).collect();
    indices.sort_unstable();

    indices.into_iter().eq(0..n)
        && actual
            .iter()
            .all(|c| c.version == version && c.chunk_total == n)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod unit {
        use uuid::Uuid;

        use super::*;
        use crate::domain::models::knowledge_base::KnowledgeBaseId;

        fn make_chunk(index: usize, total: usize, version: &str) -> DocumentChunk {
            DocumentChunk {
                id: DocumentChunkId(Uuid::new_v4()),
                document_id: DocumentId("doc".to_string()),
                kb_id: KnowledgeBaseId(Uuid::new_v4()),
                chunk_index: index,
                chunk_total: total,
                chunk_offset: 0,
                chunk_length: 0,
                document_total_length: 0,
                content: String::new(),
                embedding: Vec::new(),
                version: version.to_string(),
                title: String::new(),
                url: String::new(),
                metadata: serde_json::Value::Null,
            }
        }

        #[test]
        fn up_to_date_accepts_contiguous_matching_version() {
            let chunks = vec![
                make_chunk(0, 3, "v1"),
                make_chunk(1, 3, "v1"),
                make_chunk(2, 3, "v1"),
            ];
            assert!(chunks_up_to_date(&chunks, "v1"));
        }

        #[test]
        fn up_to_date_rejects_empty() {
            assert!(!chunks_up_to_date(&[], "v1"));
        }

        #[test]
        fn up_to_date_rejects_version_mismatch() {
            let chunks = vec![make_chunk(0, 2, "v1"), make_chunk(1, 2, "v2")];
            assert!(!chunks_up_to_date(&chunks, "v1"));
        }

        #[test]
        fn up_to_date_rejects_gap_in_indices() {
            let chunks = vec![make_chunk(0, 3, "v1"), make_chunk(2, 3, "v1")];
            assert!(!chunks_up_to_date(&chunks, "v1"));
        }

        #[test]
        fn up_to_date_rejects_wrong_chunk_total() {
            let chunks = vec![make_chunk(0, 3, "v1"), make_chunk(1, 2, "v1")];
            assert!(!chunks_up_to_date(&chunks, "v1"));
        }
    }

    mod integration {
        use deadpool_diesel::postgres::{Manager, Pool};
        use deadpool_diesel::Runtime;
        use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
        use secrecy::SecretString;
        use tokio::sync::OnceCell;
        use uuid::Uuid;

        use super::*;
        use crate::config::EmbeddingConfig;
        use crate::infra::clients::embedding_client::EmbeddingClient;
        use crate::infra::clients::qdrant_client::QdrantClient;
        use crate::services::chunking_service::ChunkingService;

        const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/");

        const QDRANT_URL: &str = "http://127.0.0.1:6334";
        const EMBEDDING_BASE_URL: &str = "http://127.0.0.1:8888/v1";
        const EMBEDDING_MODEL: &str = "EMBEDDINGS";
        const EMBEDDING_DIMENSIONS: u32 = 1024;

        static TEST_POOL: OnceCell<Pool> = OnceCell::const_new();

        fn test_db_url() -> String {
            std::env::var("TEST_DATABASE_URL")
                .unwrap_or_else(|_| "postgres://chiroo:1234@127.0.0.1/chiroo_tests".to_string())
        }

        async fn get_or_create_test_pool() -> Pool {
            TEST_POOL
                .get_or_init(|| async {
                    let manager = Manager::new(test_db_url(), Runtime::Tokio1);
                    let pool = Pool::builder(manager)
                        .build()
                        .expect("Failed to create test pool");

                    let conn = pool.get().await.expect("Failed to get connection");
                    conn.interact(|conn| conn.run_pending_migrations(MIGRATIONS).map(|_| ()))
                        .await
                        .expect("interact failed")
                        .expect("migrations failed");

                    pool
                })
                .await
                .clone()
        }

        fn make_embedding_client() -> EmbeddingClient {
            let config = EmbeddingConfig {
                base_url: EMBEDDING_BASE_URL.to_string(),
                api_key: SecretString::new("not-needed".to_string().into_boxed_str()),
                model: EMBEDDING_MODEL.to_string(),
                dimensions: EMBEDDING_DIMENSIONS,
            };
            EmbeddingClient::new(&config)
        }

        fn make_qdrant_client(test_id: &str) -> QdrantClient {
            let prefix = format!("test_knowledge_{test_id}");
            QdrantClient::new(QDRANT_URL, None, prefix, std::time::Duration::from_secs(30))
                .expect("Failed to create Qdrant client")
        }

        fn make_chunking_service() -> ChunkingService {
            ChunkingService::new(512)
        }

        #[tokio::test]
        #[ignore = "requires Postgres, Qdrant, and embedding service"]
        async fn create_and_index_document() {
            let pool = get_or_create_test_pool().await;
            let qdrant = make_qdrant_client("create_and_index");
            let embedding = make_embedding_client();
            let chunking = make_chunking_service();

            let service = KnowledgeService::new(
                pool.clone(),
                chunking,
                qdrant.clone(),
                embedding,
                EMBEDDING_DIMENSIONS,
            );

            let kb = service
                .create_kb(
                    "test".into(),
                    "desc".into(),
                    false,
                    serde_json::json!({}),
                    None,
                    None,
                )
                .await
                .expect("create_kb");

            let doc_id = Uuid::new_v4().to_string();
            let text = "# Hello\n\nThis is a test document with enough text to produce chunks.";
            let doc = service
                .create_document(
                    doc_id.clone(),
                    kb.id,
                    "Test Doc".into(),
                    "https://example.com/test".into(),
                    "v1".into(),
                    MimeType::TextPlain,
                    serde_json::json!({}),
                    text.as_bytes().to_vec(),
                )
                .await
                .expect("create_document");

            assert_eq!(doc.id.0, doc_id);

            let chunks = qdrant
                .scroll_by_document(&kb.id, &doc.id)
                .await
                .expect("scroll_by_document");

            assert!(!chunks.is_empty(), "document should have indexed chunks");

            service.delete_kb(kb.id).await.expect("delete_kb");
        }

        #[tokio::test]
        #[ignore = "requires Postgres, Qdrant, and embedding service"]
        async fn update_document_reindexes() {
            let pool = get_or_create_test_pool().await;
            let qdrant = make_qdrant_client("update_reindexes");
            let embedding = make_embedding_client();
            let chunking = make_chunking_service();

            let service = KnowledgeService::new(
                pool.clone(),
                chunking,
                qdrant.clone(),
                embedding,
                EMBEDDING_DIMENSIONS,
            );

            let kb = service
                .create_kb(
                    "test".into(),
                    "desc".into(),
                    false,
                    serde_json::json!({}),
                    None,
                    None,
                )
                .await
                .expect("create_kb");

            let doc_id = Uuid::new_v4().to_string();
            let old_text = "Old document content that is long enough to produce a chunk here.";

            service
                .create_document(
                    doc_id.clone(),
                    kb.id,
                    "Test Doc".into(),
                    "https://example.com/test".into(),
                    "v1".into(),
                    MimeType::TextPlain,
                    serde_json::json!({}),
                    old_text.as_bytes().to_vec(),
                )
                .await
                .expect("create_document");

            let new_text = "Completely different and new document content that changes the chunks.";

            service
                .update_document(
                    &DocumentId(doc_id.clone()),
                    "Updated Doc".into(),
                    "https://example.com/updated".into(),
                    "v2".into(),
                    MimeType::TextPlain,
                    serde_json::json!({}),
                    new_text.as_bytes().to_vec(),
                )
                .await
                .expect("update_document");

            let chunks = qdrant
                .scroll_by_document(&kb.id, &DocumentId(doc_id))
                .await
                .expect("scroll_by_document");

            assert!(!chunks.is_empty(), "updated document should have chunks");
            let all_content: String = chunks
                .iter()
                .map(|c| c.content.as_str())
                .collect::<Vec<_>>()
                .join("");
            assert!(
                all_content.contains("Completely different"),
                "chunks should contain new content, got: {all_content}"
            );

            service.delete_kb(kb.id).await.expect("delete_kb");
        }

        #[tokio::test]
        #[ignore = "requires Postgres, Qdrant, and embedding service"]
        async fn delete_document_cleans_qdrant() {
            let pool = get_or_create_test_pool().await;
            let qdrant = make_qdrant_client("delete_cleans");
            let embedding = make_embedding_client();
            let chunking = make_chunking_service();

            let service = KnowledgeService::new(
                pool.clone(),
                chunking,
                qdrant.clone(),
                embedding,
                EMBEDDING_DIMENSIONS,
            );

            let kb = service
                .create_kb(
                    "test".into(),
                    "desc".into(),
                    false,
                    serde_json::json!({}),
                    None,
                    None,
                )
                .await
                .expect("create_kb");

            let doc_id = Uuid::new_v4().to_string();
            let text = "Document text that produces some chunks for indexing.";

            service
                .create_document(
                    doc_id.clone(),
                    kb.id,
                    "Test Doc".into(),
                    "https://example.com/test".into(),
                    "v1".into(),
                    MimeType::TextPlain,
                    serde_json::json!({}),
                    text.as_bytes().to_vec(),
                )
                .await
                .expect("create_document");

            let chunks_before = qdrant
                .scroll_by_document(&kb.id, &DocumentId(doc_id.clone()))
                .await
                .expect("scroll_before_delete");
            assert!(
                !chunks_before.is_empty(),
                "should have chunks before delete"
            );

            service
                .delete_document(&DocumentId(doc_id.clone()))
                .await
                .expect("delete_document");

            let doc_after = service
                .get_document(&DocumentId(doc_id.clone()))
                .await
                .expect("get_document");
            assert!(doc_after.is_none(), "document should be deleted from DB");

            let chunks_after = qdrant
                .scroll_by_document(&kb.id, &DocumentId(doc_id))
                .await
                .expect("scroll_after_delete");
            assert!(
                chunks_after.is_empty(),
                "Qdrant should have no chunks after document delete"
            );

            service.delete_kb(kb.id).await.expect("delete_kb");
        }

        #[tokio::test]
        #[ignore = "requires Postgres, Qdrant, and embedding service"]
        async fn empty_text_document() {
            let pool = get_or_create_test_pool().await;
            let qdrant = make_qdrant_client("empty_text");
            let embedding = make_embedding_client();
            let chunking = make_chunking_service();

            let service = KnowledgeService::new(
                pool.clone(),
                chunking,
                qdrant.clone(),
                embedding,
                EMBEDDING_DIMENSIONS,
            );

            let kb = service
                .create_kb(
                    "test".into(),
                    "desc".into(),
                    false,
                    serde_json::json!({}),
                    None,
                    None,
                )
                .await
                .expect("create_kb");

            let doc_id = Uuid::new_v4().to_string();
            let result = service
                .create_document(
                    doc_id.clone(),
                    kb.id,
                    "Empty Doc".into(),
                    "https://example.com/empty".into(),
                    "v1".into(),
                    MimeType::TextPlain,
                    serde_json::json!({}),
                    b"".to_vec(),
                )
                .await;

            assert!(result.is_err(), "empty document content should be rejected");

            let doc = service
                .get_document(&DocumentId(doc_id))
                .await
                .expect("get_document");
            assert!(
                doc.is_none(),
                "rejected document should not be stored in DB"
            );

            service.delete_kb(kb.id).await.expect("delete_kb");
        }

        #[tokio::test]
        #[ignore = "requires Postgres, Qdrant, and embedding service"]
        async fn update_document_metadata_only() {
            let pool = get_or_create_test_pool().await;
            let qdrant = make_qdrant_client("metadata_only");
            let embedding = make_embedding_client();
            let chunking = make_chunking_service();

            let service = KnowledgeService::new(
                pool.clone(),
                chunking,
                qdrant.clone(),
                embedding,
                EMBEDDING_DIMENSIONS,
            );

            let kb = service
                .create_kb(
                    "test".into(),
                    "desc".into(),
                    false,
                    serde_json::json!({}),
                    None,
                    None,
                )
                .await
                .expect("create_kb");

            let doc_id = Uuid::new_v4().to_string();
            let text = "Metadata update test content that forms a chunk for indexing.";
            let metadata = serde_json::json!({ "author": "Alice" });

            service
                .create_document(
                    doc_id.clone(),
                    kb.id,
                    "Original Title".into(),
                    "https://example.com/original".into(),
                    "v1".into(),
                    MimeType::TextPlain,
                    metadata,
                    text.as_bytes().to_vec(),
                )
                .await
                .expect("create_document");

            let before = qdrant
                .scroll_by_document(&kb.id, &DocumentId(doc_id.clone()))
                .await
                .expect("scroll_before");
            assert!(!before.is_empty(), "should have chunks before update");
            let before_embeddings: Vec<Vec<f32>> =
                before.iter().map(|c| c.embedding.clone()).collect();
            let before_content: Vec<String> = before.iter().map(|c| c.content.clone()).collect();

            let updated = service
                .update_document_metadata(
                    &DocumentId(doc_id.clone()),
                    "Updated Title".into(),
                    "https://example.com/updated".into(),
                    serde_json::json!({ "author": "Bob" }),
                )
                .await
                .expect("update_document_metadata");

            assert_eq!(updated.title, "Updated Title");
            assert_eq!(updated.url, "https://example.com/updated");
            assert_eq!(updated.version, "v1", "version must stay unchanged");

            let after = qdrant
                .scroll_by_document(&kb.id, &DocumentId(doc_id))
                .await
                .expect("scroll_after");

            assert_eq!(after.len(), before_content.len(), "chunk count unchanged");

            let after_content: Vec<String> = after.iter().map(|c| c.content.clone()).collect();
            assert_eq!(after_content, before_content, "content must stay unchanged");

            let after_embeddings: Vec<Vec<f32>> =
                after.iter().map(|c| c.embedding.clone()).collect();
            assert_eq!(
                after_embeddings, before_embeddings,
                "embeddings must stay unchanged"
            );

            assert!(
                after.iter().all(|c| c.title == "Updated Title"),
                "all chunks must have updated title"
            );
            assert!(
                after.iter().all(|c| c.url == "https://example.com/updated"),
                "all chunks must have updated url"
            );

            service.delete_kb(kb.id).await.expect("delete_kb");
        }

        #[tokio::test]
        #[ignore = "requires Postgres, Qdrant, and embedding service"]
        async fn create_and_delete_kb() {
            let pool = get_or_create_test_pool().await;
            let qdrant = make_qdrant_client("create_delete_kb");
            let embedding = make_embedding_client();
            let chunking = make_chunking_service();

            let service = KnowledgeService::new(
                pool.clone(),
                chunking,
                qdrant.clone(),
                embedding,
                EMBEDDING_DIMENSIONS,
            );

            let kb = service
                .create_kb(
                    "test".into(),
                    "desc".into(),
                    false,
                    serde_json::json!({}),
                    None,
                    None,
                )
                .await
                .expect("create_kb");

            let doc_id = Uuid::new_v4().to_string();
            service
                .create_document(
                    doc_id.clone(),
                    kb.id,
                    "Doc".into(),
                    "https://example.com/doc".into(),
                    "v1".into(),
                    MimeType::TextPlain,
                    serde_json::json!({}),
                    b"some text that forms a chunk".to_vec(),
                )
                .await
                .expect("create_document");

            service.delete_kb(kb.id).await.expect("delete_kb");

            let doc = service
                .get_document(&DocumentId(doc_id))
                .await
                .expect("get_document");
            assert!(doc.is_none(), "document should be cascade-deleted with KB");
        }

        #[tokio::test]
        #[ignore = "requires Postgres, Qdrant, and embedding service"]
        async fn reindex_document_modes() {
            let pool = get_or_create_test_pool().await;
            let qdrant = make_qdrant_client("reindex_modes");
            let embedding = make_embedding_client();
            let chunking = make_chunking_service();

            let service = KnowledgeService::new(
                pool.clone(),
                chunking,
                qdrant.clone(),
                embedding,
                EMBEDDING_DIMENSIONS,
            );

            let kb = service
                .create_kb(
                    "test".into(),
                    "desc".into(),
                    false,
                    serde_json::json!({}),
                    None,
                    None,
                )
                .await
                .expect("create_kb");

            let doc_id = DocumentId(Uuid::new_v4().to_string());
            let text = "Reindex test content that produces some chunks for indexing.";
            service
                .create_document(
                    doc_id.0.clone(),
                    kb.id,
                    "Test Doc".into(),
                    "https://example.com/test".into(),
                    "v1".into(),
                    MimeType::TextPlain,
                    serde_json::json!({}),
                    text.as_bytes().to_vec(),
                )
                .await
                .expect("create_document");

            // Compare on an up-to-date document: no reindex.
            let outcome = service
                .reindex_document(&doc_id, ReindexMode::Compare)
                .await
                .expect("compare up-to-date");
            assert!(!outcome.reindexed, "up-to-date document must not reindex");

            // Simulate a failed save: drop all chunks from Qdrant.
            qdrant
                .delete_by_document(&kb.id, &doc_id)
                .await
                .expect("delete chunks");

            let outcome = service
                .reindex_document(&doc_id, ReindexMode::Compare)
                .await
                .expect("compare after delete");
            assert!(outcome.reindexed, "missing chunks must trigger reindex");
            assert!(outcome.chunks > 0, "reindexed chunks must be non-empty");

            // Force always reindexes.
            let outcome = service
                .reindex_document(&doc_id, ReindexMode::Force)
                .await
                .expect("force");
            assert!(outcome.reindexed, "force must always reindex");

            // Full always reindexes.
            let outcome = service
                .reindex_document(&doc_id, ReindexMode::Full)
                .await
                .expect("full");
            assert!(outcome.reindexed, "full must always reindex");
            assert!(outcome.chunks > 0, "full reindex must produce chunks");

            service.delete_kb(kb.id).await.expect("delete_kb");
        }

        #[tokio::test]
        #[ignore = "requires Postgres"]
        async fn create_kb_rejects_invalid_indexes() {
            let pool = get_or_create_test_pool().await;
            let service = KnowledgeService::new(
                pool.clone(),
                make_chunking_service(),
                make_qdrant_client("kb_index_validation"),
                make_embedding_client(),
                EMBEDDING_DIMENSIONS,
            );

            let cases: Vec<&str> = vec![
                r#"{"a.b": {"type": "keyword"}}"#,  // field name with a dot
                r#"{"x": {"type": "not-a-type"}}"#, // unknown type
                r#"{"x": "keyword"}"#,              // spec is not an object
                r#"[ "keyword" ]"#,                 // not an object at all
                r#"{"": {"type": "keyword"}}"#,     // empty field name
            ];
            for raw in cases {
                let indexes = serde_json::from_str::<Value>(raw).expect("valid JSON");
                let err = service
                    .create_kb(
                        "bad".into(),
                        "desc".into(),
                        false,
                        serde_json::json!({}),
                        None,
                        Some(indexes),
                    )
                    .await
                    .expect_err("invalid indexes must be rejected");
                assert!(
                    matches!(err, AppError::BadRequest(_)),
                    "case {raw}: expected BadRequest, got {err}"
                );
            }
        }

        #[tokio::test]
        #[ignore = "requires Postgres, Qdrant, and embedding service"]
        async fn update_kb_replaces_indexes_full_replace_semantics() {
            let pool = get_or_create_test_pool().await;
            let service = KnowledgeService::new(
                pool.clone(),
                make_chunking_service(),
                make_qdrant_client("kb_index_replace"),
                make_embedding_client(),
                EMBEDDING_DIMENSIONS,
            );

            let kb = service
                .create_kb(
                    "idx".into(),
                    "desc".into(),
                    false,
                    serde_json::json!({}),
                    None,
                    Some(serde_json::json!({ "section": { "type": "keyword" } })),
                )
                .await
                .expect("create_kb");
            assert!(
                service
                    .get_kb(kb.id)
                    .await
                    .expect("get_kb")
                    .unwrap()
                    .indexes
                    .find("section")
                    .is_some(),
                "created KB should carry the section index"
            );

            // Full replace: the new set exactly replaces the old one.
            let updated = service
                .update_kb(
                    kb.id,
                    "idx".into(),
                    "desc".into(),
                    false,
                    serde_json::json!({}),
                    Some(serde_json::json!({ "priority": { "type": "integer" } })),
                )
                .await
                .expect("update_kb");
            assert!(updated.indexes.find("priority").is_some());
            assert!(
                updated.indexes.find("section").is_none(),
                "replaced index must be gone"
            );

            // Omitting indexes (None) clears them entirely.
            let cleared = service
                .update_kb(
                    kb.id,
                    "idx".into(),
                    "desc".into(),
                    false,
                    serde_json::json!({}),
                    None,
                )
                .await
                .expect("update_kb");
            assert!(cleared.indexes.is_empty(), "indexes must be cleared");
            assert!(
                service
                    .get_kb(kb.id)
                    .await
                    .expect("get_kb")
                    .unwrap()
                    .indexes
                    .is_empty(),
                "cleared indexes must persist"
            );

            service.delete_kb(kb.id).await.expect("delete_kb");
        }

        #[tokio::test]
        #[ignore = "requires Postgres, Qdrant, and embedding service"]
        async fn update_kb_rejects_invalid_indexes_and_keeps_old() {
            let pool = get_or_create_test_pool().await;
            let service = KnowledgeService::new(
                pool.clone(),
                make_chunking_service(),
                make_qdrant_client("kb_index_update_reject"),
                make_embedding_client(),
                EMBEDDING_DIMENSIONS,
            );

            let kb = service
                .create_kb(
                    "idx".into(),
                    "desc".into(),
                    false,
                    serde_json::json!({}),
                    None,
                    Some(serde_json::json!({ "section": { "type": "keyword" } })),
                )
                .await
                .expect("create_kb");

            let err = service
                .update_kb(
                    kb.id,
                    "idx".into(),
                    "desc".into(),
                    false,
                    serde_json::json!({}),
                    Some(serde_json::json!({ "a.b": { "type": "keyword" } })),
                )
                .await
                .expect_err("invalid indexes must be rejected");
            assert!(matches!(err, AppError::BadRequest(_)));

            let unchanged = service.get_kb(kb.id).await.expect("get_kb").unwrap();
            assert!(
                unchanged.indexes.find("section").is_some(),
                "failed update must not touch the existing indexes"
            );

            service.delete_kb(kb.id).await.expect("delete_kb");
        }
    }
}
