use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use std::collections::HashMap;

use crate::domain::models::messages::{MessagePart, ToolCallState};

#[derive(Debug, Clone, Serialize)]
pub struct UIMessage {
    pub id: String,
    pub role: MessageRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    pub parts: Vec<UIMessagePart>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// A UI message part in the shape expected by the AI SDK frontend.
///
/// `#[serde(tag = "type")]` cannot express dynamic tool names (`tool-{tool_name}`),
/// so serialization is implemented manually.
#[derive(Debug, Clone)]
pub enum UIMessagePart {
    Text(TextUIPart),
    Reasoning(ReasoningUIPart),
    StepStart,
    SourceUrl(SourceUrlUIPart),
    SourceDocument(SourceDocumentUIPart),
    File(FileUIPart),
    Tool(ToolUIPart),
    DataChatTitle(DataUIPart),
    Other(Value),
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
            UIMessagePart::Tool(_) => "tool",
            UIMessagePart::DataChatTitle(_) => "data-chat-title",
            UIMessagePart::Other(_) => "unknown",
        }
    }
}

impl Serialize for UIMessagePart {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            UIMessagePart::Other(value) => value.serialize(serializer),
            UIMessagePart::StepStart => tagged_value("step-start", &()).serialize(serializer),
            UIMessagePart::Text(p) => tagged_value("text", p).serialize(serializer),
            UIMessagePart::Reasoning(p) => tagged_value("reasoning", p).serialize(serializer),
            UIMessagePart::SourceUrl(p) => tagged_value("source-url", p).serialize(serializer),
            UIMessagePart::SourceDocument(p) => {
                tagged_value("source-document", p).serialize(serializer)
            },
            UIMessagePart::File(p) => tagged_value("file", p).serialize(serializer),
            UIMessagePart::Tool(p) => {
                tagged_value(&format!("tool-{}", p.tool_name), p).serialize(serializer)
            },
            UIMessagePart::DataChatTitle(p) => {
                tagged_value("data-chat-title", p).serialize(serializer)
            },
        }
    }
}

fn tagged_value<T: Serialize>(r#type: &str, value: &T) -> Value {
    let mut value = serde_json::to_value(value).unwrap_or(Value::Null);
    if let Value::Object(map) = &mut value {
        map.insert("type".into(), Value::String(r#type.into()));
    }
    value
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
#[serde(rename_all = "kebab-case")]
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

// ─── Domain → UI conversion ───

/// Convert a stored message `parts` JSON payload to UI parts.
///
/// Messages are persisted in the domain format (`MessagePart`), so parts are
/// mapped part-by-part. Unknown values are preserved as-is instead of being dropped.
pub fn convert_parts_to_ui(parts: &serde_json::Value) -> Vec<UIMessagePart> {
    match parts {
        Value::Array(items) => items.iter().map(value_to_ui_part).collect(),
        single => vec![single_to_ui_part(single)],
    }
}

fn value_to_ui_part(value: &Value) -> UIMessagePart {
    serde_json::from_value::<MessagePart>(value.clone())
        .map(|part| domain_part_to_ui(&part))
        .unwrap_or_else(|_| UIMessagePart::Other(value.clone()))
}

fn single_to_ui_part(value: &Value) -> UIMessagePart {
    let part = value_to_ui_part(value);
    if matches!(part, UIMessagePart::Other(_)) {
        // Fallback: wrap the entire JSON value as a text part so nothing is lost.
        UIMessagePart::Text(TextUIPart {
            text: value.to_string(),
            state: Some(TextState::Done),
        })
    } else {
        part
    }
}

fn domain_part_to_ui(part: &MessagePart) -> UIMessagePart {
    match part {
        MessagePart::Text(t) => UIMessagePart::Text(TextUIPart {
            text: t.text.clone(),
            state: Some(TextState::Done),
        }),
        MessagePart::Reasoning(r) => UIMessagePart::Reasoning(ReasoningUIPart {
            text: r.text.clone(),
            state: Some(ReasoningState::Done),
        }),
        MessagePart::ToolCall(tc) => {
            let state = match tc.state {
                ToolCallState::Error => ToolState::OutputError,
                ToolCallState::Done => ToolState::OutputAvailable,
                _ => ToolState::InputStreaming,
            };
            UIMessagePart::Tool(ToolUIPart {
                tool_call_id: tc.tool_call_id.clone(),
                tool_name: tc.tool_name.clone(),
                title: None,
                tool_metadata: HashMap::new(),
                provider_executed: true,
                state,
                input: Some(tc.input.clone()),
                output: tc.output.clone(),
                error_text: tc.error.clone(),
            })
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_call_part_maps_to_dynamic_tool_ui_part() {
        let parts = serde_json::json!([
            {
                "type": "ToolCall",
                "tool_call_id": "call-1",
                "tool_name": "search_knowledge_base",
                "input": { "queries": ["rust ownership"] },
                "output": { "chunks": [{ "document_id": "doc-1" }] },
                "error": null,
                "state": "Done"
            }
        ]);

        let ui_parts = convert_parts_to_ui(&parts);
        assert_eq!(ui_parts.len(), 1);

        let json = serde_json::to_value(&ui_parts[0]).unwrap();
        assert_eq!(json["type"], "tool-search_knowledge_base");
        assert_eq!(json["toolCallId"], "call-1");
        assert_eq!(json["toolName"], "search_knowledge_base");
        assert_eq!(json["state"], "output-available");
        assert_eq!(json["providerExecuted"], true);
        assert_eq!(json["input"]["queries"][0], "rust ownership");
        assert_eq!(json["output"]["chunks"][0]["document_id"], "doc-1");
    }

    #[test]
    fn tool_call_error_maps_to_output_error() {
        let parts = serde_json::json!([
            {
                "type": "ToolCall",
                "tool_call_id": "call-2",
                "tool_name": "extract_document",
                "input": { "document_id": "doc-1" },
                "output": null,
                "error": "document not found",
                "state": "Error"
            }
        ]);

        let ui_parts = convert_parts_to_ui(&parts);
        let json = serde_json::to_value(&ui_parts[0]).unwrap();
        assert_eq!(json["type"], "tool-extract_document");
        assert_eq!(json["state"], "output-error");
        assert_eq!(json["errorText"], "document not found");
    }

    #[test]
    fn text_and_reasoning_parts_keep_their_state() {
        let parts = serde_json::json!([
            { "type": "text", "text": "hello" },
            { "type": "reasoning", "text": "thinking" }
        ]);

        let ui_parts = convert_parts_to_ui(&parts);
        assert_eq!(ui_parts.len(), 2);

        let json = serde_json::to_value(&ui_parts).unwrap();
        assert_eq!(json[0]["type"], "text");
        assert_eq!(json[0]["text"], "hello");
        assert_eq!(json[0]["state"], "Done");
        assert_eq!(json[1]["type"], "reasoning");
        assert_eq!(json[1]["state"], "Done");
    }

    #[test]
    fn unknown_part_is_preserved() {
        let parts = serde_json::json!([{ "type": "something-future", "value": 42 }]);
        let ui_parts = convert_parts_to_ui(&parts);
        assert!(matches!(ui_parts[0], UIMessagePart::Other(_)));
        let json = serde_json::to_value(&ui_parts[0]).unwrap();
        assert_eq!(json["value"], 42);
    }

    #[test]
    fn non_array_falls_back_to_text_part() {
        let parts = serde_json::json!({ "foo": "bar" });
        let ui_parts = convert_parts_to_ui(&parts);
        assert!(matches!(ui_parts[0], UIMessagePart::Text(_)));
    }

    #[test]
    fn pending_tool_call_maps_to_input_streaming() {
        let parts = serde_json::json!([
            {
                "type": "ToolCall",
                "tool_call_id": "call-3",
                "tool_name": "search_knowledge_base",
                "input": { "queries": [] },
                "output": null,
                "error": null,
                "state": "Invoked"
            }
        ]);

        let ui_parts = convert_parts_to_ui(&parts);
        let json = serde_json::to_value(&ui_parts[0]).unwrap();
        assert_eq!(json["state"], "input-streaming");
    }
}
