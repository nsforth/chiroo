use std::collections::{HashMap, HashSet};

use futures::future::join_all;
use tracing::{info, instrument};

use crate::config::SearchConfig;
use crate::domain::models::document_chunk::SearchedChunk;
use crate::domain::models::knowledge_base::KnowledgeBase;
use crate::errors::AppError;
use crate::infra::clients::embedding_client::EmbeddingClient;
use crate::infra::clients::qdrant_client::QdrantClient;
use crate::infra::clients::rerank_client::RerankClient;

#[derive(Clone)]
pub struct SearchService {
    embedding_client: EmbeddingClient,
    qdrant_client: QdrantClient,
    rerank_client: RerankClient,
    config: SearchConfig,
}

impl SearchService {
    pub fn new(
        embedding_client: EmbeddingClient,
        qdrant_client: QdrantClient,
        rerank_client: RerankClient,
        config: SearchConfig,
    ) -> Self {
        Self {
            embedding_client,
            qdrant_client,
            rerank_client,
            config,
        }
    }

    #[instrument(skip(self, queries, kbs))]
    pub async fn search(
        &self,
        queries: Vec<String>,
        kbs: &[KnowledgeBase],
    ) -> Result<HashMap<String, Vec<SearchedChunk>>, AppError> {
        let result = self.run_search(queries, kbs).await;
        match &result {
            Ok(map) => info!(queries = map.len(), status = "success"),
            Err(e) => info!(error = e.to_string(), status = "failed"),
        }
        result
    }

    async fn run_search(
        &self,
        queries: Vec<String>,
        kbs: &[KnowledgeBase],
    ) -> Result<HashMap<String, Vec<SearchedChunk>>, AppError> {
        let unique = dedup_queries(queries);
        if unique.is_empty() {
            return Ok(HashMap::new());
        }

        let query_map: HashMap<String, String> =
            unique.iter().map(|q| (q.clone(), q.clone())).collect();
        let embeddings = self.embedding_client.embed_map(query_map, None).await?;

        let kbs = kbs.to_vec();

        let candidate_limit = self.config.candidate_limit;
        let threshold = self.config.score_threshold;
        let limit = self.config.limit;

        let mut futures = Vec::new();
        for query in &unique {
            let dense_vector = embeddings.get(query).cloned().unwrap_or_default();
            let qdrant_client = self.qdrant_client.clone();
            let rerank_client = self.rerank_client.clone();
            let kbs = kbs.clone();
            let query = query.clone();

            futures.push(async move {
                let result = search_one_query(
                    &qdrant_client,
                    &rerank_client,
                    &kbs,
                    &query,
                    dense_vector,
                    candidate_limit,
                    threshold,
                    limit,
                )
                .await;
                (query, result)
            });
        }

        let mut output = HashMap::new();
        for (query, result) in join_all(futures).await {
            output.insert(query, result?);
        }
        Ok(output)
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
    limit: usize,
) -> Result<Vec<SearchedChunk>, AppError> {
    let chunks = search_in_kbs(qdrant_client, kbs, query, dense_vector, candidate_limit).await?;
    let chunks = dedup_chunks(chunks);

    let contents: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();
    let reranked = rerank_client.rerank(query, &contents, None, None).await?;
    let scores: Vec<(usize, f32)> = reranked
        .into_iter()
        .map(|r| (r.index as usize, r.relevance_score))
        .collect();

    Ok(select_chunks(chunks, scores, threshold, limit))
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

fn select_chunks(
    chunks: Vec<SearchedChunk>,
    scores: Vec<(usize, f32)>,
    threshold: f32,
    limit: usize,
) -> Vec<SearchedChunk> {
    let mut selected: Vec<SearchedChunk> = scores
        .into_iter()
        .filter(|(_, score)| *score >= threshold)
        .filter_map(|(index, score)| chunks.get(index).map(|chunk| chunk.with_score(score)))
        .collect();

    selected.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    selected.truncate(limit);
    selected
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
    fn select_chunks_filters_below_threshold() {
        let chunks = vec![make_chunk("a"), make_chunk("b"), make_chunk("c")];
        let scores = vec![(0, 0.9), (1, 0.4), (2, 0.6)];
        let selected = select_chunks(chunks, scores, 0.5, 10);
        let contents: Vec<&str> = selected.iter().map(|c| c.content.as_str()).collect();
        assert_eq!(contents, vec!["a", "c"]);
        assert!(selected.iter().all(|c| c.score >= 0.5));
    }

    #[test]
    fn select_chunks_sorts_descending_and_truncates_to_limit() {
        let chunks: Vec<SearchedChunk> = (0..12).map(|i| make_chunk(&format!("c{i}"))).collect();
        let scores: Vec<(usize, f32)> = (0..12).map(|i| (i, i as f32 * 0.1)).collect();
        let selected = select_chunks(chunks, scores, 0.0, 10);
        assert_eq!(selected.len(), 10);
        assert_eq!(selected[0].content, "c11");
        assert_eq!(selected[9].content, "c2");
        for pair in selected.windows(2) {
            assert!(pair[0].score >= pair[1].score);
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
        use crate::config::{EmbeddingConfig, RerankConfig, SearchConfig};
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
            QdrantClient::new(QDRANT_URL, None, prefix).expect("Failed to create Qdrant client")
        }

        fn make_chunking_service() -> ChunkingService {
            ChunkingService::new(512)
        }

        fn make_search_config() -> SearchConfig {
            SearchConfig {
                limit: 10,
                score_threshold: 0.5,
                candidate_limit: 30,
            }
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

            let search_service =
                SearchService::new(embedding, qdrant.clone(), rerank, make_search_config());

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

            let results = search_service
                .search(
                    vec!["What is the capital of France?".to_string()],
                    &[kb.clone()],
                )
                .await
                .expect("search");

            assert!(
                results.contains_key("What is the capital of France?"),
                "results should be keyed by query"
            );
            let chunks = &results["What is the capital of France?"];
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
