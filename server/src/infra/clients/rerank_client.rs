use std::collections::HashMap;
use std::hash::Hash;

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::config::RerankConfig;
use crate::errors::AppError;

#[derive(Debug, Clone, Deserialize)]
pub struct RerankResult {
    pub index: u32,
    pub relevance_score: f32,
}

#[derive(Serialize)]
struct RerankRequest {
    model: String,
    query: String,
    documents: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority: Option<u32>,
}

#[derive(Deserialize)]
struct RerankResponse {
    results: Vec<RerankResult>,
}

#[derive(Clone)]
pub struct RerankClient {
    client: reqwest::Client,
    url: String,
    model: String,
}

impl RerankClient {
    pub fn new(config: &RerankConfig) -> Result<Self, AppError> {
        let model = config.model();
        let api_key = config.api_key().expose_secret().to_string();
        let url = format!("{}/rerank", config.base_url().trim_end_matches('/'));

        let mut headers = HeaderMap::new();
        if !api_key.is_empty() {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {api_key}"))
                    .map_err(|e| AppError::RerankError(e.to_string()))?,
            );
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| AppError::RerankError(e.to_string()))?;

        Ok(Self { client, url, model })
    }

    #[instrument(level = "debug", skip(self, documents), fields(documents = documents.len()))]
    pub async fn rerank(
        &self,
        query: &str,
        documents: &[String],
        top_n: Option<u32>,
        priority: Option<u32>,
    ) -> Result<Vec<RerankResult>, AppError> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }

        let body = RerankRequest {
            model: self.model.clone(),
            query: query.to_string(),
            documents: documents.to_vec(),
            top_n,
            priority,
        };

        let response = self
            .client
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                let msg = e.to_string();
                debug!(error = msg, status = "failed");
                AppError::RerankError(msg)
            })?;

        if !response.status().is_success() {
            let http_status = response.status().as_u16();
            debug!(http_status, status = "failed");
            return Err(AppError::RerankError(format!(
                "rerank request failed: {}",
                response.status()
            )));
        }

        let body: RerankResponse = response.json().await.map_err(|e| {
            let msg = e.to_string();
            debug!(error = msg, status = "failed");
            AppError::RerankError(msg)
        })?;

        let mut results = body.results;
        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results)
    }

    #[instrument(level = "debug", skip(self, documents), fields(documents = documents.len()))]
    pub async fn rerank_map<K>(
        &self,
        query: &str,
        documents: HashMap<K, String>,
        top_n: Option<u32>,
        priority: Option<u32>,
    ) -> Result<Vec<(K, f32)>, AppError>
    where
        K: Eq + Hash + Clone,
    {
        let entries: Vec<(K, String)> = documents.into_iter().collect();
        if entries.is_empty() {
            return Ok(Vec::new());
        }

        let docs: Vec<String> = entries.iter().map(|(_, text)| text.clone()).collect();
        let results = self.rerank(query, &docs, top_n, priority).await?;

        Ok(results
            .into_iter()
            .map(|r| {
                let key = entries[r.index as usize].0.clone();
                (key, r.relevance_score)
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use secrecy::SecretString;
    use serde_json::Value;

    use super::{RerankClient, RerankRequest};
    use crate::config::RerankConfig;

    fn request_json(priority: Option<u32>) -> Value {
        let req = RerankRequest {
            model: "RERANKER".into(),
            query: "test query".into(),
            documents: vec!["doc 1".into(), "doc 2".into()],
            top_n: None,
            priority,
        };
        serde_json::to_value(&req).unwrap()
    }

    #[test]
    fn priority_is_omitted_when_none() {
        let json = request_json(None);
        assert!(json.get("priority").is_none());
    }

    #[test]
    fn priority_is_serialized_when_some() {
        let json = request_json(Some(3));
        assert_eq!(json["priority"], 3);
    }

    fn make_client() -> RerankClient {
        let config = RerankConfig {
            base_url: "http://127.0.0.1:8888/v1".into(),
            api_key: SecretString::new("".to_string().into_boxed_str()),
            model: "RERANKER".into(),
        };
        RerankClient::new(&config).expect("failed to create RerankClient")
    }

    #[tokio::test]
    #[ignore = "requires local rerank model at http://127.0.0.1:8888/v1/rerank"]
    async fn rerank_against_local_model() {
        let client = make_client();

        let results = client
            .rerank(
                "What is the capital of France?",
                &[
                    "The capital of Brazil is Brasilia.".into(),
                    "The capital of France is Paris.".into(),
                    "Horses and cows are both animals".into(),
                ],
                None,
                None,
            )
            .await
            .expect("rerank should succeed");

        assert_eq!(results.len(), 3, "should return one result per document");
        assert!(results[0].relevance_score > results[2].relevance_score);
    }

    #[tokio::test]
    #[ignore = "requires local rerank model at http://127.0.0.1:8888/v1/rerank"]
    async fn rerank_top_n() {
        let client = make_client();

        let results = client
            .rerank("q", &["a".into(), "b".into(), "c".into()], Some(2), None)
            .await
            .expect("rerank with top_n should succeed");

        assert_eq!(results.len(), 2, "top_n should limit results");
    }

    #[tokio::test]
    #[ignore = "requires local rerank model at http://127.0.0.1:8888/v1/rerank"]
    async fn rerank_map_against_local_model() {
        let client = make_client();

        let inputs = HashMap::from([
            (
                "brazil".to_string(),
                "The capital of Brazil is Brasilia.".into(),
            ),
            (
                "france".to_string(),
                "The capital of France is Paris.".into(),
            ),
            (
                "animals".to_string(),
                "Horses and cows are both animals".into(),
            ),
        ]);
        let results = client
            .rerank_map("What is the capital of France?", inputs, None, None)
            .await
            .expect("rerank_map should succeed");

        assert_eq!(results.len(), 3);
        assert!(
            results.first().unwrap().1 > results.last().unwrap().1,
            "first result should have higher score than last"
        );
    }

    #[tokio::test]
    async fn rerank_map_with_empty_input() {
        let client = make_client();

        let results = client
            .rerank_map("query", HashMap::<String, String>::new(), None, None)
            .await
            .expect("empty input should not error");
        assert!(results.is_empty());
    }
}
