use async_openai::types::chat::ChatCompletionTools;
use async_trait::async_trait;
use serde_json::Value;

use crate::domain::models::source::Source;

/// The result of executing a tool: the raw JSON passed to the LLM plus any
/// structured sources it discovered (for UI citation rendering).
pub struct ToolExecution {
    pub output: Value,
    pub sources: Vec<Source>,
}

impl ToolExecution {
    pub fn new(output: Value) -> Self {
        Self {
            output,
            sources: Vec::new(),
        }
    }
}

#[async_trait]
pub trait LLMTool: Send {
    fn definition(&self) -> ChatCompletionTools;
    async fn execute(&mut self, arguments: &str) -> Result<ToolExecution, String>;

    /// LLM-facing projection of the stored tool output (e.g. trimmed of fields
    /// the model does not need). Default: the output unchanged.
    fn llm_output(&self, output: &Value) -> Value {
        output.clone()
    }
}
