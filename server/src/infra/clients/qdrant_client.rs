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
    chunk_index: i32,
    chunk_total: i32,
    chunk_offset: i32,
    chunk_length: i32,
    document_total_length: i32,
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
