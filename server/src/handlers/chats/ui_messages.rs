use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIMessage {
    pub id: String,
    pub role: MessageRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    pub parts: Vec<UIMessagePart>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum UIMessagePart {
    #[serde(rename = "text")]
    Text(TextUIPart),
    #[serde(rename = "reasoning")]
    Reasoning(ReasoningUIPart),
    #[serde(rename = "step-start")]
    StepStart,
    #[serde(rename = "source-url")]
    SourceUrl(SourceUrlUIPart),
    #[serde(rename = "source-document")]
    SourceDocument(SourceDocumentUIPart),
    #[serde(rename = "file")]
    File(FileUIPart),
    #[serde(rename = "tool-weather")]
    ToolWeather(ToolUIPart),
    #[serde(rename = "tool-chart")]
    ToolChart(ToolUIPart),
    #[serde(rename = "data-chat-title")]
    DataChatTitle(DataUIPart),
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextUIPart {
    pub text: String,
    #[serde(default)]
    pub state: Option<TextState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextState {
    Streaming,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningUIPart {
    pub text: String,
    #[serde(default)]
    pub state: Option<ReasoningState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReasoningState {
    Streaming,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUIPart {
    #[serde(rename = "toolCallId")]
    pub tool_call_id: String,
    #[serde(rename = "toolName")]
    pub tool_name: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default, rename = "toolMetadata")]
    pub tool_metadata: HashMap<String, serde_json::Value>,
    #[serde(default, rename = "providerExecuted")]
    pub provider_executed: bool,
    pub state: ToolState,
    #[serde(default)]
    pub input: Option<serde_json::Value>,
    #[serde(default)]
    pub output: Option<serde_json::Value>,
    #[serde(default, rename = "errorText")]
    pub error_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolState {
    InputStreaming,
    InputAvailable,
    ApprovalRequested,
    ApprovalResponded,
    OutputAvailable,
    OutputError,
    OutputDenied,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceUrlUIPart {
    #[serde(rename = "sourceId")]
    pub source_id: String,
    pub url: String,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDocumentUIPart {
    #[serde(rename = "sourceId")]
    pub source_id: String,
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub title: String,
    #[serde(default)]
    pub filename: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUIPart {
    #[serde(rename = "mediaType")]
    pub media_type: String,
    #[serde(default)]
    pub filename: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataUIPart {
    pub data: serde_json::Value,
}

impl UIMessagePart {
    pub fn get_type(&self) -> &str {
        match self {
            UIMessagePart::Text(_) => "text",
            UIMessagePart::Reasoning(_) => "reasoning",
            UIMessagePart::StepStart => "step-start",
            UIMessagePart::SourceUrl(_) => "source-url",
            UIMessagePart::SourceDocument(_) => "source-document",
            UIMessagePart::File(_) => "file",
            UIMessagePart::ToolWeather(_) => "tool-weather",
            UIMessagePart::ToolChart(_) => "tool-chart",
            UIMessagePart::DataChatTitle(_) => "data-chat-title",
            UIMessagePart::Other => "unknown",
        }
    }
}
