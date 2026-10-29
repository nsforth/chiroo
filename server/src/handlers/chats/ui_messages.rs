use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use std::collections::HashMap;

use crate::domain::models::messages::{MessagePart, ToolCallState};
use crate::domain::models::scenario::{Round, Scenario};
use crate::errors::AppError;

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
    SourceUrl(SourceUrlUIPart),
    SourceDocument(SourceDocumentUIPart),
    File(FileUIPart),
    Tool(ToolUIPart),
    DataChatTitle(DataUIPart),
    DataRound(DataUIPart),
    Other(Value),
}

impl UIMessagePart {
    pub fn get_type(&self) -> &str {
        match self {
            UIMessagePart::Text(_) => "text",
            UIMessagePart::Reasoning(_) => "reasoning",
            UIMessagePart::SourceUrl(_) => "source-url",
            UIMessagePart::SourceDocument(_) => "source-document",
            UIMessagePart::File(_) => "file",
            UIMessagePart::Tool(_) => "tool",
            UIMessagePart::DataChatTitle(_) => "data-chat-title",
            UIMessagePart::DataRound(_) => "data-round",
            UIMessagePart::Other(_) => "unknown",
        }
    }
}

impl Serialize for UIMessagePart {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            UIMessagePart::Other(value) => value.serialize(serializer),
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
            UIMessagePart::DataRound(p) => tagged_value("data-round", p).serialize(serializer),
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
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "providerMetadata"
    )]
    pub provider_metadata: Option<serde_json::Value>,
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
        MessagePart::SourceUrl(s) => UIMessagePart::SourceUrl(SourceUrlUIPart {
            source_id: s.source_id.clone(),
            url: s.url.clone(),
            title: s.title.clone(),
            provider_metadata: s
                .score
                .map(|score| serde_json::json!({ "chiroo": { "score": score } })),
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

pub fn convert_message_parts_to_ui(parts: &[MessagePart]) -> Vec<UIMessagePart> {
    parts.iter().map(domain_part_to_ui).collect()
}

pub fn scenario_to_ui_messages(scenario: &Scenario) -> Result<Vec<UIMessage>, AppError> {
    // rounds[0] is the user's message; rounds[1..] are assistant rounds.
    let Some((user_round, assistant_rounds)) = scenario.rounds.split_first() else {
        return Err(AppError::InternalServerError);
    };

    let mut messages = Vec::new();

    messages.push(UIMessage {
        id: user_round.message.id.0.to_string(),
        role: MessageRole::User,
        metadata: Some(serde_json::json!({
            "scenarioId": scenario.id.0.to_string(),
        })),
        parts: convert_message_parts_to_ui(&user_round.message.parts),
    });

    let Some(first_assistant) = assistant_rounds.first() else {
        return Ok(messages);
    };

    let mut parts: Vec<UIMessagePart> = Vec::new();
    for round in assistant_rounds {
        parts.push(UIMessagePart::DataRound(DataUIPart {
            data: serde_json::json!({ "hidden": round.hidden }),
        }));
        parts.extend(convert_message_parts_to_ui(&round.message.parts));
    }
    parts.extend(source_parts(&scenario.rounds));

    messages.push(UIMessage {
        // The assistant UIMessage.id is the FIRST assistant round's message id.
        id: first_assistant.message.id.0.to_string(),
        role: MessageRole::Assistant,
        metadata: Some(serde_json::json!({
            "scenarioId": scenario.id.0.to_string(),
            "name": scenario.name,
        })),
        parts,
    });

    Ok(messages)
}

fn source_parts(rounds: &[Round]) -> Vec<UIMessagePart> {
    let mut seen = std::collections::HashSet::new();
    let mut parts = Vec::new();
    for round in rounds {
        for source in &round.sources {
            if seen.insert(source.source_id.clone()) {
                parts.push(UIMessagePart::SourceUrl(SourceUrlUIPart {
                    source_id: source.source_id.clone(),
                    url: source.url.clone(),
                    title: source.title.clone(),
                    provider_metadata: source
                        .score
                        .map(|score| serde_json::json!({ "chiroo": { "score": score } })),
                }));
            }
        }
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::messages::{ReasoningBlock, TextBlock, ToolCallBlock};

    #[test]
    fn tool_call_part_maps_to_dynamic_tool_ui_part() {
        let part = MessagePart::ToolCall(ToolCallBlock {
            tool_call_id: "call-1".into(),
            tool_name: "search_in_all_kbs".into(),
            input: serde_json::json!({ "queries": ["rust ownership"] }),
            output: Some(serde_json::json!({ "chunks": [{ "document_id": "doc-1" }] })),
            error: None,
            state: ToolCallState::Done,
        });

        let ui = domain_part_to_ui(&part);
        let json = serde_json::to_value(&ui).unwrap();
        assert_eq!(json["type"], "tool-search_in_all_kbs");
        assert_eq!(json["toolCallId"], "call-1");
        assert_eq!(json["toolName"], "search_in_all_kbs");
        assert_eq!(json["state"], "output-available");
        assert_eq!(json["providerExecuted"], true);
        assert_eq!(json["input"]["queries"][0], "rust ownership");
        assert_eq!(json["output"]["chunks"][0]["document_id"], "doc-1");
    }

    #[test]
    fn tool_call_error_maps_to_output_error() {
        let part = MessagePart::ToolCall(ToolCallBlock {
            tool_call_id: "call-2".into(),
            tool_name: "extract_document".into(),
            input: serde_json::json!({ "document_id": "doc-1" }),
            output: None,
            error: Some("document not found".into()),
            state: ToolCallState::Error,
        });

        let ui = domain_part_to_ui(&part);
        let json = serde_json::to_value(&ui).unwrap();
        assert_eq!(json["type"], "tool-extract_document");
        assert_eq!(json["state"], "output-error");
        assert_eq!(json["errorText"], "document not found");
    }

    #[test]
    fn text_and_reasoning_parts_keep_their_state() {
        let parts = [
            MessagePart::Text(TextBlock {
                text: "hello".into(),
            }),
            MessagePart::Reasoning(ReasoningBlock {
                text: "thinking".into(),
            }),
        ];

        let ui_parts = convert_message_parts_to_ui(&parts);
        assert_eq!(ui_parts.len(), 2);

        let json = serde_json::to_value(&ui_parts).unwrap();
        assert_eq!(json[0]["type"], "text");
        assert_eq!(json[0]["text"], "hello");
        assert_eq!(json[0]["state"], "Done");
        assert_eq!(json[1]["type"], "reasoning");
        assert_eq!(json[1]["state"], "Done");
    }

    #[test]
    fn pending_tool_call_maps_to_input_streaming() {
        let part = MessagePart::ToolCall(ToolCallBlock {
            tool_call_id: "call-3".into(),
            tool_name: "search_in_all_kbs".into(),
            input: serde_json::json!({ "queries": [] }),
            output: None,
            error: None,
            state: ToolCallState::Invoked,
        });

        let ui = domain_part_to_ui(&part);
        let json = serde_json::to_value(&ui).unwrap();
        assert_eq!(json["state"], "input-streaming");
    }

    #[test]
    fn scenario_maps_to_user_and_assistant_ui_messages() {
        use crate::domain::models::chat::ChatId;
        use crate::domain::models::messages::{Message, Role, TextBlock};
        use crate::domain::models::scenario::{Round, Scenario, ScenarioId, ScenarioStatus};
        use uuid::Uuid;

        let chat_id = ChatId(Uuid::new_v4());
        let scenario_id = ScenarioId(Uuid::new_v4());

        let mut user = Message::new(chat_id, Role::User);
        user.parts.push(MessagePart::Text(TextBlock {
            text: "question".into(),
        }));

        let mut first = Message::new(chat_id, Role::Assistant);
        first.parts.push(MessagePart::Text(TextBlock {
            text: "first".into(),
        }));

        let mut second = Message::new(chat_id, Role::Assistant);
        second.parts.push(MessagePart::Text(TextBlock {
            text: "second".into(),
        }));

        let scenario = Scenario {
            id: scenario_id,
            chat_id,
            name: "simple".into(),
            status: ScenarioStatus::Completed,
            rounds: vec![
                Round {
                    message: user.clone(),
                    sources: vec![],
                    hidden: false,
                    usage: None,
                },
                Round {
                    message: first.clone(),
                    sources: vec![],
                    hidden: true,
                    usage: None,
                },
                Round {
                    message: second.clone(),
                    sources: vec![],
                    hidden: false,
                    usage: None,
                },
            ],
            usage: None,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };

        let messages = scenario_to_ui_messages(&scenario).unwrap();
        assert_eq!(messages.len(), 2);

        assert_eq!(messages[0].id, user.id.0.to_string());
        assert_eq!(messages[0].role, MessageRole::User);
        let user_metadata = messages[0].metadata.as_ref().expect("metadata");
        assert_eq!(
            user_metadata["scenarioId"].as_str().unwrap(),
            scenario_id.0.to_string()
        );

        assert_eq!(messages[1].id, first.id.0.to_string());
        assert_eq!(messages[1].role, MessageRole::Assistant);

        let metadata = messages[1].metadata.as_ref().expect("metadata");
        assert_eq!(
            metadata["scenarioId"].as_str().unwrap(),
            scenario_id.0.to_string()
        );
        assert_eq!(metadata["name"].as_str().unwrap(), "simple");

        // Assistant parts are the concatenation of both assistant rounds, with a
        // `data-round` marker before each round carrying its hidden flag.
        assert_eq!(messages[1].parts.len(), 4);
        let json = serde_json::to_value(&messages[1].parts).unwrap();
        assert_eq!(json[0]["type"], "data-round");
        assert_eq!(json[0]["data"]["hidden"], true);
        assert_eq!(json[1]["type"], "text");
        assert_eq!(json[1]["text"], "first");
        assert_eq!(json[2]["type"], "data-round");
        assert_eq!(json[2]["data"]["hidden"], false);
        assert_eq!(json[3]["type"], "text");
        assert_eq!(json[3]["text"], "second");
    }
}
