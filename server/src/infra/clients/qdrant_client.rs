use std::collections::HashMap;

use qdrant_client::qdrant::condition::ConditionOneOf;
use qdrant_client::qdrant::points_selector::PointsSelectorOneOf;
use qdrant_client::qdrant::{
    point_id::PointIdOptions, vectors::VectorsOptions, Condition, CreateCollectionBuilder,
    DeletePointsBuilder, Distance, DocumentBuilder, FieldCondition, Filter, Fusion, NamedVectors,
    PointId, PointStruct, PointsIdsList, PrefetchQueryBuilder, QueryPointsBuilder, ScoredPoint,
    SparseVectorConfig, SparseVectorParamsBuilder, UpsertPointsBuilder, Vector,
    VectorParamsBuilder, Vectors,
};
use qdrant_client::Qdrant;
use serde::{Deserialize, Serialize};

use crate::domain::models::document::DocumentId;
use crate::domain::models::document_chunk::{DocumentChunk, DocumentChunkId, SearchedChunk};
use crate::domain::models::knowledge_base::KnowledgeBaseId;
use crate::errors::AppError;

const SPARSE_VECTOR_NAME: &str = "bm25";
const BM25_MODEL: &str = "qdrant/bm25";

#[derive(Clone)]
pub struct QdrantClient {
    client: Qdrant,
    collection_prefix: String,
}

impl QdrantClient {
    pub fn new(
        url: &str,
        api_key: Option<&str>,
        collection_prefix: String,
    ) -> Result<Self, AppError> {
        let mut builder = Qdrant::from_url(url);
        if let Some(key) = api_key {
            builder = builder.api_key(key);
        }
        let client = builder
            .build()
            .map_err(|e| AppError::QdrantError(e.to_string()))?;
        Ok(Self {
            client,
            collection_prefix,
        })
    }

    fn collection_name(&self, kb_id: &KnowledgeBaseId) -> String {
        format!("{}_{}", self.collection_prefix, kb_id.0)
    }

    pub async fn create_collection(
        &self,
        kb_id: &KnowledgeBaseId,
        vector_size: u64,
    ) -> Result<(), AppError> {
        let name = self.collection_name(kb_id);
        let sparse_params = SparseVectorParamsBuilder::default().build();

        let mut map = HashMap::new();
        map.insert(SPARSE_VECTOR_NAME.to_string(), sparse_params);

        self.client
            .create_collection(
                CreateCollectionBuilder::new(&name)
                    .vectors_config(VectorParamsBuilder::new(vector_size, Distance::Cosine))
                    .sparse_vectors_config(SparseVectorConfig { map }),
            )
            .await
            .map_err(|e| AppError::QdrantError(e.to_string()))?;
        Ok(())
    }

    pub async fn delete_collection(&self, kb_id: &KnowledgeBaseId) -> Result<(), AppError> {
        let name = self.collection_name(kb_id);
        self.client
            .delete_collection(&name)
            .await
            .map_err(|e| AppError::QdrantError(e.to_string()))?;
        Ok(())
    }

    pub async fn upsert_chunks(
        &self,
        kb_id: &KnowledgeBaseId,
        chunks: &[DocumentChunk],
    ) -> Result<(), AppError> {
        let name = self.collection_name(kb_id);
        let points: Vec<PointStruct> = chunks.iter().map(chunk_to_point_struct).collect();

        self.client
            .upsert_points(UpsertPointsBuilder::new(&name, points))
            .await
            .map_err(|e| AppError::QdrantError(e.to_string()))?;
        Ok(())
    }

    pub async fn delete_chunks(
        &self,
        kb_id: &KnowledgeBaseId,
        chunk_ids: &[DocumentChunkId],
    ) -> Result<(), AppError> {
        let name = self.collection_name(kb_id);
        let ids: Vec<PointId> = chunk_ids
            .iter()
            .map(|id| PointId {
                point_id_options: Some(PointIdOptions::Uuid(id.0.clone())),
            })
            .collect();

        self.client
            .delete_points(
                DeletePointsBuilder::new(name).points(PointsSelectorOneOf::Points(PointsIdsList {
                    ids: ids.clone(),
                })),
            )
            .await
            .map_err(|e| AppError::QdrantError(e.to_string()))?;
        Ok(())
    }

    pub async fn delete_by_document(
        &self,
        kb_id: &KnowledgeBaseId,
        document_id: &DocumentId,
    ) -> Result<(), AppError> {
        let name = self.collection_name(kb_id);

        let filter = Filter {
            must: vec![Condition {
                condition_one_of: Some(ConditionOneOf::Field(FieldCondition {
                    key: "document_id".to_string(),
                    r#match: Some(qdrant_client::qdrant::Match {
                        match_value: Some(qdrant_client::qdrant::r#match::MatchValue::Keyword(
                            document_id.0.clone(),
                        )),
                    }),
                    ..Default::default()
                })),
            }],
            ..Default::default()
        };

        self.client
            .delete_points(
                DeletePointsBuilder::new(name).points(PointsSelectorOneOf::Filter(filter)),
            )
            .await
            .map_err(|e| AppError::QdrantError(e.to_string()))?;
        Ok(())
    }

    pub async fn search_hybrid(
        &self,
        kb_id: &KnowledgeBaseId,
        dense_vector: Vec<f32>,
        query_text: &str,
        limit: u64,
    ) -> Result<Vec<SearchedChunk>, AppError> {
        let name = self.collection_name(kb_id);

        let dense_prefetch = PrefetchQueryBuilder::default()
            .query(dense_vector)
            .limit(limit);

        let bm25_doc = DocumentBuilder::new(query_text, BM25_MODEL).build();

        let bm25_vector_input = qdrant_client::qdrant::VectorInput {
            variant: Some(qdrant_client::qdrant::vector_input::Variant::Document(
                bm25_doc,
            )),
        };
        let bm25_query = qdrant_client::qdrant::Query {
            variant: Some(qdrant_client::qdrant::query::Variant::Nearest(
                bm25_vector_input,
            )),
        };

        let sparse_prefetch = PrefetchQueryBuilder::default()
            .query(bm25_query)
            .using(SPARSE_VECTOR_NAME.to_string())
            .limit(limit);

        let response = self
            .client
            .query(
                QueryPointsBuilder::new(&name)
                    .add_prefetch(dense_prefetch)
                    .add_prefetch(sparse_prefetch)
                    .query(Fusion::Rrf)
                    .with_payload(true)
                    .limit(limit),
            )
            .await
            .map_err(|e| AppError::QdrantError(e.to_string()))?;

        let results = response
            .result
            .into_iter()
            .filter_map(|point| SearchedChunk::try_from(point).ok())
            .collect();

        Ok(results)
    }
}

fn chunk_to_point_struct(chunk: &DocumentChunk) -> PointStruct {
    let mut named_vectors = HashMap::new();
    named_vectors.insert(String::new(), Vector::from(chunk.embedding.clone()));
    named_vectors.insert(
        SPARSE_VECTOR_NAME.to_string(),
        Vector::from(DocumentBuilder::new(chunk.content.clone(), BM25_MODEL).build()),
    );

    let vectors = Vectors {
        vectors_options: Some(VectorsOptions::Vectors(NamedVectors {
            vectors: named_vectors,
        })),
    };

    let cp = ChunkPayload::from(chunk);
    let json = serde_json::to_value(&cp).expect("ChunkPayload is always serializable");
    let payload =
        qdrant_client::Payload::try_from(json).expect("ChunkPayload serializes to a JSON object");

    let point_id = PointId {
        point_id_options: Some(PointIdOptions::Uuid(chunk.id.0.clone())),
    };

    PointStruct::new(point_id, vectors, payload)
}

#[derive(Deserialize, Serialize)]
struct ChunkPayload {
    document_id: String,
    chunk_index: usize,
    chunk_total: usize,
    chunk_offset: usize,
    chunk_length: usize,
    document_total_length: usize,
    content: String,
    version: String,
    title: String,
    url: String,
    metadata: String,
}

impl From<&DocumentChunk> for ChunkPayload {
    fn from(chunk: &DocumentChunk) -> Self {
        Self {
            document_id: chunk.document_id.0.clone(),
            chunk_index: chunk.chunk_index,
            chunk_total: chunk.chunk_total,
            chunk_offset: chunk.chunk_offset,
            chunk_length: chunk.chunk_length,
            document_total_length: chunk.document_total_length,
            content: chunk.content.clone(),
            version: chunk.version.clone(),
            title: chunk.title.clone(),
            url: chunk.url.clone(),
            metadata: serde_json::to_string(&chunk.metadata).unwrap_or_default(),
        }
    }
}

impl ChunkPayload {
    fn into_searched_chunk(
        self,
        id: DocumentChunkId,
        score: f32,
    ) -> Result<SearchedChunk, AppError> {
        let metadata = serde_json::from_str(&self.metadata)
            .map_err(|e| AppError::QdrantError(e.to_string()))?;
        Ok(SearchedChunk {
            id,
            document_id: DocumentId(self.document_id),
            chunk_index: self.chunk_index,
            chunk_total: self.chunk_total,
            chunk_offset: self.chunk_offset,
            chunk_length: self.chunk_length,
            document_total_length: self.document_total_length,
            content: self.content,
            version: self.version,
            title: self.title,
            url: self.url,
            metadata,
            score,
        })
    }
}

impl TryFrom<ScoredPoint> for SearchedChunk {
    type Error = AppError;

    fn try_from(point: ScoredPoint) -> Result<Self, Self::Error> {
        let id = point
            .id
            .and_then(|id| match id.point_id_options {
                Some(PointIdOptions::Uuid(s)) => Some(s),
                _ => None,
            })
            .ok_or_else(|| AppError::QdrantError("point is missing a UUID ID".into()))?;

        let payload = qdrant_client::Payload::from(point.payload);
        let cp: ChunkPayload = payload
            .deserialize()
            .map_err(|e| AppError::QdrantError(e.to_string()))?;

        cp.into_searched_chunk(DocumentChunkId(id), point.score)
    }
}
#[cfg(test)]
mod tests {
    use serde_json::Value;
    use uuid::Uuid;

    use super::QdrantClient;
    use crate::domain::models::document::DocumentId;
    use crate::domain::models::document_chunk::{DocumentChunk, DocumentChunkId};
    use crate::domain::models::knowledge_base::KnowledgeBaseId;

    const QDRANT_URL: &str = "http://localhost:6334";
    const VECTOR_SIZE: u64 = 4;

    fn make_client(test_id: &str) -> QdrantClient {
        let prefix = format!("test_{test_id}");
        QdrantClient::new(QDRANT_URL, None, prefix).expect("failed to create QdrantClient")
    }

    fn make_kb_id() -> KnowledgeBaseId {
        KnowledgeBaseId(Uuid::new_v4())
    }

    fn make_chunk(
        document_id: &str,
        kb_id: KnowledgeBaseId,
        chunk_index: usize,
        content: &str,
        embedding: Vec<f32>,
    ) -> DocumentChunk {
        let id = Uuid::new_v4().to_string();
        DocumentChunk {
            id: DocumentChunkId(id),
            document_id: DocumentId(document_id.to_string()),
            kb_id,
            chunk_index,
            chunk_total: 3,
            chunk_offset: chunk_index * 100,
            chunk_length: content.len(),
            document_total_length: 300,
            content: content.to_string(),
            embedding,
            version: "v1".to_string(),
            title: format!("Test Doc {document_id}"),
            url: format!("https://example.com/{document_id}"),
            metadata: Value::Null,
        }
    }

    fn small_embedding(v: f32) -> Vec<f32> {
        vec![v, v * 2.0, v * 3.0, v * 4.0]
    }

    #[tokio::test]
    #[ignore]
    async fn create_and_delete_collection() {
        let kb_id = make_kb_id();
        let client = make_client("create_and_delete");

        client
            .create_collection(&kb_id, VECTOR_SIZE)
            .await
            .expect("create_collection should succeed");

        client
            .delete_collection(&kb_id)
            .await
            .expect("delete_collection should succeed");
    }

    #[tokio::test]
    #[ignore]
    async fn upsert_and_search_hybrid() {
        let kb_id = make_kb_id();
        let client = make_client("upsert_and_search");

        client
            .create_collection(&kb_id, VECTOR_SIZE)
            .await
            .expect("create collection");

        let expected_id = DocumentChunkId(Uuid::new_v4().to_string());
        let expected_doc_id = DocumentId("chunk-42".to_string());
        let expected_metadata = serde_json::json!({
            "author": "Alice",
            "tags": ["rust", "qdrant"],
            "depth": 3,
        });
        let expected = DocumentChunk {
            id: expected_id.clone(),
            document_id: expected_doc_id.clone(),
            kb_id,
            chunk_index: 7,
            chunk_total: 13,
            chunk_offset: 700,
            chunk_length: 35,
            document_total_length: 1300,
            content: "Pick the largest non-negative integer, call it n".to_string(),
            embedding: small_embedding(0.8),
            version: "v3".to_string(),
            title: "Art of Round-Trip".to_string(),
            url: "https://example.com/round-trip".to_string(),
            metadata: expected_metadata.clone(),
        };
        let decoy = make_chunk("decoy", kb_id, 0, "unrelated", small_embedding(0.1));
        client
            .upsert_chunks(&kb_id, &[decoy, expected])
            .await
            .expect("upsert chunks");

        let results = client
            .search_hybrid(
                &kb_id,
                small_embedding(0.8),
                "largest non-negative integer",
                5,
            )
            .await
            .expect("search hybrid");

        assert!(!results.is_empty(), "search should return results");
        let found = &results[0];

        assert_eq!(found.id, expected_id);
        assert_eq!(found.document_id, expected_doc_id);
        assert_eq!(found.chunk_index, 7);
        assert_eq!(found.chunk_total, 13);
        assert_eq!(found.chunk_offset, 700);
        assert_eq!(found.chunk_length, 35);
        assert_eq!(found.document_total_length, 1300);
        assert_eq!(
            found.content,
            "Pick the largest non-negative integer, call it n"
        );
        assert_eq!(found.version, "v3");
        assert_eq!(found.title, "Art of Round-Trip");
        assert_eq!(found.url, "https://example.com/round-trip");
        assert_eq!(
            found.metadata, expected_metadata,
            "metadata round-trip mismatch"
        );
        assert!(found.score > 0.0, "score should be positive");

        client
            .delete_collection(&kb_id)
            .await
            .expect("delete collection");
    }

    #[tokio::test]
    #[ignore]
    async fn delete_chunks() {
        let kb_id = make_kb_id();
        let client = make_client("delete_chunks");

        client
            .create_collection(&kb_id, VECTOR_SIZE)
            .await
            .expect("create collection");

        let chunk_a_id = DocumentChunkId(Uuid::new_v4().to_string());
        let chunk_b_id = DocumentChunkId(Uuid::new_v4().to_string());

        let chunks = vec![
            DocumentChunk {
                id: chunk_a_id.clone(),
                document_id: DocumentId("doc-x".to_string()),
                kb_id,
                chunk_index: 0,
                chunk_total: 2,
                chunk_offset: 0,
                chunk_length: 7,
                document_total_length: 200,
                content: "chunk A".to_string(),
                embedding: small_embedding(0.5),
                version: "v1".to_string(),
                title: "Test".to_string(),
                url: "https://example.com".to_string(),
                metadata: Value::Null,
            },
            DocumentChunk {
                id: chunk_b_id,
                document_id: DocumentId("doc-x".to_string()),
                kb_id,
                chunk_index: 1,
                chunk_total: 2,
                chunk_offset: 100,
                chunk_length: 7,
                document_total_length: 200,
                content: "chunk B".to_string(),
                embedding: small_embedding(0.5),
                version: "v1".to_string(),
                title: "Test".to_string(),
                url: "https://example.com".to_string(),
                metadata: Value::Null,
            },
        ];
        client.upsert_chunks(&kb_id, &chunks).await.expect("upsert");

        client
            .delete_chunks(&kb_id, &[chunk_a_id])
            .await
            .expect("delete chunks");

        let results = client
            .search_hybrid(&kb_id, small_embedding(0.5), "chunk", 5)
            .await
            .expect("search");

        assert_eq!(results.len(), 1, "only one chunk should remain");
        assert_eq!(results[0].content, "chunk B");

        client
            .delete_collection(&kb_id)
            .await
            .expect("delete collection");
    }

    #[tokio::test]
    #[ignore]
    async fn delete_by_document() {
        let kb_id = make_kb_id();
        let client = make_client("delete_by_doc");

        client
            .create_collection(&kb_id, VECTOR_SIZE)
            .await
            .expect("create collection");

        let all_chunks = vec![
            make_chunk("doc-a", kb_id, 0, "doc A chunk 0", small_embedding(0.3)),
            make_chunk("doc-a", kb_id, 1, "doc A chunk 1", small_embedding(0.3)),
            make_chunk("doc-b", kb_id, 0, "doc B chunk 0", small_embedding(0.3)),
        ];
        client
            .upsert_chunks(&kb_id, &all_chunks)
            .await
            .expect("upsert");

        client
            .delete_by_document(&kb_id, &DocumentId("doc-a".to_string()))
            .await
            .expect("delete by document");

        let results = client
            .search_hybrid(&kb_id, small_embedding(0.3), "chunk", 10)
            .await
            .expect("search");

        assert_eq!(results.len(), 1, "only doc B chunk should remain");
        assert_eq!(results[0].document_id, DocumentId("doc-b".to_string()));

        client
            .delete_collection(&kb_id)
            .await
            .expect("delete collection");
    }

    #[tokio::test]
    #[ignore]
    async fn search_empty_collection() {
        let kb_id = make_kb_id();
        let client = make_client("search_empty");

        client
            .create_collection(&kb_id, VECTOR_SIZE)
            .await
            .expect("create collection");

        let results = client
            .search_hybrid(&kb_id, small_embedding(0.5), "anything", 5)
            .await
            .expect("search on empty collection");

        assert!(
            results.is_empty(),
            "empty collection should return no results"
        );

        client
            .delete_collection(&kb_id)
            .await
            .expect("delete collection");
    }

    #[tokio::test]
    #[ignore]
    async fn search_nonexistent_collection() {
        let kb_id = make_kb_id();
        let client = make_client("search_nonexistent");

        let result = client
            .search_hybrid(&kb_id, small_embedding(0.5), "anything", 5)
            .await;

        assert!(
            result.is_err(),
            "search on nonexistent collection should error"
        );
    }

    #[tokio::test]
    #[ignore]
    async fn delete_nonexistent_collection() {
        let kb_id = make_kb_id();
        let client = make_client("delete_nonexistent");

        let result = client.delete_collection(&kb_id).await;
        assert!(
            result.is_ok(),
            "delete nonexistent collection should be idempotent"
        );
    }
}
