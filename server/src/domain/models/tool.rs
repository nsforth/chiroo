use async_openai::types::chat::ChatCompletionTools;
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait LLMTool: Send {
    fn definition(&self) -> ChatCompletionTools;
    async fn execute(&mut self, arguments: &str) -> Result<Value, String>;
}
