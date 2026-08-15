use anyhow::{bail, Context, Result};
use reqwest::multipart::{Form, Part};
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

pub(crate) struct Api {
    client: Client,
    base: String,
}

impl Api {
    pub(crate) fn new(base: String) -> Self {
        Self {
            client: Client::new(),
            base: base.trim_end_matches('/').to_string(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/{}", self.base, path.trim_start_matches('/'))
    }

    pub(crate) async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T> {
        let mut request = self.client.get(self.url(path));
        for (key, value) in query {
            request = request.query(&[(key, value)]);
        }
        let response = request.send().await.context("request failed")?;
        let response = ensure_success(response).await?;
        response.json().await.context("invalid JSON response")
    }

    pub(crate) async fn get_bytes(&self, path: &str) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(self.url(path))
            .send()
            .await
            .context("request failed")?;
        let response = ensure_success(response).await?;
        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .context("failed to read response body")
    }

    pub(crate) async fn send_json<T: Serialize, R: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: &T,
    ) -> Result<R> {
        let response = self
            .client
            .request(method, self.url(path))
            .json(body)
            .send()
            .await
            .context("request failed")?;
        let response = ensure_success(response).await?;
        response.json().await.context("invalid JSON response")
    }

    pub(crate) async fn send_multipart<R: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        form: Form,
    ) -> Result<R> {
        let response = self
            .client
            .request(method, self.url(path))
            .multipart(form)
            .send()
            .await
            .context("request failed")?;
        let response = ensure_success(response).await?;
        response.json().await.context("invalid JSON response")
    }

    pub(crate) async fn delete(&self, path: &str) -> Result<()> {
        let response = self
            .client
            .delete(self.url(path))
            .send()
            .await
            .context("request failed")?;
        ensure_success(response).await?;
        Ok(())
    }
}

async fn ensure_success(response: reqwest::Response) -> Result<reqwest::Response> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    bail!("HTTP {status}: {body}")
}

pub(crate) fn build_doc_form(
    doc_id: Option<&str>,
    kb_id: Option<&str>,
    title: &str,
    url: &str,
    version: &str,
    metadata: &Value,
    data: Vec<u8>,
) -> Form {
    let mut form = Form::new()
        .text("version", version.to_string())
        .text("title", title.to_string())
        .text("url", url.to_string())
        .text("mime_type", "text/plain")
        .text("metadata", metadata.to_string())
        .part("data", Part::bytes(data));
    if let Some(doc_id) = doc_id {
        form = form.text("id", doc_id.to_string());
    }
    if let Some(kb_id) = kb_id {
        form = form.text("kb_id", kb_id.to_string());
    }
    form
}
