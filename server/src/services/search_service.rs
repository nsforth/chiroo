use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use futures::future::join_all;
use tracing::{info, instrument};

use crate::domain::models::document_chunk::{DocumentChunkId, SearchedChunk};
use crate::domain::models::knowledge_base::KnowledgeBase;
use crate::errors::AppError;
use crate::infra::clients::embedding_client::EmbeddingClient;
use crate::infra::clients::qdrant_client::QdrantClient;
use crate::infra::clients::rerank_client::RerankClient;
use crate::utils::LogView;

#[derive(Clone)]
pub struct SearchService {
    embedding_client: EmbeddingClient,
    qdrant_client: QdrantClient,
    rerank_client: RerankClient,
    score_threshold: f32,
    candidate_limit: u64,
}

impl SearchService {
    pub fn new(
        embedding_client: EmbeddingClient,
        qdrant_client: QdrantClient,
        rerank_client: RerankClient,
        score_threshold: f32,
        candidate_limit: u64,
    ) -> Self {
        Self {
            embedding_client,
            qdrant_client,
            rerank_client,
            score_threshold,
            candidate_limit,
        }
    }

    #[instrument(skip(self, kbs), fields(kbs = %kbs.log_view()))]
    pub async fn search_one(
        &self,
        query: &str,
        kbs: &[KnowledgeBase],
        limit: usize,
    ) -> Result<Vec<SearchedChunk>, AppError> {
        let result = self
            .search_impl(vec![query.to_string()], kbs, None, limit)
            .await;
        match &result {
            Ok(chunks) => info!(chunks = chunks.len(), status = "success"),
            Err(e) => info!(error = e.to_string(), status = "failed"),
        }
        result
    }

    #[instrument(skip(self, kbs), fields(kbs = %kbs.log_view()))]
    pub async fn search_many(
        &self,
        queries: Vec<String>,
        kbs: &[KnowledgeBase],
        limit: usize,
    ) -> Result<Vec<SearchedChunk>, AppError> {
        let result = self.search_impl(queries, kbs, None, limit).await;
        match &result {
            Ok(chunks) => info!(chunks = chunks.len(), status = "success"),
            Err(e) => info!(error = e.to_string(), status = "failed"),
        }
        result
    }

    #[instrument(skip(self, kbs), fields(kbs = %kbs.log_view()))]
    pub async fn search_grouped(
        &self,
        queries: Vec<String>,
        kbs: &[KnowledgeBase],
        per_query_limit: usize,
        total_limit: usize,
    ) -> Result<Vec<SearchedChunk>, AppError> {
        let result = self
            .search_impl(queries, kbs, Some(per_query_limit), total_limit)
            .await;
        match &result {
            Ok(chunks) => info!(chunks = chunks.len(), status = "success"),
            Err(e) => info!(error = e.to_string(), status = "failed"),
        }
        result
    }

    async fn search_impl(
        &self,
        queries: Vec<String>,
        kbs: &[KnowledgeBase],
        per_query_limit: Option<usize>,
        total_limit: usize,
    ) -> Result<Vec<SearchedChunk>, AppError> {
        let unique = dedup_queries(queries);
        if unique.is_empty() {
            return Ok(Vec::new());
        }

        let query_map: HashMap<String, String> =
            unique.iter().map(|q| (q.clone(), q.clone())).collect();
        let embeddings = self.embedding_client.embed_map(query_map, None).await?;

        let kbs = kbs.to_vec();

        let candidate_limit = self.candidate_limit;
        let threshold = self.score_threshold;

        let mut futures = Vec::new();
        for query in &unique {
            let dense_vector = embeddings.get(query).cloned().unwrap_or_default();
            let qdrant_client = self.qdrant_client.clone();
            let rerank_client = self.rerank_client.clone();
            let kbs = kbs.clone();
            let query = query.clone();

            futures.push(async move {
                search_one_query(
                    &qdrant_client,
                    &rerank_client,
                    &kbs,
                    &query,
                    dense_vector,
                    candidate_limit,
                    threshold,
                )
                .await
            });
        }

        let mut all = Vec::new();
        for result in join_all(futures).await {
            let mut chunks = result?;
            if let Some(limit) = per_query_limit {
                chunks.truncate(limit);
            }
            all.extend(chunks);
        }

        Ok(merge_top(all, total_limit))
    }
}

async fn search_one_query(
    qdrant_client: &QdrantClient,
    rerank_client: &RerankClient,
    kbs: &[KnowledgeBase],
    query: &str,
    dense_vector: Vec<f32>,
    candidate_limit: u64,
    threshold: f32,
) -> Result<Vec<SearchedChunk>, AppError> {
    let chunks = search_in_kbs(qdrant_client, kbs, query, dense_vector, candidate_limit).await?;
    let chunks = dedup_chunks(chunks);

    let contents: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();
    let reranked = rerank_client.rerank(query, &contents, None, None).await?;
    let scores: Vec<(usize, f32)> = reranked
        .into_iter()
        .map(|r| (r.index as usize, r.relevance_score))
        .collect();

    Ok(filter_chunks(chunks, scores, threshold))
}

async fn search_in_kbs(
    qdrant_client: &QdrantClient,
    kbs: &[KnowledgeBase],
    query: &str,
    dense_vector: Vec<f32>,
    candidate_limit: u64,
) -> Result<Vec<SearchedChunk>, AppError> {
    let futures = kbs.iter().map(|kb| {
        let client = qdrant_client.clone();
        let kb_id = kb.id;
        let language = kb.language.clone();
        let dense_vector = dense_vector.clone();
        let query = query.to_string();
        async move {
            client
                .search_hybrid(
                    &kb_id,
                    dense_vector,
                    &query,
                    candidate_limit,
                    language.as_deref(),
                )
                .await
        }
    });

    let mut chunks = Vec::new();
    for result in join_all(futures).await {
        chunks.extend(result?);
    }
    Ok(chunks)
}

fn dedup_chunks(chunks: Vec<SearchedChunk>) -> Vec<SearchedChunk> {
    let mut seen = HashSet::new();
    chunks
        .into_iter()
        .filter(|chunk| seen.insert(chunk.id.clone()))
        .collect()
}

fn dedup_queries(queries: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    queries
        .into_iter()
        .filter(|query| seen.insert(query.clone()))
        .collect()
}

fn filter_chunks(
    chunks: Vec<SearchedChunk>,
    scores: Vec<(usize, f32)>,
    threshold: f32,
) -> Vec<SearchedChunk> {
    scores
        .into_iter()
        .filter(|(_, score)| *score >= threshold)
        .filter_map(|(index, score)| chunks.get(index).map(|chunk| chunk.with_score(score)))
        .collect()
}

fn merge_top(chunks: Vec<SearchedChunk>, limit: usize) -> Vec<SearchedChunk> {
    let mut best: HashMap<DocumentChunkId, SearchedChunk> = HashMap::new();
    for chunk in chunks {
        match best.entry(chunk.id.clone()) {
            Entry::Occupied(mut e) => {
                if chunk.score > e.get().score {
                    e.insert(chunk);
                }
            },
            Entry::Vacant(e) => {
                e.insert(chunk);
            },
        }
    }

    let mut ranked: Vec<SearchedChunk> = best.into_values().collect();
    ranked.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ranked.truncate(limit);
    ranked
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::domain::models::document::DocumentId;
    use crate::domain::models::document_chunk::DocumentChunkId;
    use crate::domain::models::knowledge_base::KnowledgeBaseId;

    fn make_chunk(content: &str) -> SearchedChunk {
        SearchedChunk {
            id: DocumentChunkId(Uuid::new_v4()),
            document_id: DocumentId("doc".to_string()),
            kb_id: KnowledgeBaseId(Uuid::new_v4()),
            chunk_index: 0,
            chunk_total: 1,
            chunk_offset: 0,
            chunk_length: content.len(),
            document_total_length: content.len(),
            content: content.to_string(),
            version: "v1".to_string(),
            title: "title".to_string(),
            url: "url".to_string(),
            metadata: serde_json::json!({}),
            score: 0.0,
        }
    }

    #[test]
    fn filter_chunks_filters_below_threshold() {
        let chunks = vec![make_chunk("a"), make_chunk("b"), make_chunk("c")];
        let scores = vec![(0, 0.9), (1, 0.4), (2, 0.6)];
        let selected = filter_chunks(chunks, scores, 0.5);
        let contents: Vec<&str> = selected.iter().map(|c| c.content.as_str()).collect();
        assert_eq!(contents, vec!["a", "c"]);
        assert!(selected.iter().all(|c| c.score >= 0.5));
    }

    #[test]
    fn merge_top_keeps_higher_score_for_duplicate_chunk() {
        let shared = make_chunk("shared");
        let shared_high = shared.with_score(0.9);
        let shared_low = shared.with_score(0.4);
        let other = make_chunk("other").with_score(0.7);

        let merged = merge_top(vec![shared_low, shared_high, other.clone()], 10);

        assert_eq!(merged.len(), 2, "duplicate chunk must appear exactly once");
        let shared_in = merged.iter().find(|c| c.content == "shared").unwrap();
        assert_eq!(shared_in.score, 0.9, "the higher score must win");
        assert!(merged.iter().any(|c| c.content == "other"));
    }

    #[test]
    fn merge_top_truncates_globally() {
        let chunks: Vec<SearchedChunk> = (0..15)
            .map(|i| make_chunk(&format!("c{i}")).with_score(i as f32 * 0.1))
            .collect();

        let merged = merge_top(chunks, 4);

        assert_eq!(merged.len(), 4, "global limit must cap the combined result");
        for pair in merged.windows(2) {
            assert!(
                pair[0].score >= pair[1].score,
                "chunks must be globally sorted"
            );
        }
    }

    #[test]
    fn dedup_chunks_removes_duplicates_preserving_order() {
        let first = make_chunk("first");
        let second = make_chunk("second");
        let third = make_chunk("third");
        let duplicate = first.clone();
        let chunks = vec![first.clone(), second, third, duplicate];
        let deduped = dedup_chunks(chunks);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0].id, first.id);
    }

    #[test]
    fn dedup_queries_removes_duplicates() {
        let queries = vec![
            "a".to_string(),
            "b".to_string(),
            "a".to_string(),
            "c".to_string(),
            "b".to_string(),
        ];
        let unique = dedup_queries(queries);
        assert_eq!(
            unique,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    mod integration {
        use deadpool_diesel::postgres::{Manager, Pool};
        use deadpool_diesel::Runtime;
        use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
        use secrecy::SecretString;
        use tokio::sync::OnceCell;
        use uuid::Uuid;

        use super::*;
        use crate::config::{EmbeddingConfig, RerankConfig};
        use crate::domain::models::document::MimeType;
        use crate::infra::clients::embedding_client::EmbeddingClient;
        use crate::infra::clients::qdrant_client::QdrantClient;
        use crate::infra::clients::rerank_client::RerankClient;
        use crate::services::chunking_service::ChunkingService;
        use crate::services::knowledge_service::KnowledgeService;

        const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/");

        const QDRANT_URL: &str = "http://localhost:6334";
        const EMBEDDING_BASE_URL: &str = "http://localhost:11434/v1";
        const EMBEDDING_MODEL: &str = "EMBEDDINGS";
        const EMBEDDING_DIMENSIONS: u32 = 1024;
        const RERANK_BASE_URL: &str = "http://localhost:11434/v1";
        const RERANK_MODEL: &str = "RERANKER";

        static TEST_POOL: OnceCell<Pool> = OnceCell::const_new();

        fn test_db_url() -> String {
            std::env::var("TEST_DATABASE_URL")
                .unwrap_or_else(|_| "postgres://chiroo:1234@localhost/chiroo_tests".to_string())
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

        fn make_rerank_client() -> RerankClient {
            let config = RerankConfig {
                base_url: RERANK_BASE_URL.to_string(),
                api_key: SecretString::new("".to_string().into_boxed_str()),
                model: RERANK_MODEL.to_string(),
            };
            RerankClient::new(&config).expect("Failed to create Rerank client")
        }

        fn make_qdrant_client(test_id: &str) -> QdrantClient {
            let prefix = format!("test_search_{test_id}");
            QdrantClient::new(QDRANT_URL, None, prefix, std::time::Duration::from_secs(30))
                .expect("Failed to create Qdrant client")
        }

        fn make_chunking_service() -> ChunkingService {
            ChunkingService::new(512)
        }

        #[tokio::test]
        #[ignore = "requires Postgres, Qdrant, and embedding service"]
        async fn search_returns_reranked_chunks() {
            let pool = get_or_create_test_pool().await;
            let qdrant = make_qdrant_client("search");
            let embedding = make_embedding_client();
            let rerank = make_rerank_client();
            let chunking = make_chunking_service();

            let knowledge_service = KnowledgeService::new(
                pool.clone(),
                chunking.clone(),
                qdrant.clone(),
                embedding.clone(),
                EMBEDDING_DIMENSIONS,
            );

            let search_service = SearchService::new(embedding, qdrant.clone(), rerank, 0.5, 30);

            let kb = knowledge_service
                .create_kb(
                    "test".into(),
                    "desc".into(),
                    false,
                    serde_json::json!({}),
                    None,
                )
                .await
                .expect("create_kb");

            let doc_id = Uuid::new_v4().to_string();
            let text = "The Eiffel Tower is a wrought-iron lattice tower on the Champ de Mars \
                        in Paris, France. Paris is the capital city of France and is known for its \
                        art, fashion, gastronomy and culture. The city has a population of over \
                        two million people.";
            knowledge_service
                .create_document(
                    doc_id,
                    kb.id,
                    "Paris Facts".into(),
                    "https://example.com/paris".into(),
                    "v1".into(),
                    MimeType::TextPlain,
                    serde_json::json!({}),
                    text.as_bytes().to_vec(),
                )
                .await
                .expect("create_document");

            let chunks = search_service
                .search_many(
                    vec!["What is the capital of France?".to_string()],
                    &[kb.clone()],
                    10,
                )
                .await
                .expect("search");

            assert!(!chunks.is_empty(), "search should return chunks");
            assert!(
                chunks.iter().all(|c| c.kb_id == kb.id),
                "all chunks should belong to the searched KB"
            );
            assert!(
                chunks.iter().all(|c| c.score >= 0.5),
                "all chunks should pass the reranker threshold"
            );

            knowledge_service.delete_kb(kb.id).await.expect("delete_kb");
        }
    }
}
