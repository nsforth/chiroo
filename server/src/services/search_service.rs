use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use futures::future::join_all;
use tracing::{info, instrument};

use crate::domain::models::document_chunk::{DocumentChunkId, SearchedChunk};
use crate::domain::models::knowledge_base::{KnowledgeBase, KnowledgeBaseId};
use crate::domain::models::search_filter::SearchFilter;
use crate::domain::models::search_params::SearchParams;
use crate::errors::AppError;
use crate::infra::clients::embedding_client::EmbeddingClient;
use crate::infra::clients::qdrant_client::QdrantClient;
use crate::infra::clients::rerank_client::RerankClient;
use crate::operation_result::OperationResult;
use crate::utils::LogView;

#[derive(Clone)]
pub struct SearchService {
    embedding_client: EmbeddingClient,
    qdrant_client: QdrantClient,
    rerank_client: RerankClient,
}

impl SearchService {
    pub fn new(
        embedding_client: EmbeddingClient,
        qdrant_client: QdrantClient,
        rerank_client: RerankClient,
    ) -> Self {
        Self {
            embedding_client,
            qdrant_client,
            rerank_client,
        }
    }

    #[instrument(skip(self, kbs), fields(kbs = %kbs.log_view()))]
    pub async fn search_all(
        &self,
        queries: Vec<String>,
        kbs: &[KnowledgeBase],
        filters: &SearchFilter,
        params: &SearchParams,
    ) -> Result<Vec<SearchedChunk>, AppError> {
        let result = self.search_impl(queries, kbs, filters, params).await;
        match &result {
            Ok(chunks) => info!(chunks = chunks.len(), status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    #[instrument(skip(self, kb), fields(kb = %kb.name))]
    pub async fn search_in_kb(
        &self,
        queries: Vec<String>,
        kb: &KnowledgeBase,
        filters: &SearchFilter,
        params: &SearchParams,
    ) -> Result<Vec<SearchedChunk>, AppError> {
        self.search_all(queries, std::slice::from_ref(kb), filters, params)
            .await
    }

    async fn search_impl(
        &self,
        queries: Vec<String>,
        kbs: &[KnowledgeBase],
        filters: &SearchFilter,
        params: &SearchParams,
    ) -> Result<Vec<SearchedChunk>, AppError> {
        let unique = dedup_queries(queries);
        if unique.is_empty() {
            return Ok(Vec::new());
        }

        let query_map: HashMap<String, String> =
            unique.iter().map(|q| (q.clone(), q.clone())).collect();
        let embeddings = self.embedding_client.embed_map(query_map, None).await?;

        let kbs = kbs.to_vec();

        let mut futures = Vec::new();
        for query in &unique {
            let dense_vector = embeddings.get(query).cloned().unwrap_or_default();
            let qdrant_client = self.qdrant_client.clone();
            let rerank_client = self.rerank_client.clone();
            let kbs = kbs.clone();
            let query = query.clone();
            let params = *params;

            futures.push(async move {
                search_one_query(
                    &qdrant_client,
                    &rerank_client,
                    &kbs,
                    &query,
                    dense_vector,
                    params,
                    filters,
                )
                .await
            });
        }

        let mut all = Vec::new();
        for result in join_all(futures).await {
            all.extend(result?);
        }

        Ok(merge_per_kb(all, params.limit))
    }
}

/// Hybrid search + rerank + threshold filter, applied independently per
/// knowledge base, then combined with a per-KB `params.limit` cap.
async fn search_one_query(
    qdrant_client: &QdrantClient,
    rerank_client: &RerankClient,
    kbs: &[KnowledgeBase],
    query: &str,
    dense_vector: Vec<f32>,
    params: SearchParams,
    filters: &SearchFilter,
) -> Result<Vec<SearchedChunk>, AppError> {
    let futures = kbs.iter().map(|kb| {
        let client = qdrant_client.clone();
        let kb_id = kb.id;
        let language = kb.language.clone();
        let dense_vector = dense_vector.clone();
        let query = query.to_string();
        async move {
            let scoped = filters.scope(&kb.indexes);
            let chunks = client
                .search_hybrid(
                    &kb_id,
                    dense_vector,
                    &query,
                    params.candidate_limit,
                    language.as_deref(),
                    Some(&scoped),
                )
                .await?;
            // The dense and BM25 prefetches can both return the same point.
            let chunks = dedup_chunks(chunks);

            let contents: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();
            let reranked = rerank_client
                .rerank(&query, &contents, Some(params.reranker_limit), None)
                .await?;
            let scores: Vec<(usize, f32)> = reranked
                .into_iter()
                .map(|r| (r.index as usize, r.relevance_score))
                .collect();

            Ok::<Vec<SearchedChunk>, AppError>(filter_chunks(
                chunks,
                scores,
                params.reranker_threshold,
            ))
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

fn merge_per_kb(chunks: Vec<SearchedChunk>, limit: usize) -> Vec<SearchedChunk> {
    let mut by_kb: HashMap<KnowledgeBaseId, Vec<SearchedChunk>> = HashMap::new();
    for chunk in chunks {
        by_kb.entry(chunk.kb_id).or_default().push(chunk);
    }

    let per_kb: Vec<SearchedChunk> = by_kb
        .into_values()
        .flat_map(|group| merge_top(group, limit))
        .collect();

    let mut best: HashMap<DocumentChunkId, SearchedChunk> = HashMap::new();
    for chunk in per_kb {
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
        make_chunk_in_kb(content, KnowledgeBaseId(Uuid::new_v4()))
    }

    fn make_chunk_in_kb(content: &str, kb_id: KnowledgeBaseId) -> SearchedChunk {
        SearchedChunk {
            id: DocumentChunkId(Uuid::new_v4()),
            document_id: DocumentId("doc".to_string()),
            kb_id,
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
            query: String::new(),
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
    fn merge_top_keeps_query_of_higher_scoring_duplicate() {
        let kb = KnowledgeBaseId(Uuid::from_bytes([1; 16]));
        let base = make_chunk_in_kb("dup", kb);
        let high = SearchedChunk {
            query: "high query".to_string(),
            ..base.clone()
        }
        .with_score(0.9);
        let low = SearchedChunk {
            query: "low query".to_string(),
            ..base
        }
        .with_score(0.4);

        let merged = merge_top(vec![low, high], 10);

        assert_eq!(merged.len(), 1, "duplicate chunk must appear exactly once");
        assert_eq!(merged[0].score, 0.9, "the higher score must win");
        assert_eq!(
            merged[0].query, "high query",
            "the winner's query must survive the merge"
        );
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
    fn merge_per_kb_caps_each_kb_separately() {
        let kb_a = KnowledgeBaseId(Uuid::from_bytes([1; 16]));
        let kb_b = KnowledgeBaseId(Uuid::from_bytes([2; 16]));

        let mut chunks = Vec::new();
        for i in 0..30 {
            chunks.push(
                make_chunk_in_kb(&format!("a{i}"), kb_a).with_score(0.5 + (i % 30) as f32 * 0.01),
            );
            chunks.push(
                make_chunk_in_kb(&format!("b{i}"), kb_b).with_score(0.5 + (i % 30) as f32 * 0.01),
            );
        }

        let merged = merge_per_kb(chunks, 5);

        assert_eq!(merged.len(), 10, "each KB must keep its own limit");
        assert_eq!(
            merged.iter().filter(|c| c.kb_id == kb_a).count(),
            5,
            "kb A must keep exactly `limit` chunks"
        );
        assert_eq!(
            merged.iter().filter(|c| c.kb_id == kb_b).count(),
            5,
            "kb B must keep exactly `limit` chunks"
        );
        for pair in merged.windows(2) {
            assert!(
                pair[0].score >= pair[1].score,
                "result must be globally sorted by score desc"
            );
        }
    }

    #[test]
    fn merge_per_kb_single_kb_equals_merge_top() {
        let kb = KnowledgeBaseId(Uuid::from_bytes([1; 16]));
        let chunks: Vec<SearchedChunk> = (0..15)
            .map(|i| make_chunk_in_kb(&format!("c{i}"), kb).with_score(i as f32 * 0.1))
            .collect();
        let expected = merge_top(chunks.clone(), 4);
        let merged = merge_per_kb(chunks, 4);

        assert_eq!(merged.len(), 4);
        assert_eq!(
            merged.iter().map(|c| c.id.clone()).collect::<Vec<_>>(),
            expected.iter().map(|c| c.id.clone()).collect::<Vec<_>>(),
            "single-KB merge_per_kb must behave like merge_top"
        );
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

        const QDRANT_URL: &str = "http://127.0.0.1:6334";
        const EMBEDDING_BASE_URL: &str = "http://127.0.0.1:8888/v1";
        const EMBEDDING_MODEL: &str = "EMBEDDINGS";
        const EMBEDDING_DIMENSIONS: u32 = 1024;
        const RERANK_BASE_URL: &str = "http://127.0.0.1:8888/v1";
        const RERANK_MODEL: &str = "RERANKER";

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

            let search_service = SearchService::new(embedding, qdrant.clone(), rerank);
            let params = SearchParams::try_new(10, 30, 30, 0.5).expect("valid params");

            let kb = knowledge_service
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
                .search_all(
                    vec!["What is the capital of France?".to_string()],
                    &[kb.clone()],
                    &SearchFilter::default(),
                    &params,
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

        /// KB with indexable metadata and three documents sharing the same text,
        /// so a single query retrieves all of them and only the filter narrows.
        async fn make_filter_fixture(
            test_id: &str,
        ) -> (KnowledgeService, SearchService, KnowledgeBase, Vec<String>) {
            let pool = get_or_create_test_pool().await;
            let qdrant = make_qdrant_client(test_id);
            let embedding = make_embedding_client();
            let rerank = make_rerank_client();
            let knowledge_service = KnowledgeService::new(
                pool.clone(),
                make_chunking_service(),
                qdrant.clone(),
                embedding.clone(),
                EMBEDDING_DIMENSIONS,
            );
            // Zero threshold so reranker scores never hide chunks the filter kept.
            let search_service = SearchService::new(embedding, qdrant, rerank);

            let kb = knowledge_service
                .create_kb(
                    "filterable".into(),
                    "desc".into(),
                    false,
                    serde_json::json!({}),
                    None,
                    Some(serde_json::json!({
                        "section": { "type": "keyword" },
                        "priority": { "type": "integer" },
                        "updated_at": { "type": "datetime" }
                    })),
                )
                .await
                .expect("create_kb");

            let docs = [
                ("alpha", 1, "2026-01-01T00:00:00Z"),
                ("beta", 2, "2026-02-01T00:00:00Z"),
                ("beta", 3, "2026-03-01T00:00:00Z"),
            ];
            let mut doc_ids = Vec::new();
            for (section, priority, updated_at) in docs {
                let doc_id = Uuid::new_v4().to_string();
                knowledge_service
                    .create_document(
                        doc_id.clone(),
                        kb.id,
                        "Quarterly review".into(),
                        "https://example.com/review".into(),
                        "v1".into(),
                        MimeType::TextPlain,
                        serde_json::json!({
                            "section": section,
                            "priority": priority,
                            "updated_at": updated_at,
                        }),
                        b"Notes about the quarterly planning and budget review process.".to_vec(),
                    )
                    .await
                    .expect("create_document");
                doc_ids.push(doc_id);
            }

            (knowledge_service, search_service, kb, doc_ids)
        }

        fn chunk_doc_ids(chunks: &[SearchedChunk]) -> Vec<String> {
            let mut ids: Vec<String> = chunks.iter().map(|c| c.document_id.0.clone()).collect();
            ids.sort();
            ids
        }

        async fn search_with_filter(
            search: &SearchService,
            kb: &KnowledgeBase,
            filters: serde_json::Value,
        ) -> Vec<SearchedChunk> {
            let filter = SearchFilter::from_json_value(&filters).expect("valid filter value");
            let params = SearchParams::try_new(30, 30, 30, 0.0).expect("valid params");
            search
                .search_in_kb(
                    vec!["quarterly planning and budget review".to_string()],
                    kb,
                    &filter,
                    &params,
                )
                .await
                .expect("search_in_kb")
        }

        #[tokio::test]
        #[ignore = "requires Postgres, Qdrant, embedding, and rerank services"]
        async fn search_filter_keyword_eq() {
            let (knowledge, search, kb, doc_ids) = make_filter_fixture("filter_kw").await;
            let (doc_a, doc_b, doc_c) =
                (doc_ids[0].clone(), doc_ids[1].clone(), doc_ids[2].clone());

            let all = search_with_filter(&search, &kb, serde_json::json!({})).await;
            let all_ids: std::collections::HashSet<_> =
                all.iter().map(|c| &c.document_id.0).collect();
            for id in [&doc_a, &doc_b, &doc_c] {
                assert!(
                    all_ids.contains(&id),
                    "unfiltered search must retrieve every doc, got {all_ids:?}"
                );
            }

            let chunks =
                search_with_filter(&search, &kb, serde_json::json!({ "section": "beta" })).await;
            assert!(!chunks.is_empty(), "beta docs must match");
            let ids = chunk_doc_ids(&chunks);
            assert!(ids.contains(&doc_b), "beta doc #2 must match, got {ids:?}");
            assert!(ids.contains(&doc_c), "beta doc #3 must match, got {ids:?}");
            assert!(
                !ids.contains(&doc_a),
                "alpha doc must be filtered out, got {ids:?}"
            );

            knowledge.delete_kb(kb.id).await.expect("delete_kb");
        }

        #[tokio::test]
        #[ignore = "requires Postgres, Qdrant, embedding, and rerank services"]
        async fn search_filter_integer_range() {
            let (knowledge, search, kb, doc_ids) = make_filter_fixture("filter_int").await;
            let (doc_a, doc_b, doc_c) =
                (doc_ids[0].clone(), doc_ids[1].clone(), doc_ids[2].clone());

            let chunks = search_with_filter(
                &search,
                &kb,
                serde_json::json!({ "priority": { "gte": 2 } }),
            )
            .await;
            let ids = chunk_doc_ids(&chunks);
            assert!(
                !ids.contains(&doc_a),
                "priority 1 must be excluded, got {ids:?}"
            );
            assert!(ids.contains(&doc_b), "priority 2 must match, got {ids:?}");
            assert!(ids.contains(&doc_c), "priority 3 must match, got {ids:?}");

            let chunks =
                search_with_filter(&search, &kb, serde_json::json!({ "priority": { "gt": 2 } }))
                    .await;
            let ids = chunk_doc_ids(&chunks);
            assert!(ids.contains(&doc_c), "priority 3 must match, got {ids:?}");
            assert!(
                !ids.contains(&doc_b),
                "priority 2 must be excluded by gt, got {ids:?}"
            );
            assert!(
                !ids.contains(&doc_a),
                "priority 1 must be excluded, got {ids:?}"
            );

            knowledge.delete_kb(kb.id).await.expect("delete_kb");
        }

        #[tokio::test]
        #[ignore = "requires Postgres, Qdrant, embedding, and rerank services"]
        async fn search_filter_datetime_range() {
            let (knowledge, search, kb, doc_ids) = make_filter_fixture("filter_dt").await;
            let (doc_a, doc_b, doc_c) =
                (doc_ids[0].clone(), doc_ids[1].clone(), doc_ids[2].clone());

            let chunks = search_with_filter(
                &search,
                &kb,
                serde_json::json!({
                    "updated_at": { "gte": "2026-02-01T00:00:00Z", "lt": "2026-03-01T00:00:00Z" }
                }),
            )
            .await;
            let ids = chunk_doc_ids(&chunks);
            assert!(ids.contains(&doc_b), "Feb doc must match, got {ids:?}");
            assert!(
                !ids.contains(&doc_a),
                "Jan doc must be excluded, got {ids:?}"
            );
            assert!(
                !ids.contains(&doc_c),
                "Mar doc must be excluded by lt, got {ids:?}"
            );

            knowledge.delete_kb(kb.id).await.expect("delete_kb");
        }

        #[tokio::test]
        #[ignore = "requires Postgres, Qdrant, embedding, and rerank services"]
        async fn search_filter_and_combination() {
            let (knowledge, search, kb, doc_ids) = make_filter_fixture("filter_and").await;
            let (doc_a, doc_b, doc_c) =
                (doc_ids[0].clone(), doc_ids[1].clone(), doc_ids[2].clone());

            // section=beta AND priority=2 -> only doc B.
            let chunks = search_with_filter(
                &search,
                &kb,
                serde_json::json!({ "section": "beta", "priority": 2 }),
            )
            .await;
            let ids = chunk_doc_ids(&chunks);
            assert!(ids.contains(&doc_b), "doc B must match, got {ids:?}");
            assert!(
                !ids.contains(&doc_a),
                "alpha doc must be excluded, got {ids:?}"
            );
            assert!(
                !ids.contains(&doc_c),
                "priority 3 must be excluded, got {ids:?}"
            );

            knowledge.delete_kb(kb.id).await.expect("delete_kb");
        }

        #[tokio::test]
        #[ignore = "requires Postgres, Qdrant, embedding, and rerank services"]
        async fn search_filter_unindexed_field_is_dropped() {
            let (knowledge, search, kb, _) = make_filter_fixture("filter_unknown").await;

            // Fields with no index in this KB are scoped out, so the filter is a no-op.
            let chunks =
                search_with_filter(&search, &kb, serde_json::json!({ "no_such_field": "x" })).await;
            assert_eq!(
                chunk_doc_ids(&chunks).len(),
                3,
                "dropped filter must not narrow the result"
            );

            knowledge.delete_kb(kb.id).await.expect("delete_kb");
        }

        #[tokio::test]
        #[ignore = "requires Postgres, Qdrant, embedding, and rerank services"]
        async fn search_all_respects_kb_subset() {
            let pool = get_or_create_test_pool().await;
            let qdrant = make_qdrant_client("filter_subset");
            let embedding = make_embedding_client();
            let rerank = make_rerank_client();
            let knowledge_service = KnowledgeService::new(
                pool.clone(),
                make_chunking_service(),
                qdrant.clone(),
                embedding.clone(),
                EMBEDDING_DIMENSIONS,
            );
            let search_service = SearchService::new(embedding, qdrant, rerank);
            let params = SearchParams::try_new(30, 30, 30, 0.0).expect("valid params");

            let kb_a = knowledge_service
                .create_kb(
                    "kb a".into(),
                    "desc".into(),
                    false,
                    serde_json::json!({}),
                    None,
                    None,
                )
                .await
                .expect("create_kb a");
            let kb_b = knowledge_service
                .create_kb(
                    "kb b".into(),
                    "desc".into(),
                    false,
                    serde_json::json!({}),
                    None,
                    None,
                )
                .await
                .expect("create_kb b");

            let doc_a = Uuid::new_v4().to_string();
            let doc_b = Uuid::new_v4().to_string();
            knowledge_service
                .create_document(
                    doc_a.clone(),
                    kb_a.id,
                    "Alpha report".into(),
                    "https://example.com/a".into(),
                    "v1".into(),
                    MimeType::TextPlain,
                    serde_json::json!({}),
                    b"Notes about the quarterly planning and budget review process.".to_vec(),
                )
                .await
                .expect("create_document a");
            knowledge_service
                .create_document(
                    doc_b.clone(),
                    kb_b.id,
                    "Beta report".into(),
                    "https://example.com/b".into(),
                    "v1".into(),
                    MimeType::TextPlain,
                    serde_json::json!({}),
                    b"Notes about the quarterly planning and budget review process.".to_vec(),
                )
                .await
                .expect("create_document b");

            let query = vec!["quarterly planning and budget review".to_string()];

            // Both KBs: both documents must be reachable.
            let all = search_service
                .search_all(
                    query.clone(),
                    &[kb_a.clone(), kb_b.clone()],
                    &SearchFilter::default(),
                    &params,
                )
                .await
                .expect("search_all");
            let all_ids: std::collections::HashSet<_> =
                all.iter().map(|c| &c.document_id.0).collect();
            assert!(
                all_ids.contains(&doc_a),
                "kb A doc must be present, got {all_ids:?}"
            );
            assert!(
                all_ids.contains(&doc_b),
                "kb B doc must be present, got {all_ids:?}"
            );

            // Subset with only KB A: KB B must not leak in.
            let subset = search_service
                .search_all(query, &[kb_a.clone()], &SearchFilter::default(), &params)
                .await
                .expect("search_all subset");
            assert!(!subset.is_empty(), "subset search must return chunks");
            assert!(
                subset.iter().all(|c| c.kb_id == kb_a.id),
                "only chunks from the requested KB may be returned"
            );

            knowledge_service
                .delete_kb(kb_a.id)
                .await
                .expect("delete_kb a");
            knowledge_service
                .delete_kb(kb_b.id)
                .await
                .expect("delete_kb b");
        }
    }
}
