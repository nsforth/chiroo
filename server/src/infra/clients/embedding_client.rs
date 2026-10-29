use std::collections::HashMap;
use std::hash::Hash;

use async_openai::config::OpenAIConfig;
use async_openai::types::embeddings::{
    CreateEmbeddingRequest, CreateEmbeddingRequestArgs, CreateEmbeddingResponse, EmbeddingInput,
};
use async_openai::Client;
use secrecy::ExposeSecret;
use serde::Serialize;
use tracing::{debug, instrument};

use crate::config::EmbeddingConfig;
use crate::errors::AppError;

#[derive(Clone)]
pub struct EmbeddingClient {
    client: Client<OpenAIConfig>,
    model: String,
    dimensions: u32,
}

#[derive(Serialize)]
struct CreateEmbeddingRequestWithPriority {
    #[serde(flatten)]
    request: CreateEmbeddingRequest,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<u32>,
}

impl EmbeddingClient {
    pub fn new(config: &EmbeddingConfig) -> Self {
        let openai_config = OpenAIConfig::new()
            .with_api_base(config.base_url())
            .with_api_key(config.api_key().expose_secret());

        Self {
            client: Client::with_config(openai_config),
            model: config.model(),
            dimensions: config.dimensions(),
        }
    }

    #[instrument(level = "debug", skip(self, text))]
    pub async fn embed(&self, text: &str, priority: Option<u32>) -> Result<Vec<f32>, AppError> {
        let mut vectors = self.embed_vecs(vec![text.to_string()], priority).await?;
        vectors
            .pop()
            .ok_or_else(|| AppError::EmbeddingError("unexpected embeddings response".into()))
    }

    #[instrument(level = "debug", skip(self, inputs), fields(inputs = inputs.len()))]
    pub async fn embed_map<K>(
        &self,
        inputs: HashMap<K, String>,
        priority: Option<u32>,
    ) -> Result<HashMap<K, Vec<f32>>, AppError>
    where
        K: Eq + Hash,
    {
        let entries: Vec<(K, String)> = inputs.into_iter().collect();
        if entries.is_empty() {
            return Ok(HashMap::new());
        }

        let texts: Vec<String> = entries.iter().map(|(_, text)| text.clone()).collect();
        let vectors = self.embed_vecs(texts, priority).await?;

        Ok(entries
            .into_iter()
            .zip(vectors)
            .map(|((key, _), vector)| (key, vector))
            .collect())
    }

    async fn embed_vecs(
        &self,
        texts: Vec<String>,
        priority: Option<u32>,
    ) -> Result<Vec<Vec<f32>>, AppError> {
        let request = CreateEmbeddingRequestArgs::default()
            .model(self.model.as_str())
            .input(EmbeddingInput::StringArray(texts))
            .build()
            .map_err(|e| AppError::EmbeddingError(e.to_string()))?;

        let response: CreateEmbeddingResponse = self
            .client
            .embeddings()
            .create_byot(CreateEmbeddingRequestWithPriority { request, priority })
            .await
            .map_err(|e| {
                let msg = e.to_string();
                debug!(error = msg, status = "failed");
                AppError::EmbeddingError(msg)
            })?;

        let mut data = response.data;
        data.sort_by_key(|e| e.index);

        let vectors: Vec<Vec<f32>> = data.into_iter().map(|e| e.embedding).collect();
        check_dimensions(&vectors, self.dimensions)?;

        Ok(vectors)
    }
}

fn check_dimensions(vectors: &[Vec<f32>], expected: u32) -> Result<(), AppError> {
    let expected = expected as usize;
    if let Some(vector) = vectors.iter().find(|v| v.len() != expected) {
        return Err(AppError::EmbeddingError(format!(
            "embedding dimension mismatch: expected {expected}, got {}",
            vector.len()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use async_openai::types::embeddings::{CreateEmbeddingRequestArgs, EmbeddingInput};
    use secrecy::SecretString;
    use serde_json::Value;
    use std::collections::HashMap;

    use super::{check_dimensions, CreateEmbeddingRequestWithPriority, EmbeddingClient};
    use crate::config::EmbeddingConfig;

    fn request_json(priority: Option<u32>) -> Value {
        let request = CreateEmbeddingRequestArgs::default()
            .model("EMBEDDINGS")
            .input(EmbeddingInput::String("hello".into()))
            .build()
            .unwrap();
        let wrapper = CreateEmbeddingRequestWithPriority { request, priority };
        serde_json::to_value(&wrapper).unwrap()
    }

    #[test]
    fn priority_is_omitted_when_none() {
        let json = request_json(None);
        assert!(json.get("priority").is_none());
        assert_eq!(json["model"], "EMBEDDINGS");
        assert_eq!(json["input"], "hello");
    }

    #[test]
    fn priority_is_serialized_when_some() {
        let json = request_json(Some(3));
        assert_eq!(json["priority"], 3);
    }

    fn make_client() -> EmbeddingClient {
        let config = EmbeddingConfig {
            base_url: "http://127.0.0.1:8888/v1".into(),
            api_key: SecretString::new("not-needed".to_string().into_boxed_str()),
            model: "EMBEDDINGS".into(),
            dimensions: 1024,
        };
        EmbeddingClient::new(&config)
    }

    #[test]
    fn dimensions_validated_when_matching() {
        let vectors = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        check_dimensions(&vectors, 3).expect("matching size should pass");
    }

    #[test]
    fn dimensions_mismatch_returns_error() {
        let vectors = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0]];
        let err = check_dimensions(&vectors, 3).expect_err("mismatch should error");
        assert!(err.to_string().contains("expected 3"), "{err}");
        assert!(err.to_string().contains("got 2"), "{err}");
    }

    #[tokio::test]
    #[ignore = "requires local embedding model at http://127.0.0.1:8888/v1/embeddings"]
    async fn embed_against_local_model() {
        let client = make_client();

        let vector = client
            .embed("hello world", None)
            .await
            .expect("embed should succeed");
        assert!(!vector.is_empty(), "embedding vector should not be empty");
    }

    #[tokio::test]
    #[ignore = "requires local embedding model at http://127.0.0.1:8888/v1/embeddings"]
    async fn embed_map_against_local_model() {
        let client = make_client();

        let inputs = HashMap::from([
            ("first".to_string(), "first text".to_string()),
            ("second".to_string(), "second text".to_string()),
        ]);
        let result = client
            .embed_map(inputs, Some(3))
            .await
            .expect("embed_map should succeed");

        assert_eq!(result.len(), 2, "should return one vector per key");
        assert!(result.contains_key("first"));
        assert!(result.contains_key("second"));
        for vector in result.values() {
            assert!(!vector.is_empty(), "embedding vector should not be empty");
        }
    }

    #[tokio::test]
    #[ignore = "requires local embedding model at http://127.0.0.1:8888/v1/embeddings"]
    async fn embed_mismatched_dimension_errors() {
        let config = EmbeddingConfig {
            base_url: "http://127.0.0.1:8888/v1".into(),
            api_key: SecretString::new("not-needed".to_string().into_boxed_str()),
            model: "EMBEDDINGS".into(),
            dimensions: 1,
        };
        let client = EmbeddingClient::new(&config);

        let err = client
            .embed("hello world", None)
            .await
            .expect_err("mismatched dimension should error");
        assert!(
            err.to_string().contains("dimension mismatch"),
            "unexpected error: {err}"
        );
    }

    #[tokio::test]
    #[ignore = "requires local embedding model at http://127.0.0.1:8888/v1/embeddings"]
    async fn embed_matching_dimension_succeeds() {
        let discover = make_client();
        let vector = discover
            .embed("hello world", None)
            .await
            .expect("embed should succeed");

        let config = EmbeddingConfig {
            base_url: "http://127.0.0.1:8888/v1".into(),
            api_key: SecretString::new("not-needed".to_string().into_boxed_str()),
            model: "EMBEDDINGS".into(),
            dimensions: vector.len() as u32,
        };
        let client = EmbeddingClient::new(&config);

        let expected_len = vector.len();
        let result = client
            .embed_map(
                HashMap::from([
                    ("first".to_string(), "first text".to_string()),
                    ("second".to_string(), "second text".to_string()),
                ]),
                None,
            )
            .await
            .expect("matching dimension should succeed");

        assert_eq!(result.len(), 2);
        for vector in result.values() {
            assert_eq!(
                vector.len(),
                expected_len,
                "all vectors must match expected size"
            );
        }
    }

    #[tokio::test]
    async fn embed_map_with_empty_input() {
        let client = make_client();

        let result = client
            .embed_map(HashMap::<String, String>::new(), None)
            .await
            .expect("empty input should not error");
        assert!(result.is_empty());
    }
}
