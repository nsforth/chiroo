use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::{self, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use uuid::Uuid;

use crate::domain::models::auth_user::AuthUser;
use crate::domain::models::chat::ChatId;
use crate::domain::models::messages::{Message, MessageId};
use crate::domain::models::scenario::ScenarioId;
use crate::domain::models::stream_event::{FinishReason, MessageStreamEvent};
use crate::errors::AppError;
use crate::services::ai_service::StreamRequest;
use crate::utils::{JsonExtractor, PathExtractor};
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum SseChunk {
    #[serde(rename = "start")]
    Start {
        #[serde(skip_serializing_if = "Option::is_none", rename = "messageId")]
        message_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "messageMetadata")]
        message_metadata: Option<serde_json::Value>,
    },
    #[serde(rename = "start-step")]
    StartStep,
    #[serde(rename = "finish-step")]
    FinishStep,

    #[serde(rename = "text-start")]
    TextStart { id: String },
    #[serde(rename = "text-delta")]
    TextDelta { id: String, delta: String },
    #[serde(rename = "text-end")]
    TextEnd { id: String },

    #[serde(rename = "reasoning-start")]
    ReasoningStart { id: String },
    #[serde(rename = "reasoning-delta")]
    ReasoningDelta { id: String, delta: String },
    #[serde(rename = "reasoning-end")]
    ReasoningEnd { id: String },

    #[serde(rename = "tool-input-start")]
    ToolInputStart {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(rename = "providerExecuted")]
        provider_executed: bool,
    },
    #[serde(rename = "tool-input-delta")]
    ToolInputDelta {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "inputTextDelta")]
        input_text_delta: String,
    },
    #[serde(rename = "tool-input-available")]
    ToolInputAvailable {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        input: serde_json::Value,
        #[serde(rename = "providerExecuted")]
        provider_executed: bool,
    },
    #[serde(rename = "tool-output-available")]
    ToolOutputAvailable {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        output: serde_json::Value,
        #[serde(rename = "providerExecuted")]
        provider_executed: bool,
    },
    #[serde(rename = "tool-output-error")]
    ToolOutputError {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "errorText")]
        error_text: String,
        #[serde(rename = "providerExecuted")]
        provider_executed: bool,
    },

    #[serde(rename = "source-url")]
    SourceUrl {
        #[serde(rename = "sourceId")]
        source_id: String,
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "providerMetadata")]
        provider_metadata: Option<serde_json::Value>,
    },

    #[serde(rename = "finish")]
    Finish {
        #[serde(skip_serializing_if = "Option::is_none", rename = "finishReason")]
        finish_reason: Option<String>,
    },

    #[serde(rename = "data-round")]
    DataRound { data: serde_json::Value },

    #[serde(rename = "error")]
    #[allow(non_snake_case)]
    ErrorChunk { errorText: String },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MessageTrigger {
    SubmitMessage,
    RegenerateMessage,
    Continue,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub trigger: MessageTrigger,
    #[serde(default, rename = "messageId")]
    pub message_id: Option<Uuid>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub text: String,
    #[serde(default, rename = "userMessageId")]
    pub user_message_id: Option<Uuid>,
    #[serde(default, rename = "scenarioId")]
    pub scenario_id: Option<Uuid>,
}

// ── Handler ──

#[axum::debug_handler]
pub async fn send_message(
    State(state): State<AppState>,
    auth_user: AuthUser,
    PathExtractor(chat_id): PathExtractor<Uuid>,
    JsonExtractor(body): JsonExtractor<SendMessageRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>> + Send>, AppError> {
    let chat_id = ChatId(chat_id);
    state
        .chat_service
        .check_ownership(chat_id, auth_user.user_id)
        .await?;

    let SendMessageRequest {
        trigger,
        message_id,
        model,
        text,
        user_message_id,
        scenario_id,
    } = body;

    let request = match trigger {
        MessageTrigger::SubmitMessage => match message_id {
            Some(id) => {
                let scenario_id = scenario_id
                    .ok_or_else(|| AppError::BadRequest("edit requires scenarioId".to_string()))?;
                StreamRequest::Edit {
                    user_message: Message::user_with_id(chat_id, MessageId(id), text),
                    scenario_id: ScenarioId(scenario_id),
                }
            },
            None => StreamRequest::Submit {
                user_message: Message::user_with_id(
                    chat_id,
                    user_message_id
                        .map(MessageId)
                        .unwrap_or_else(MessageId::new),
                    text,
                ),
                model: model
                    .ok_or_else(|| AppError::BadRequest("model is required".to_string()))?,
            },
        },
        MessageTrigger::RegenerateMessage => {
            let scenario_id = scenario_id.ok_or_else(|| {
                AppError::BadRequest("regenerate requires scenarioId".to_string())
            })?;
            StreamRequest::Regenerate {
                scenario_id: ScenarioId(scenario_id),
            }
        },
        MessageTrigger::Continue => StreamRequest::Continue,
    };

    let (scenario_id, assistant_message_id, domain_stream) =
        state.ai_service.stream(chat_id, auth_user, request).await?;

    let assistant_message_id = assistant_message_id.0.to_string();
    let scenario_id = scenario_id.0.to_string();
    let mut started = false;

    let sse_stream = domain_stream.flat_map(move |event| {
        let chunks =
            domain_to_sse_chunks(&event, &mut started, &assistant_message_id, &scenario_id);
        stream::iter(chunks.into_iter().map(|chunk| {
            let json = serde_json::to_string(&chunk).unwrap_or_default();
            Ok(Event::default().data(json))
        }))
    });

    Ok(Sse::new(sse_stream).keep_alive(KeepAlive::default()))
}

fn domain_to_sse_chunks(
    event: &MessageStreamEvent,
    started: &mut bool,
    assistant_id: &str,
    scenario_id: &str,
) -> Vec<SseChunk> {
    match event {
        MessageStreamEvent::LlmCallStart => {
            if !*started {
                *started = true;
                vec![
                    SseChunk::Start {
                        message_id: Some(assistant_id.to_string()),
                        message_metadata: Some(serde_json::json!({ "scenarioId": scenario_id })),
                    },
                    SseChunk::StartStep,
                ]
            } else {
                vec![SseChunk::StartStep]
            }
        },
        MessageStreamEvent::TextStart { id } => vec![SseChunk::TextStart { id: id.clone() }],
        MessageStreamEvent::TextDelta { id, delta } => vec![SseChunk::TextDelta {
            id: id.clone(),
            delta: delta.clone(),
        }],
        MessageStreamEvent::TextEnd { id } => vec![SseChunk::TextEnd { id: id.clone() }],
        MessageStreamEvent::ReasoningStart { id } => {
            vec![SseChunk::ReasoningStart { id: id.clone() }]
        },
        MessageStreamEvent::ReasoningDelta { id, delta } => {
            vec![SseChunk::ReasoningDelta {
                id: id.clone(),
                delta: delta.clone(),
            }]
        },
        MessageStreamEvent::ReasoningEnd { id } => {
            vec![SseChunk::ReasoningEnd { id: id.clone() }]
        },
        MessageStreamEvent::ToolCallStart {
            tool_call_id,
            tool_name,
            title,
        } => vec![SseChunk::ToolInputStart {
            tool_call_id: tool_call_id.clone(),
            tool_name: tool_name.clone(),
            title: title.clone(),
            provider_executed: true,
        }],
        MessageStreamEvent::ToolCallDelta {
            tool_call_id,
            input_text_delta,
        } => vec![SseChunk::ToolInputDelta {
            tool_call_id: tool_call_id.clone(),
            input_text_delta: input_text_delta.clone(),
        }],
        MessageStreamEvent::ToolCallAvailable {
            tool_call_id,
            tool_name,
            input,
        } => vec![SseChunk::ToolInputAvailable {
            tool_call_id: tool_call_id.clone(),
            tool_name: tool_name.clone(),
            input: input.clone(),
            provider_executed: true,
        }],
        MessageStreamEvent::ToolResult {
            tool_call_id,
            output,
        } => vec![SseChunk::ToolOutputAvailable {
            tool_call_id: tool_call_id.clone(),
            output: output.clone(),
            provider_executed: true,
        }],
        MessageStreamEvent::ToolError {
            tool_call_id,
            error_text,
        } => vec![SseChunk::ToolOutputError {
            tool_call_id: tool_call_id.clone(),
            error_text: error_text.clone(),
            provider_executed: true,
        }],
        MessageStreamEvent::SourceUrl(source) => vec![SseChunk::SourceUrl {
            source_id: source.source_id.clone(),
            url: source.url.clone(),
            title: source.title.clone(),
            provider_metadata: source
                .score
                .map(|score| serde_json::json!({ "chiroo": { "score": score } })),
        }],
        MessageStreamEvent::RoundMarker { hidden } => vec![SseChunk::DataRound {
            data: serde_json::json!({ "hidden": hidden }),
        }],
        MessageStreamEvent::LlmCallFinish {
            finish_reason,
            message: _,
        } => match finish_reason {
            FinishReason::Stop => vec![
                SseChunk::FinishStep,
                SseChunk::Finish {
                    finish_reason: Some("stop".into()),
                },
            ],
            _ => vec![SseChunk::FinishStep],
        },
        MessageStreamEvent::Error { error_text } => vec![SseChunk::ErrorChunk {
            errorText: error_text.clone(),
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_marker_serializes_as_data_round_chunk() {
        let chunks = domain_to_sse_chunks(
            &MessageStreamEvent::RoundMarker { hidden: true },
            &mut false,
            "assistant-id",
            "scenario-id",
        );
        let json = serde_json::to_value(&chunks).unwrap();
        assert_eq!(json[0]["type"], "data-round");
        assert_eq!(json[0]["data"]["hidden"], true);
    }
}
