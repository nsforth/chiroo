use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIMessage {
    /// A unique identifier for the message.
    pub id: String,

    /// The role of the message (who sent it).
    pub role: MessageRole,

    /// Optional metadata attached to the message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,

    /// The parts of the message. Use this for rendering the message in the UI.
    pub parts: Vec<UIMessagePart>,
}

/// Role of the message sender.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// A part of a message. This is a discriminated union of all possible part types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UIMessagePart {
    /// A text part of a message.
    Text(TextUIPart),

    /// A reasoning part of a message (AI's internal thinking).
    Reasoning(ReasoningUIPart),

    /// A tool invocation part (chart, weather, web_search, etc.).
    Tool(ToolUIPart),

    /// A dynamic tool call (unknown tool at compile time).
    DynamicTool(DynamicToolUIPart),

    /// A source URL reference.
    SourceUrl(SourceUrlUIPart),

    /// A document source reference.
    SourceDocument(SourceDocumentUIPart),

    /// A file attachment.
    File(FileUIPart),

    /// Custom data part with arbitrary payload.
    Data(DataUIPart),

    /// A step boundary marker.
    StepStart,
}

/// A text part of a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextUIPart {
    pub type_: String, // always "text"
    pub text: String,
    #[serde(default)]
    pub state: Option<TextState>,
}

/// State of a text part during streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TextState {
    Streaming,
    Done,
}

/// A reasoning part of a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningUIPart {
    pub type_: String, // always "reasoning"
    pub text: String,
    #[serde(default)]
    pub state: Option<ReasoningState>,
}

/// State of a reasoning part during streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReasoningState {
    Streaming,
    Done,
}

/// A tool invocation part containing all info needed to render a tool in the UI.
///
/// This corresponds to `ToolUIPart<TOOLS>` from ai-sdk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUIPart {
    /// The type identifier (e.g., "tool-chart", "tool-weather").
    pub type_: String,

    /// ID of the tool call.
    pub tool_call_id: String,

    /// Optional title for the tool.
    #[serde(default)]
    pub title: Option<String>,

    /// Metadata about the tool call.
    #[serde(default)]
    pub tool_metadata: HashMap<String, serde_json::Value>,

    /// Whether the tool call was executed by the provider.
    #[serde(default)]
    pub provider_executed: bool,

    /// The state of the tool invocation.
    pub state: ToolState,

    /// Input data for the tool (if available).
    #[serde(default)]
    pub input: Option<serde_json::Value>,

    /// Output data from the tool (if available).
    #[serde(default)]
    pub output: Option<serde_json::Value>,

    /// Error text if the tool execution failed.
    #[serde(default)]
    pub error_text: Option<String>,
}

/// State of a tool invocation.
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

/// A dynamic tool call for unknown tools at runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicToolUIPart {
    pub type_: String, // always "dynamic-tool"
    pub tool_name: String,
    pub tool_call_id: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub tool_metadata: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub provider_executed: bool,
    pub state: ToolState,
    #[serde(default)]
    pub input: Option<serde_json::Value>,
    #[serde(default)]
    pub output: Option<serde_json::Value>,
    #[serde(default)]
    pub error_text: Option<String>,
}

/// A source URL reference part.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceUrlUIPart {
    pub type_: String, // always "source-url"
    pub source_id: String,
    pub url: String,
    #[serde(default)]
    pub title: Option<String>,
}

/// A document source reference part.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDocumentUIPart {
    pub type_: String, // always "source-document"
    pub source_id: String,
    pub media_type: String,
    pub title: String,
    #[serde(default)]
    pub filename: Option<String>,
}

/// A file attachment part.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUIPart {
    pub type_: String, // always "file"
    /// IANA media type (e.g., "image/png", "application/pdf").
    pub media_type: String,
    #[serde(default)]
    pub filename: Option<String>,
    /// URL to the file (hosted URL or Data URL).
    pub url: String,
}

/// A custom data part with arbitrary payload.
///
/// This corresponds to `DataUIPart<DATA_TYPES>` from ai-sdk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataUIPart {
    /// The type identifier (e.g., "data-chat-title").
    pub type_: String,
    /// Arbitrary data payload.
    pub data: serde_json::Value,
}

/// Provider metadata attached to parts during streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMetadata {
    pub warnings: Option<Vec<ProviderWarning>>,
}

/// A warning from the model provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderWarning {
    pub text: String,
}

/// Helper trait for extracting the part type string.
pub trait PartType {
    fn get_type(&self) -> &str;
}

impl UIMessagePart {
    /// Returns the type discriminator for this part.
    pub fn get_type(&self) -> &str {
        match self {
            UIMessagePart::Text(part) => &part.type_,
            UIMessagePart::Reasoning(part) => &part.type_,
            UIMessagePart::Tool(part) => &part.type_,
            UIMessagePart::DynamicTool(part) => &part.type_,
            UIMessagePart::SourceUrl(part) => &part.type_,
            UIMessagePart::SourceDocument(part) => &part.type_,
            UIMessagePart::File(part) => &part.type_,
            UIMessagePart::Data(part) => &part.type_,
            UIMessagePart::StepStart => "step-start",
        }
    }
}
