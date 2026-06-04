use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::{self, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use uuid::Uuid;

use crate::domain::models::chat::ChatId;
use crate::domain::models::stream_event::{FinishReason, MessageStreamEvent};
use crate::errors::AppError;
use crate::middleware::auth::AuthUser;
use crate::utils::{JsonExtractor, PathExtractor};
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum SseChunk {
    #[serde(rename = "start")]
    Start {
        #[serde(skip_serializing_if = "Option::is_none", rename = "messageId")]
        message_id: Option<String>,
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
    },
    #[serde(rename = "tool-output-available")]
    ToolOutputAvailable {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        output: serde_json::Value,
    },
    #[serde(rename = "tool-output-error")]
    ToolOutputError {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "errorText")]
        error_text: String,
    },

    #[serde(rename = "finish")]
    Finish {
        #[serde(skip_serializing_if = "Option::is_none", rename = "finishReason")]
        finish_reason: Option<String>,
    },

    #[serde(rename = "error")]
    #[allow(non_snake_case)]
    ErrorChunk { errorText: String },
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    #[allow(dead_code)]
    pub id: Uuid,
    #[allow(dead_code)]
    pub messages: Vec<serde_json::Value>, // TODO: Change to UIMessage
    #[allow(dead_code)]
    #[serde(default)]
    pub trigger: Option<String>, // TODO: Investigate what trigger is?
    #[allow(dead_code)]
    #[serde(default, rename = "messageId")]
    pub message_id: Option<Uuid>,
    #[allow(dead_code)]
    #[serde(default)]
    pub model: Option<String>,
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
    state.chat_service.check_ownership(chat_id, auth_user.user_id).await?;

    let user_text = extract_last_user_text(&body.messages);

    let domain_stream = state.ai_service.stream(chat_id, &user_text);
    let mut started = false;

    let sse_stream = domain_stream.flat_map(move |event| {
        let chunks = domain_to_sse_chunks(&event, &mut started);
        stream::iter(chunks.into_iter().map(|chunk| {
            let json = serde_json::to_string(&chunk).unwrap_or_default();
            Ok(Event::default().data(json))
        }))
    });

    Ok(Sse::new(sse_stream).keep_alive(KeepAlive::default()))
}

// TODO: Replace with UIMessage converter
fn extract_last_user_text(messages: &[serde_json::Value]) -> String {
    messages
        .iter()
        .rev()
        .find(|m| m.get("role").and_then(|r| r.as_str()) == Some("user"))
        .and_then(|m| {
            m.get("parts").and_then(|p| p.as_array()).and_then(|parts| {
                parts
                    .iter()
                    .filter(|p| p.get("type").and_then(|t| t.as_str()) == Some("text"))
                    .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
                    .into()
            })
        })
        .unwrap_or_default()
}

// TODO: replace with struct + state machine and full rewrite
fn domain_to_sse_chunks(event: &MessageStreamEvent, started: &mut bool) -> Vec<SseChunk> {
    match event {
        MessageStreamEvent::LlmCallStart => {
            if !*started {
                *started = true;
                vec![SseChunk::Start { message_id: None }, SseChunk::StartStep]
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
        }],
        MessageStreamEvent::ToolResult {
            tool_call_id,
            output,
        } => vec![SseChunk::ToolOutputAvailable {
            tool_call_id: tool_call_id.clone(),
            output: output.clone(),
        }],
        MessageStreamEvent::ToolError {
            tool_call_id,
            error_text,
        } => vec![SseChunk::ToolOutputError {
            tool_call_id: tool_call_id.clone(),
            error_text: error_text.clone(),
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
