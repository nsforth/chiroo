use std::collections::HashMap;
use std::pin::Pin;

use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionMessageToolCall, ChatCompletionMessageToolCallChunk,
    ChatCompletionMessageToolCalls, ChatCompletionRequestAssistantMessageArgs,
    ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
    ChatCompletionRequestToolMessageArgs, ChatCompletionRequestUserMessageArgs,
    ChatCompletionToolChoiceOption, ChatCompletionTools, CreateChatCompletionRequest,
    CreateChatCompletionRequestArgs, FinishReason, FunctionCall, ReasoningEffort,
};
use async_openai::Client;
use futures::StreamExt;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, info, trace};
use uuid::Uuid;

use crate::domain::models::chat::ChatId;
use crate::domain::models::messages::{
    Message, MessageId, MessagePart, ReasoningBlock, Role, TextBlock, ToolCallBlock, ToolCallState,
};
use crate::domain::models::stream_event::{FinishReason as DomainFinishReason, MessageStreamEvent};
use crate::domain::models::tool::LLMTool;

// ─── BYOT response types ───

#[derive(Debug, Deserialize)]
struct CustomDelta {
    content: Option<String>,
    #[serde(rename = "reasoning_content", alias = "reasoning", default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ChatCompletionMessageToolCallChunk>>,
}

enum ChunkKind<'a> {
    Reasoning(&'a str),
    Text(&'a str),
    Tool(&'a [ChatCompletionMessageToolCallChunk]),
}

impl CustomDelta {
    fn kind(&self) -> Option<ChunkKind<'_>> {
        if let Some(rc) = self.reasoning_content.as_deref().filter(|s| !s.is_empty()) {
            Some(ChunkKind::Reasoning(rc))
        } else if let Some(content) = self.content.as_deref().filter(|s| !s.is_empty()) {
            Some(ChunkKind::Text(content))
        } else if let Some(chunks) = &self.tool_calls {
            if !chunks.is_empty() {
                Some(ChunkKind::Tool(chunks.as_slice()))
            } else {
                None
            }
        } else {
            None
        }
    }
}

#[derive(Debug, Deserialize)]
struct CustomChoice {
    delta: CustomDelta,
    finish_reason: Option<FinishReason>,
}

#[derive(Debug, Deserialize)]
struct CustomChunk {
    choices: Vec<CustomChoice>,
}

type CustomStream = Pin<
    Box<dyn futures::Stream<Item = Result<CustomChunk, async_openai::error::OpenAIError>> + Send>,
>;

// ─── Public API ───

#[derive(Clone)]
pub struct LlmClient {
    client: Client<OpenAIConfig>,
    model: String,
}

pub struct RunRoundParams {
    pub system_prompt: String,
    pub history: Vec<Message>,
    pub tools: Vec<Box<dyn LLMTool>>,
    pub max_llm_calls: usize,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub extra_body: Option<Value>,
    pub tool_choice: Option<ChatCompletionToolChoiceOption>,
    pub preserve_thinking: bool,
    pub message_id_override: Option<MessageId>,
}

impl LlmClient {
    pub fn new(client: Client<OpenAIConfig>, model: String) -> Self {
        Self { client, model }
    }

    pub fn execute_round(
        &self,
        chat_id: ChatId,
        params: RunRoundParams,
    ) -> impl futures::Stream<Item = MessageStreamEvent> + Send {
        let client = self.client.clone();
        let model = self.model.clone();
        let (tx, rx) = mpsc::channel(64);

        tokio::spawn(async move {
            if let Err(e) = run_round(client, model, params, chat_id, tx.clone()).await {
                info!(
                    chat_id = ?chat_id,
                    error = format_error_chain(&*e, ""),
                    status = "failed"
                );
                let _ = tx
                    .send(MessageStreamEvent::Error {
                        error_text: format_error_chain(&*e, ""),
                    })
                    .await;
            }
        });

        ReceiverStream::new(rx)
    }
}

// ─── Accumulator ───

#[derive(Debug)]
enum AccState {
    Initial,
    Reasoning { id: String },
    Text { id: String },
    Tool,
}

#[derive(Debug)]
struct PendingToolCall {
    id: String,
    name: String,
    arguments: String,
    started: bool,
}

#[derive(Debug)]
struct Accumulator {
    chat_id: ChatId,
    state: AccState,

    reasoning: String,
    text: String,

    tool_calls: HashMap<u32, PendingToolCall>,
}

struct AccumulatorResult {
    message: Message,
    completed_tools: Vec<ChatCompletionMessageToolCall>,
}

impl Accumulator {
    fn new(chat_id: ChatId) -> Self {
        Self {
            chat_id,
            state: AccState::Initial,
            reasoning: String::new(),
            text: String::new(),
            tool_calls: HashMap::new(),
        }
    }

    fn process_delta(&mut self, delta: &CustomDelta) -> Vec<MessageStreamEvent> {
        let chunk = match delta.kind() {
            Some(k) => k,
            None => return Vec::new(),
        };
        let mut events = Vec::new();

        match (&self.state, chunk) {
            (AccState::Initial, ChunkKind::Reasoning(rc)) => {
                let id = Uuid::new_v4().to_string();
                events.push(MessageStreamEvent::ReasoningStart { id: id.clone() });
                events.push(MessageStreamEvent::ReasoningDelta {
                    id: id.clone(),
                    delta: rc.to_string(),
                });
                self.reasoning.push_str(rc);
                self.state = AccState::Reasoning { id };
            },
            (AccState::Initial, ChunkKind::Text(content)) => {
                let id = Uuid::new_v4().to_string();
                events.push(MessageStreamEvent::TextStart { id: id.clone() });
                events.push(MessageStreamEvent::TextDelta {
                    id: id.clone(),
                    delta: content.to_string(),
                });
                self.text.push_str(content);
                self.state = AccState::Text { id };
            },
            (AccState::Initial, ChunkKind::Tool(chunks)) => {
                self.ingest_tool_chunks(chunks, &mut events);
                self.state = AccState::Tool;
            },

            (AccState::Reasoning { id }, ChunkKind::Reasoning(rc)) => {
                events.push(MessageStreamEvent::ReasoningDelta {
                    id: id.clone(),
                    delta: rc.to_string(),
                });
                self.reasoning.push_str(rc);
            },
            (AccState::Reasoning { id }, ChunkKind::Text(content)) => {
                events.push(MessageStreamEvent::ReasoningEnd { id: id.clone() });
                let new_id = Uuid::new_v4().to_string();
                events.push(MessageStreamEvent::TextStart { id: new_id.clone() });
                events.push(MessageStreamEvent::TextDelta {
                    id: new_id.clone(),
                    delta: content.to_string(),
                });
                self.text.push_str(content);
                self.state = AccState::Text { id: new_id };
            },
            (AccState::Reasoning { id }, ChunkKind::Tool(chunks)) => {
                events.push(MessageStreamEvent::ReasoningEnd { id: id.clone() });
                self.ingest_tool_chunks(chunks, &mut events);
                self.state = AccState::Tool;
            },

            (AccState::Text { id }, ChunkKind::Text(content)) => {
                events.push(MessageStreamEvent::TextDelta {
                    id: id.clone(),
                    delta: content.to_string(),
                });
                self.text.push_str(content);
            },
            (AccState::Text { id }, ChunkKind::Reasoning(rc)) => {
                events.push(MessageStreamEvent::TextEnd { id: id.clone() });
                let new_id = Uuid::new_v4().to_string();
                events.push(MessageStreamEvent::ReasoningStart { id: new_id.clone() });
                events.push(MessageStreamEvent::ReasoningDelta {
                    id: new_id.clone(),
                    delta: rc.to_string(),
                });
                self.reasoning.push_str(rc);
                self.state = AccState::Reasoning { id: new_id };
            },
            (AccState::Text { id }, ChunkKind::Tool(chunks)) => {
                events.push(MessageStreamEvent::TextEnd { id: id.clone() });
                self.ingest_tool_chunks(chunks, &mut events);
                self.state = AccState::Tool;
            },

            (AccState::Tool, ChunkKind::Tool(chunks)) => {
                self.ingest_tool_chunks(chunks, &mut events);
            },
            (AccState::Tool, ChunkKind::Reasoning(rc)) => {
                let id = Uuid::new_v4().to_string();
                events.push(MessageStreamEvent::ReasoningStart { id: id.clone() });
                events.push(MessageStreamEvent::ReasoningDelta {
                    id: id.clone(),
                    delta: rc.to_string(),
                });
                self.reasoning.push_str(rc);
                self.state = AccState::Reasoning { id };
            },
            (AccState::Tool, ChunkKind::Text(content)) => {
                let id = Uuid::new_v4().to_string();
                events.push(MessageStreamEvent::TextStart { id: id.clone() });
                events.push(MessageStreamEvent::TextDelta {
                    id: id.clone(),
                    delta: content.to_string(),
                });
                self.text.push_str(content);
                self.state = AccState::Text { id };
            },
        }

        events
    }

    fn finish(&mut self) -> (Vec<MessageStreamEvent>, AccumulatorResult) {
        let mut events = Vec::new();

        match &self.state {
            AccState::Reasoning { id } => {
                events.push(MessageStreamEvent::ReasoningEnd { id: id.clone() });
            },
            AccState::Text { id } => {
                events.push(MessageStreamEvent::TextEnd { id: id.clone() });
            },
            AccState::Initial | AccState::Tool => {},
        }

        let mut message = Message::new(self.chat_id, Role::Assistant);

        if !self.reasoning.is_empty() {
            message.parts.push(MessagePart::Reasoning(ReasoningBlock {
                text: std::mem::take(&mut self.reasoning),
            }));
        }
        if !self.text.is_empty() {
            message.parts.push(MessagePart::Text(TextBlock {
                text: std::mem::take(&mut self.text),
            }));
        }
        if !self.tool_calls.is_empty() {
            for tc in self.tool_calls.values() {
                let input: Value = serde_json::from_str(&tc.arguments).unwrap_or_default();
                message.parts.push(MessagePart::ToolCall(ToolCallBlock {
                    tool_call_id: tc.id.clone(),
                    tool_name: tc.name.clone(),
                    input,
                    output: None,
                    error: None,
                    state: ToolCallState::Invoked,
                }));
            }
        }

        let completed_tools: Vec<ChatCompletionMessageToolCall> = self
            .tool_calls
            .values()
            .map(|tc| ChatCompletionMessageToolCall {
                id: tc.id.clone(),
                function: async_openai::types::chat::FunctionCall {
                    name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                },
            })
            .collect();

        (
            events,
            AccumulatorResult {
                message,
                completed_tools,
            },
        )
    }

    fn ingest_tool_chunks(
        &mut self,
        chunks: &[ChatCompletionMessageToolCallChunk],
        events: &mut Vec<MessageStreamEvent>,
    ) {
        for chunk in chunks {
            let entry = self
                .tool_calls
                .entry(chunk.index)
                .or_insert_with(|| PendingToolCall {
                    id: chunk.id.clone().unwrap_or_default(),
                    name: String::new(),
                    arguments: String::new(),
                    started: false,
                });

            if let Some(id) = &chunk.id {
                entry.id = id.clone();
            }
            if let Some(func) = &chunk.function {
                if let Some(name) = &func.name {
                    entry.name = name.clone();
                }
                if let Some(args) = &func.arguments {
                    entry.arguments.push_str(args);
                }
            }

            // Emit the tool call start only once the id, name and at least one
            // argument chunk are known. The tool call id may arrive in a later
            // chunk than the first arguments; emitting start/delta with an empty
            // or changing id breaks the frontend stream protocol (deltas are
            // matched by tool_call_id). All buffered arguments go out together
            // with the start so nothing is lost.
            if !entry.started && !entry.id.is_empty() && !entry.name.is_empty() {
                entry.started = true;
                events.push(MessageStreamEvent::ToolCallStart {
                    tool_call_id: entry.id.clone(),
                    tool_name: entry.name.clone(),
                    title: Some(format!("Calling {}", entry.name)),
                });
                events.push(MessageStreamEvent::ToolCallDelta {
                    tool_call_id: entry.id.clone(),
                    input_text_delta: entry.arguments.clone(),
                });
            } else if entry.started {
                if let Some(func) = &chunk.function {
                    if let Some(args) = &func.arguments {
                        events.push(MessageStreamEvent::ToolCallDelta {
                            tool_call_id: entry.id.clone(),
                            input_text_delta: args.clone(),
                        });
                    }
                }
            }
        }
    }
}

// ─── Step result ───

enum StepResult {
    Stop {
        message: Message,
    },
    ToolCalls {
        message: Message,
        completed_tools: Vec<ChatCompletionMessageToolCall>,
    },
}

// ─── Stream orchestration ───

async fn send_or_bail(
    tx: &mpsc::Sender<MessageStreamEvent>,
    event: MessageStreamEvent,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tx.send(event).await.map_err(|_| "client disconnected")?;
    Ok(())
}

fn format_error_chain(e: &(dyn std::error::Error + '_), prefix: &str) -> String {
    let mut text = if prefix.is_empty() {
        e.to_string()
    } else {
        format!("{prefix}: {}", e)
    };
    let mut source = e.source();
    while let Some(s) = source {
        text.push_str(" | caused by: ");
        text.push_str(&s.to_string());
        source = s.source();
    }
    text
}

fn count_message_roles(messages: &[ChatCompletionRequestMessage]) -> serde_json::Value {
    let mut developer = 0;
    let mut system = 0;
    let mut user = 0;
    let mut assistant = 0;
    let mut tool = 0;
    let mut function = 0;
    for m in messages {
        match m {
            ChatCompletionRequestMessage::Developer(_) => developer += 1,
            ChatCompletionRequestMessage::System(_) => system += 1,
            ChatCompletionRequestMessage::User(_) => user += 1,
            ChatCompletionRequestMessage::Assistant(_) => assistant += 1,
            ChatCompletionRequestMessage::Tool(_) => tool += 1,
            ChatCompletionRequestMessage::Function(_) => function += 1,
        }
    }
    serde_json::json!({
        "developer": developer,
        "system": system,
        "user": user,
        "assistant": assistant,
        "tool": tool,
        "function": function,
    })
}

fn log_round_finished(round_message: &Message) {
    let text_parts = round_message
        .parts
        .iter()
        .filter(|p| matches!(p, MessagePart::Text(_)))
        .count();
    let reasoning_parts = round_message
        .parts
        .iter()
        .filter(|p| matches!(p, MessagePart::Reasoning(_)))
        .count();
    let tool_calls = round_message
        .parts
        .iter()
        .filter(|p| matches!(p, MessagePart::ToolCall(_)))
        .count();
    debug!(
        text_parts,
        reasoning_parts,
        tool_calls,
        total_parts = round_message.parts.len(),
        status = "round_finished"
    );
    trace!(
        round_message = %serde_json::to_string_pretty(&round_message.parts).unwrap_or_default(),
        status = "round_finished_json"
    );
}

async fn run_round(
    client: Client<OpenAIConfig>,
    model: String,
    mut params: RunRoundParams,
    chat_id: ChatId,
    tx: mpsc::Sender<MessageStreamEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let tool_defs: Vec<ChatCompletionTools> = params.tools.iter().map(|t| t.definition()).collect();
    let preserve_thinking = params.preserve_thinking;

    // TODO: System prompt must not be empty, we may check it on start and don't check it here
    let mut messages: Vec<ChatCompletionRequestMessage> = if params.system_prompt.is_empty() {
        Vec::new()
    } else {
        vec![chat_completion_system_message(params.system_prompt)]
    };
    messages.extend(convert_history_to_openai_messages(
        &params.history,
        preserve_thinking,
    ));

    let mut round_message = Message::new(chat_id, Role::Assistant);
    if let Some(id) = params.message_id_override {
        round_message.id = id;
    }

    for step in 0..params.max_llm_calls {
        send_or_bail(&tx, MessageStreamEvent::LlmCallStart).await?;

        let tool_names: Vec<&str> = tool_defs
            .iter()
            .filter_map(|t| match t {
                ChatCompletionTools::Function(ct) => Some(ct.function.name.as_str()),
                _ => None,
            })
            .collect();
        let role_counts = count_message_roles(&messages);
        debug!(
            step,
            messages = ?role_counts,
            tools = ?tool_names,
            reasoning_effort = ?params.reasoning_effort,
            status = "llm_call_start"
        );
        trace!(
            step,
            request = %serde_json::to_string_pretty(&messages).unwrap_or_default(),
            status = "llm_call_start_json"
        );

        match run_one_llm_call(
            client.clone(),
            &model,
            &messages,
            &tool_defs,
            params.reasoning_effort.clone(),
            params.extra_body.as_ref(),
            params.tool_choice.clone(),
            chat_id,
            &tx,
        )
        .await
        {
            Ok(StepResult::Stop { message }) => {
                debug!(
                    step,
                    finish_reason = "stop",
                    parts = message.parts.len(),
                    status = "llm_call_finish"
                );
                messages.push(build_openai_assistant_message(
                    &message,
                    &[],
                    preserve_thinking,
                ));
                round_message.parts.extend(message.parts);
                log_round_finished(&round_message);
                send_or_bail(
                    &tx,
                    MessageStreamEvent::LlmCallFinish {
                        finish_reason: DomainFinishReason::Stop,
                        message: round_message,
                    },
                )
                .await?;
                info!(reason = "stop", status = "success");
                return Ok(());
            },
            Ok(StepResult::ToolCalls {
                message,
                completed_tools,
            }) => {
                debug!(
                    step,
                    finish_reason = "tool_calls",
                    tool_count = completed_tools.len(),
                    status = "llm_call_finish"
                );
                messages.push(build_openai_assistant_message(
                    &message,
                    &completed_tools,
                    preserve_thinking,
                ));
                round_message.parts.extend(message.parts);

                // Emit ToolCallAvailable events
                for tc in &completed_tools {
                    let input: Value =
                        serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                    send_or_bail(
                        &tx,
                        MessageStreamEvent::ToolCallAvailable {
                            tool_call_id: tc.id.clone(),
                            tool_name: tc.function.name.clone(),
                            input: input.clone(),
                        },
                    )
                    .await?;
                }

                send_or_bail(
                    &tx,
                    MessageStreamEvent::LlmCallFinish {
                        finish_reason: DomainFinishReason::ToolCalls,
                        message: round_message.clone(),
                    },
                )
                .await?;

                // Execute tools and push results
                for tc in &completed_tools {
                    let tool = params.tools.iter().position(|t| match t.definition() {
                        ChatCompletionTools::Function(ref ct) => {
                            ct.function.name == tc.function.name
                        },
                        _ => false,
                    });

                    if let Some(idx) = tool {
                        let tool = &mut params.tools[idx];
                        match tool.execute(&tc.function.arguments).await {
                            Ok(output) => {
                                update_tool_output(
                                    &mut round_message,
                                    &tc.id,
                                    Some(output.clone()),
                                    None,
                                );
                                messages.push(tool_result_message(&tc.id, &output));
                                send_or_bail(
                                    &tx,
                                    MessageStreamEvent::ToolResult {
                                        tool_call_id: tc.id.clone(),
                                        output,
                                    },
                                )
                                .await?;
                            },
                            Err(err_text) => {
                                update_tool_output(
                                    &mut round_message,
                                    &tc.id,
                                    None,
                                    Some(err_text.clone()),
                                );
                                send_or_bail(
                                    &tx,
                                    MessageStreamEvent::ToolError {
                                        tool_call_id: tc.id.clone(),
                                        error_text: err_text.clone(),
                                    },
                                )
                                .await?;
                                messages.push(tool_error_message(&tc.id, &err_text));
                            },
                        }
                    } else {
                        let err = format!("Tool not found: {}", tc.function.name);
                        update_tool_output(&mut round_message, &tc.id, None, Some(err.clone()));
                        send_or_bail(
                            &tx,
                            MessageStreamEvent::ToolError {
                                tool_call_id: tc.id.clone(),
                                error_text: err.clone(),
                            },
                        )
                        .await?;
                        messages.push(tool_error_message(&tc.id, &err));
                    }
                }
            },
            Err(e) => {
                let error_text = format_error_chain(&*e, &format!("LLM call {} failed", step + 1));
                let _ = tx.send(MessageStreamEvent::Error { error_text }).await;
                return Ok(());
            },
        }
    }

    // The round hit its call limit. If it produced output (e.g. a single forced
    // tool call that was executed), close the round cleanly instead of failing —
    // the accumulated message carries the tool calls and their results.
    if round_message.parts.is_empty() {
        let _ = tx
            .send(MessageStreamEvent::Error {
                error_text: "Max LLM calls reached".into(),
            })
            .await;
        return Ok(());
    }

    log_round_finished(&round_message);
    send_or_bail(
        &tx,
        MessageStreamEvent::LlmCallFinish {
            finish_reason: DomainFinishReason::Stop,
            message: round_message,
        },
    )
    .await?;
    info!(reason = "max_llm_calls_with_results", status = "success");

    Ok(())
}

async fn run_one_llm_call(
    client: Client<OpenAIConfig>,
    model: &str,
    messages: &[ChatCompletionRequestMessage],
    tools: &[ChatCompletionTools],
    reasoning_effort: Option<ReasoningEffort>,
    extra_body: Option<&Value>,
    tool_choice: Option<ChatCompletionToolChoiceOption>,
    chat_id: ChatId,
    tx: &mpsc::Sender<MessageStreamEvent>,
) -> Result<StepResult, Box<dyn std::error::Error + Send + Sync>> {
    let request = build_request(
        model,
        messages,
        tools,
        reasoning_effort,
        extra_body,
        tool_choice,
    );

    let mut stream: CustomStream = client.chat().create_stream_byot(request).await?;
    let mut acc = Accumulator::new(chat_id);

    while let Some(result) = stream.next().await {
        let response = result?;

        for choice in response.choices {
            for event in acc.process_delta(&choice.delta) {
                send_or_bail(tx, event).await?;
            }

            if let Some(reason) = &choice.finish_reason {
                let (end_events, result) = acc.finish();
                for event in end_events {
                    send_or_bail(tx, event).await?;
                }

                return Ok(match reason {
                    FinishReason::ToolCalls => StepResult::ToolCalls {
                        message: result.message,
                        completed_tools: result.completed_tools,
                    },
                    FinishReason::Stop => StepResult::Stop {
                        message: result.message,
                    },
                    _ => {
                        let reason_str = format!("{reason:?}");
                        return Err(reason_str.into());
                    },
                });
            }
        }
    }

    Err("Stream ended without finish_reason".into())
}

// ─── Request building ───

#[derive(Serialize)]
struct ChatCompletionRequestWithExtra {
    #[serde(flatten)]
    request: CreateChatCompletionRequest,
    #[serde(flatten)]
    extra: serde_json::Map<String, Value>,
}

fn build_request(
    model: &str,
    messages: &[ChatCompletionRequestMessage],
    tools: &[ChatCompletionTools],
    reasoning_effort: Option<ReasoningEffort>,
    extra_body: Option<&Value>,
    tool_choice: Option<ChatCompletionToolChoiceOption>,
) -> ChatCompletionRequestWithExtra {
    let mut builder = CreateChatCompletionRequestArgs::default();
    builder
        .model(model)
        .messages(messages.to_vec())
        .stream(true);
    if !tools.is_empty() {
        builder.tools(tools.to_vec());
    }
    if let Some(effort) = reasoning_effort {
        builder.reasoning_effort(effort);
    }
    if let Some(choice) = tool_choice {
        builder.tool_choice(choice);
    }
    let request = builder.build().unwrap();

    let extra = if let Some(Value::Object(map)) = extra_body {
        map.clone()
    } else {
        serde_json::Map::new()
    };

    ChatCompletionRequestWithExtra { request, extra }
}

// ─── Message conversion helpers ───

fn chat_completion_system_message(content: String) -> ChatCompletionRequestMessage {
    ChatCompletionRequestSystemMessageArgs::default()
        .content(content)
        .build()
        .unwrap()
        .into()
}

/// A single assistant turn: text plus the tool calls it made (in that turn).
struct Segment<'a> {
    text: String,
    tool_blocks: Vec<&'a ToolCallBlock>,
}

/// Splits a flattened message's parts back into per-step segments.
///
/// The accumulator flattens all parts of a round into one `Message`, losing the
/// step boundaries (e.g. `[tool_call, text]` where the tool call belongs to one
/// LLM step and the final text to the next). Without splitting, the final text
/// gets bundled into the same OpenAI assistant message as the tool calls, which
/// makes the model believe it already answered.
///
/// With `preserve_thinking`, reasoning parts are kept as a `<think>` prefix of
/// the segment text so the model sees its previous reasoning in later rounds.
fn split_segments(parts: &[MessagePart], preserve_thinking: bool) -> Vec<Segment<'_>> {
    let mut segments: Vec<Segment> = Vec::new();
    for part in parts {
        match part {
            MessagePart::Reasoning(r) => {
                if preserve_thinking {
                    let wrapped = format!("<think>\n{}\n</think>\n\n", r.text.trim_end());
                    append_segment_text(&mut segments, &wrapped);
                }
            },
            MessagePart::Text(t) => {
                append_segment_text(&mut segments, &t.text);
            },
            MessagePart::ToolCall(tc) => {
                if segments.is_empty() {
                    segments.push(Segment {
                        text: String::new(),
                        tool_blocks: Vec::new(),
                    });
                }
                if let Some(seg) = segments.last_mut() {
                    seg.tool_blocks.push(tc);
                }
            },
        }
    }
    segments
}

fn append_segment_text(segments: &mut Vec<Segment>, text: &str) {
    let push_new = match segments.last_mut() {
        // Text after tool calls starts a new assistant turn.
        Some(seg) => !seg.tool_blocks.is_empty(),
        None => true,
    };
    if push_new {
        segments.push(Segment {
            text: text.to_string(),
            tool_blocks: Vec::new(),
        });
    } else if let Some(seg) = segments.last_mut() {
        seg.text.push_str(text);
    }
}

fn convert_history_to_openai_messages(
    history: &[Message],
    preserve_thinking: bool,
) -> Vec<ChatCompletionRequestMessage> {
    let mut out = Vec::new();

    for msg in history {
        match msg.role {
            Role::User => {
                out.push(
                    ChatCompletionRequestUserMessageArgs::default()
                        .content(join_text_parts(&msg.parts))
                        .build()
                        .unwrap()
                        .into(),
                );
            },
            Role::System => {
                out.push(
                    ChatCompletionRequestSystemMessageArgs::default()
                        .content(join_text_parts(&msg.parts))
                        .build()
                        .unwrap()
                        .into(),
                );
            },
            Role::Assistant => {
                for segment in split_segments(&msg.parts, preserve_thinking) {
                    if segment.text.is_empty() && segment.tool_blocks.is_empty() {
                        continue;
                    }

                    let mut builder = ChatCompletionRequestAssistantMessageArgs::default();
                    builder.content(segment.text);
                    if !segment.tool_blocks.is_empty() {
                        builder.tool_calls(tool_calls_from_blocks(&segment.tool_blocks));
                    }
                    out.push(builder.build().unwrap().into());

                    for tc in segment.tool_blocks {
                        out.push(tool_message(&tc.tool_call_id, tool_message_content(tc)));
                    }
                }
            },
        }
    }

    out
}

fn tool_calls_from_blocks(blocks: &[&ToolCallBlock]) -> Vec<ChatCompletionMessageToolCalls> {
    blocks
        .iter()
        .map(|tc| {
            ChatCompletionMessageToolCalls::Function(ChatCompletionMessageToolCall {
                id: tc.tool_call_id.clone(),
                function: FunctionCall {
                    name: tc.tool_name.clone(),
                    arguments: serde_json::to_string(&tc.input).unwrap_or_default(),
                },
            })
        })
        .collect()
}

fn tool_message_content(tc: &ToolCallBlock) -> String {
    match (&tc.output, &tc.error) {
        (Some(output), _) => output.to_string(),
        (None, Some(err)) => format!("Error: {err}"),
        (None, None) => "Tool call produced no output".to_string(),
    }
}

fn join_text_parts(parts: &[MessagePart]) -> String {
    parts
        .iter()
        .filter_map(|p| match p {
            MessagePart::Text(t) => Some(t.text.as_str()),
            _ => None,
        })
        .collect()
}

fn join_reasoning_parts(parts: &[MessagePart]) -> String {
    parts
        .iter()
        .filter_map(|p| match p {
            MessagePart::Reasoning(r) => Some(r.text.as_str()),
            _ => None,
        })
        .collect()
}

/// Builds assistant message content. With `preserve_thinking`, any reasoning is
/// kept as a `<think>` prefix so the model sees its previous reasoning.
fn assistant_content_from_parts(parts: &[MessagePart], preserve_thinking: bool) -> String {
    let text = join_text_parts(parts);
    if !preserve_thinking {
        return text;
    }
    let reasoning = join_reasoning_parts(parts);
    if reasoning.is_empty() {
        return text;
    }
    format!("<think>\n{}\n</think>\n\n{}", reasoning.trim_end(), text)
}

fn build_openai_assistant_message(
    message: &Message,
    tool_calls: &[ChatCompletionMessageToolCall],
    preserve_thinking: bool,
) -> ChatCompletionRequestMessage {
    let content = assistant_content_from_parts(&message.parts, preserve_thinking);

    let mut builder = ChatCompletionRequestAssistantMessageArgs::default();
    builder.content(content);
    if !tool_calls.is_empty() {
        let tc: Vec<ChatCompletionMessageToolCalls> = tool_calls
            .iter()
            .map(|tc| ChatCompletionMessageToolCalls::Function(tc.clone()))
            .collect();
        builder.tool_calls(tc);
    }
    builder.build().unwrap().into()
}

fn update_tool_output(
    message: &mut Message,
    tool_call_id: &str,
    output: Option<Value>,
    error: Option<String>,
) {
    if let Some(part) = message.parts.iter_mut().find_map(|p| {
        if let MessagePart::ToolCall(tc) = p {
            if tc.tool_call_id == tool_call_id {
                return Some(tc);
            }
        }
        None
    }) {
        if let Some(o) = output {
            part.output = Some(o);
            part.state = ToolCallState::Done;
        }
        if let Some(e) = error {
            part.error = Some(e);
            part.state = ToolCallState::Error;
        }
    }
}

fn tool_message(tool_call_id: &str, content: String) -> ChatCompletionRequestMessage {
    ChatCompletionRequestToolMessageArgs::default()
        .tool_call_id(tool_call_id)
        .content(content)
        .build()
        .unwrap()
        .into()
}

fn tool_result_message(tool_call_id: &str, output: &Value) -> ChatCompletionRequestMessage {
    tool_message(tool_call_id, output.to_string())
}

fn tool_error_message(tool_call_id: &str, error: &str) -> ChatCompletionRequestMessage {
    tool_message(tool_call_id, format!("Error: {error}"))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_openai::config::OpenAIConfig;
    use async_openai::types::chat::{
        ChatCompletionMessageToolCallChunk, ChatCompletionTool, ChatCompletionTools,
        FunctionCallStream, FunctionObject,
    };
    use async_trait::async_trait;
    use serde_json::Value;
    use uuid::Uuid;

    use crate::domain::models::chat::ChatId;
    use crate::domain::models::messages::{
        Message, MessagePart, ReasoningBlock, Role, TextBlock, ToolCallBlock, ToolCallState,
    };
    use crate::domain::models::stream_event::MessageStreamEvent;
    use crate::domain::models::tool::LLMTool;
    use crate::infra::clients::llm_client::{Accumulator, CustomDelta, LlmClient, RunRoundParams};

    use futures::StreamExt;

    #[derive(Clone)]
    struct TracingWeatherTool {
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl TracingWeatherTool {
        fn new() -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }
    }

    #[async_trait]
    impl LLMTool for TracingWeatherTool {
        fn definition(&self) -> ChatCompletionTools {
            ChatCompletionTools::Function(ChatCompletionTool {
                function: FunctionObject {
                    name: "get_weather".into(),
                    description: Some("Get current weather for a location".into()),
                    parameters: Some(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "location": {
                                "type": "string",
                                "description": "City name"
                            }
                        },
                        "required": ["location"]
                    })),
                    strict: None,
                },
            })
        }

        async fn execute(&mut self, arguments: &str) -> Result<Value, String> {
            self.calls.lock().unwrap().push(arguments.to_string());

            let _input: Value =
                serde_json::from_str(arguments).map_err(|e| format!("Invalid args: {e}"))?;

            Ok(serde_json::json!({
                "location": "London",
                "temperature": 18,
                "temperatureHigh": 22,
                "temperatureLow": 12,
                "condition": {
                    "text": "Cloudy",
                    "icon": "i-lucide-cloud"
                },
                "humidity": 72,
                "windSpeed": 15,
                "dailyForecast": [
                    {"day": "Today", "high": 22, "low": 14,
                     "condition": {"text": "Cloudy", "icon": "i-lucide-cloud"}},
                    {"day": "Thu", "high": 24, "low": 16,
                     "condition": {"text": "Sunny", "icon": "i-lucide-sun"}},
                    {"day": "Fri", "high": 20, "low": 11,
                     "condition": {"text": "Rainy", "icon": "i-lucide-cloud-rain"}}
                ]
            }))
        }
    }

    #[tokio::test]
    #[ignore = "requires local LLM at http://localhost:11434/v1"]
    async fn round_with_weather_tool_and_reasoning() {
        let config = OpenAIConfig::new()
            .with_api_base("http://localhost:11434/v1")
            .with_api_key("not-needed");

        let client = LlmClient::new(
            async_openai::Client::with_config(config),
            "Qwen3.6-27B".into(),
        );

        let tool = TracingWeatherTool::new();

        let chat_id = ChatId(Uuid::new_v4());

        let mut user_msg = Message::new(chat_id, Role::User);
        user_msg.parts.push(MessagePart::Text(TextBlock {
            text: "What's the weather in London?".into(),
        }));

        let params = RunRoundParams {
        system_prompt:
            "You MUST call the get_weather tool to answer. NEVER answer without calling it first."
                .into(),
        history: vec![user_msg],
        tools: vec![Box::new(tool.clone())],
        max_llm_calls: 3,
        reasoning_effort: None,
        extra_body: None,
        tool_choice: Some(
            async_openai::types::chat::ChatCompletionToolChoiceOption::Mode(
                async_openai::types::chat::ToolChoiceOptions::Required,
            ),
        ),
        preserve_thinking: true,
        message_id_override: None,
    };

        let events: Vec<MessageStreamEvent> = client.execute_round(chat_id, params).collect().await;

        // Check first — any error makes further assertions meaningless
        let error = events.iter().find_map(|e| match e {
            MessageStreamEvent::Error { error_text } => Some(error_text.as_str()),
            _ => None,
        });
        assert!(error.is_none(), "Unexpected error in stream: {error:?}");

        assert!(
            tool.call_count() >= 1,
            "LLM should call get_weather at least once, got {}",
            tool.call_count()
        );

        let has_llm_start = events
            .iter()
            .any(|e| matches!(e, MessageStreamEvent::LlmCallStart));
        assert!(has_llm_start, "Should emit LlmCallStart");

        let has_tool_start = events.iter().any(|e| {
            matches!(
                e,
                MessageStreamEvent::ToolCallStart { tool_name, .. }
                if tool_name == "get_weather"
            )
        });
        assert!(has_tool_start, "Should emit ToolCallStart for get_weather");

        let has_tool_result = events
            .iter()
            .any(|e| matches!(e, MessageStreamEvent::ToolResult { .. }));
        assert!(has_tool_result, "Should emit ToolResult");

        let has_stop = events.iter().any(|e| {
            matches!(
                e,
                MessageStreamEvent::LlmCallFinish {
                    finish_reason: crate::domain::models::stream_event::FinishReason::Stop,
                    ..
                }
            )
        });
        assert!(has_stop, "Should finish with Stop");

        let has_reasoning = events.iter().any(|e| {
            matches!(
                e,
                MessageStreamEvent::ReasoningStart { .. }
                    | MessageStreamEvent::ReasoningDelta { .. }
                    | MessageStreamEvent::ReasoningEnd { .. }
            )
        });
        assert!(has_reasoning, "Qwen model should emit reasoning chunks");

        // ─── Verify complete Message in LlmCallFinish(Stop) ───

        let final_message = events
            .iter()
            .find_map(|e| {
                if let MessageStreamEvent::LlmCallFinish {
                    finish_reason: crate::domain::models::stream_event::FinishReason::Stop,
                    message,
                } = e
                {
                    Some(message)
                } else {
                    None
                }
            })
            .expect("Should have LlmCallFinish(Stop) with a message");

        // The message must contain reasoning parts (Qwen model defaults to reasoning)
        let has_reasoning_part = final_message
            .parts
            .iter()
            .any(|p| matches!(p, MessagePart::Reasoning { .. }));
        assert!(
            has_reasoning_part,
            "Final message should contain reasoning parts"
        );

        // The message must contain text parts (the answer)
        let has_text_part = final_message
            .parts
            .iter()
            .any(|p| matches!(p, MessagePart::Text { .. }));
        assert!(has_text_part, "Final message should contain text parts");

        // The message must contain tool call parts with output set
        let tool_calls: Vec<_> = final_message
            .parts
            .iter()
            .filter_map(|p| {
                if let MessagePart::ToolCall(tc) = p {
                    Some(tc)
                } else {
                    None
                }
            })
            .collect();

        assert!(
            !tool_calls.is_empty(),
            "Final message should contain tool call parts"
        );

        for tc in &tool_calls {
            assert_eq!(
                tc.tool_name, "get_weather",
                "Tool name should be get_weather"
            );
            assert!(
                matches!(tc.state, ToolCallState::Done),
                "Tool call state should be Done"
            );
            assert!(tc.output.is_some(), "Tool call should have output set");
            let output = tc.output.as_ref().unwrap();
            assert!(
                output.get("temperature").is_some(),
                "Output should contain temperature"
            );
        }
    }

    // ─── convert_history_to_openai_messages tests ───

    fn assistant_msg(parts: Vec<MessagePart>) -> Message {
        Message {
            id: crate::domain::models::messages::MessageId::new(),
            chat_id: ChatId(Uuid::new_v4()),
            role: Role::Assistant,
            created_at: chrono::Utc::now().naive_utc(),
            parts,
        }
    }

    fn tool_call_block(id: &str, output: Value) -> MessagePart {
        MessagePart::ToolCall(ToolCallBlock {
            tool_call_id: id.to_string(),
            tool_name: "search_knowledge_base".into(),
            input: serde_json::json!({ "queries": ["test"] }),
            output: Some(output),
            error: None,
            state: ToolCallState::Done,
        })
    }

    #[test]
    fn converter_expands_assistant_with_tool_calls_and_results() {
        let msg = assistant_msg(vec![
            MessagePart::Text(TextBlock {
                text: "Found relevant chunks".into(),
            }),
            tool_call_block("c1", serde_json::json!({ "chunks": ["a"] })),
            tool_call_block("c2", serde_json::json!({ "chunks": ["b"] })),
        ]);

        let out = super::convert_history_to_openai_messages(&[msg], false);
        let json: Vec<Value> = out
            .iter()
            .map(|m| serde_json::to_value(m).unwrap())
            .collect();

        // assistant + tool + tool
        assert_eq!(json.len(), 3, "Expected assistant + 2 tool messages");

        // No two consecutive assistant messages
        for pair in json.windows(2) {
            let r1 = pair[0]["role"].as_str().unwrap();
            let r2 = pair[1]["role"].as_str().unwrap();
            assert!(
                !(r1 == "assistant" && r2 == "assistant"),
                "Found two consecutive assistant messages: {r1}, {r2}"
            );
        }

        // Assistant message has content and tool_calls
        assert_eq!(json[0]["role"], "assistant");
        assert_eq!(json[0]["content"], "Found relevant chunks");
        assert_eq!(json[0]["tool_calls"][0]["id"], "c1");
        assert_eq!(
            json[0]["tool_calls"][0]["function"]["name"],
            "search_knowledge_base"
        );
        assert_eq!(json[0]["tool_calls"][1]["id"], "c2");

        // Each tool call has a matching tool result
        assert_eq!(json[1]["role"], "tool");
        assert_eq!(json[1]["tool_call_id"], "c1");
        assert!(json[1]["content"].as_str().unwrap().contains("chunks"));
        assert_eq!(json[2]["role"], "tool");
        assert_eq!(json[2]["tool_call_id"], "c2");
    }

    #[test]
    fn converter_preserves_reasoning_only_message() {
        let msg = assistant_msg(vec![MessagePart::Reasoning(ReasoningBlock {
            text: "thinking".into(),
        })]);

        let out = super::convert_history_to_openai_messages(&[msg], true);
        let json: Vec<Value> = out
            .iter()
            .map(|m| serde_json::to_value(m).unwrap())
            .collect();

        assert_eq!(json.len(), 1, "Expected single assistant message");
        assert_eq!(json[0]["role"], "assistant");
        assert_eq!(json[0]["content"], "<think>\nthinking\n</think>\n\n");
    }

    #[test]
    fn converter_wraps_reasoning_in_think_tags() {
        let msg = assistant_msg(vec![
            MessagePart::Reasoning(ReasoningBlock {
                text: "thinking".into(),
            }),
            MessagePart::Text(TextBlock {
                text: "The answer".into(),
            }),
        ]);

        let out = super::convert_history_to_openai_messages(&[msg], true);
        let json: Vec<Value> = out
            .iter()
            .map(|m| serde_json::to_value(m).unwrap())
            .collect();

        assert_eq!(json.len(), 1);
        assert_eq!(json[0]["role"], "assistant");
        assert_eq!(
            json[0]["content"],
            "<think>\nthinking\n</think>\n\nThe answer"
        );
    }

    #[test]
    fn converter_wraps_reasoning_with_tool_call() {
        let msg = assistant_msg(vec![
            MessagePart::Reasoning(ReasoningBlock {
                text: "thinking".into(),
            }),
            tool_call_block("c1", serde_json::json!({ "chunks": ["a"] })),
        ]);

        let out = super::convert_history_to_openai_messages(&[msg], true);
        let json: Vec<Value> = out
            .iter()
            .map(|m| serde_json::to_value(m).unwrap())
            .collect();

        assert_eq!(json.len(), 2, "Expected assistant + tool");
        assert_eq!(json[0]["role"], "assistant");
        assert_eq!(json[0]["content"], "<think>\nthinking\n</think>\n\n");
        assert_eq!(json[0]["tool_calls"][0]["id"], "c1");
        assert_eq!(json[1]["role"], "tool");
    }

    #[test]
    fn converter_does_not_preserve_reasoning_when_disabled() {
        let msg = assistant_msg(vec![
            MessagePart::Reasoning(ReasoningBlock {
                text: "thinking".into(),
            }),
            MessagePart::Text(TextBlock {
                text: "The answer".into(),
            }),
        ]);

        let out = super::convert_history_to_openai_messages(&[msg], false);
        let json: Vec<Value> = out
            .iter()
            .map(|m| serde_json::to_value(m).unwrap())
            .collect();

        assert_eq!(json.len(), 1);
        assert_eq!(json[0]["role"], "assistant");
        assert_eq!(json[0]["content"], "The answer");
    }

    #[test]
    fn converter_preserves_user_messages() {
        let mut user = Message {
            id: crate::domain::models::messages::MessageId::new(),
            chat_id: ChatId(Uuid::new_v4()),
            role: Role::User,
            created_at: chrono::Utc::now().naive_utc(),
            parts: vec![],
        };
        user.parts.push(MessagePart::Text(TextBlock {
            text: "What is Rust?".into(),
        }));

        let out = super::convert_history_to_openai_messages(&[user], false);
        let json: Vec<Value> = out
            .iter()
            .map(|m| serde_json::to_value(m).unwrap())
            .collect();
        assert_eq!(json.len(), 1);
        assert_eq!(json[0]["role"], "user");
        assert_eq!(json[0]["content"], "What is Rust?");
    }

    #[test]
    fn converter_handles_tool_error_output() {
        let msg = assistant_msg(vec![MessagePart::ToolCall(ToolCallBlock {
            tool_call_id: "c1".into(),
            tool_name: "extract_document".into(),
            input: serde_json::json!({ "document_id": "doc-1" }),
            output: None,
            error: Some("document not found".into()),
            state: ToolCallState::Error,
        })]);

        let out = super::convert_history_to_openai_messages(&[msg], false);
        let json: Vec<Value> = out
            .iter()
            .map(|m| serde_json::to_value(m).unwrap())
            .collect();
        assert_eq!(json.len(), 2);
        assert_eq!(json[1]["role"], "tool");
        assert_eq!(json[1]["tool_call_id"], "c1");
        assert!(
            json[1]["content"]
                .as_str()
                .unwrap()
                .contains("document not found"),
            "Tool error should be propagated to the model"
        );
    }

    #[test]
    fn converter_keeps_consecutive_text_only_assistants_separate() {
        let text1 = assistant_msg(vec![MessagePart::Text(TextBlock {
            text: "round 1".into(),
        })]);
        let text2 = assistant_msg(vec![MessagePart::Text(TextBlock {
            text: "round 2".into(),
        })]);

        let out = super::convert_history_to_openai_messages(&[text1, text2], false);
        let json: Vec<Value> = out
            .iter()
            .map(|m| serde_json::to_value(m).unwrap())
            .collect();

        // No merging: each assistant message stays separate.
        assert_eq!(json.len(), 2, "Expected two separate assistant messages");
        assert_eq!(json[0]["role"], "assistant");
        assert_eq!(json[0]["content"], "round 1");
        assert_eq!(json[1]["role"], "assistant");
        assert_eq!(json[1]["content"], "round 2");
    }

    #[test]
    fn converter_preserves_step_boundaries_within_message() {
        // Reproduces the log bug: a round produced [tool_call, text] flattened into
        // one message. The tool call and the final text belong to different LLM
        // steps and must not be bundled into a single assistant message.
        let msg = assistant_msg(vec![
            tool_call_block("c1", serde_json::json!({ "chunks": ["a"] })),
            MessagePart::Text(TextBlock {
                text: "Full answer".into(),
            }),
        ]);

        let out = super::convert_history_to_openai_messages(&[msg], false);
        let json: Vec<Value> = out
            .iter()
            .map(|m| serde_json::to_value(m).unwrap())
            .collect();

        assert_eq!(
            json.len(),
            3,
            "Expected assistant(tools) + tool + assistant(text)"
        );

        assert_eq!(json[0]["role"], "assistant");
        assert_eq!(json[0]["content"], "");
        assert_eq!(json[0]["tool_calls"][0]["id"], "c1");

        assert_eq!(json[1]["role"], "tool");
        assert_eq!(json[1]["tool_call_id"], "c1");

        assert_eq!(json[2]["role"], "assistant");
        assert_eq!(json[2]["content"], "Full answer");
    }

    #[test]
    fn converter_keeps_tool_calls_and_following_text_separate() {
        // assistant with tool calls (in one message) followed by a text-only
        // assistant (another message) must produce assistant(tools) + tool + assistant(text).
        let first = assistant_msg(vec![tool_call_block("c1", serde_json::json!({ "x": 1 }))]);
        let second = assistant_msg(vec![MessagePart::Text(TextBlock {
            text: "synthesis".into(),
        })]);

        let out = super::convert_history_to_openai_messages(&[first, second], false);
        let json: Vec<Value> = out
            .iter()
            .map(|m| serde_json::to_value(m).unwrap())
            .collect();

        assert_eq!(json.len(), 3, "Expected assistant + tool + assistant");
        assert_eq!(json[0]["role"], "assistant");
        assert_eq!(json[0]["content"], "");
        assert_eq!(json[0]["tool_calls"][0]["id"], "c1");
        assert_eq!(json[1]["role"], "tool");
        assert_eq!(json[1]["tool_call_id"], "c1");
        assert_eq!(json[2]["role"], "assistant");
        assert_eq!(json[2]["content"], "synthesis");
    }

    #[test]
    fn converter_merges_text_before_tool_calls_into_same_assistant() {
        // Text that precedes a tool call in the same step belongs to the same
        // assistant message carrying that tool call.
        let msg = assistant_msg(vec![
            MessagePart::Text(TextBlock {
                text: "I will search".into(),
            }),
            tool_call_block("c1", serde_json::json!({ "chunks": ["a"] })),
        ]);

        let out = super::convert_history_to_openai_messages(&[msg], false);
        let json: Vec<Value> = out
            .iter()
            .map(|m| serde_json::to_value(m).unwrap())
            .collect();

        assert_eq!(json.len(), 2, "Expected assistant(text + tools) + tool");
        assert_eq!(json[0]["role"], "assistant");
        assert_eq!(json[0]["content"], "I will search");
        assert_eq!(json[0]["tool_calls"][0]["id"], "c1");
        assert_eq!(json[1]["role"], "tool");
    }

    // ─── Accumulator tool-call id consistency tests ───

    fn tool_chunk(
        index: u32,
        id: Option<&str>,
        name: Option<&str>,
        arguments: Option<&str>,
    ) -> ChatCompletionMessageToolCallChunk {
        ChatCompletionMessageToolCallChunk {
            index,
            id: id.map(String::from),
            r#type: None,
            function: Some(FunctionCallStream {
                name: name.map(String::from),
                arguments: arguments.map(String::from),
            }),
        }
    }

    fn delta_with_tools(chunks: Vec<ChatCompletionMessageToolCallChunk>) -> CustomDelta {
        CustomDelta {
            content: None,
            reasoning_content: None,
            tool_calls: Some(chunks),
        }
    }

    fn collect_tool_events(
        chunks: Vec<ChatCompletionMessageToolCallChunk>,
    ) -> Vec<MessageStreamEvent> {
        let mut acc = Accumulator::new(ChatId(Uuid::new_v4()));
        acc.process_delta(&delta_with_tools(chunks))
    }

    #[test]
    fn accumulator_emits_consistent_tool_call_ids_when_id_comes_first() {
        let events = collect_tool_events(vec![
            tool_chunk(
                0,
                Some("call_1"),
                Some("search_knowledge_base"),
                Some("{\"queries\":"),
            ),
            tool_chunk(0, None, None, Some("[\"rust\"]}")),
        ]);

        let starts: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                MessageStreamEvent::ToolCallStart { tool_call_id, .. } => {
                    Some(tool_call_id.as_str())
                },
                _ => None,
            })
            .collect();
        assert_eq!(starts, vec!["call_1"], "Exactly one start with the real id");

        for e in &events {
            match e {
                MessageStreamEvent::ToolCallStart { tool_call_id, .. }
                | MessageStreamEvent::ToolCallDelta { tool_call_id, .. } => {
                    assert_eq!(tool_call_id, "call_1");
                },
                _ => {},
            }
        }

        let deltas: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                MessageStreamEvent::ToolCallDelta {
                    input_text_delta, ..
                } => Some(input_text_delta.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(deltas.join(""), "{\"queries\":[\"rust\"]}");
    }

    #[test]
    fn accumulator_defers_start_until_tool_call_id_arrives() {
        let events = collect_tool_events(vec![
            tool_chunk(
                0,
                None,
                Some("search_knowledge_base"),
                Some("{\"queries\":"),
            ),
            tool_chunk(0, Some("call_9"), None, Some("[\"rust\"]}")),
        ]);

        let starts: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                MessageStreamEvent::ToolCallStart { tool_call_id, .. } => {
                    Some(tool_call_id.as_str())
                },
                _ => None,
            })
            .collect();
        assert_eq!(
            starts,
            vec!["call_9"],
            "Never emit a start with an empty id"
        );

        let ids: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                MessageStreamEvent::ToolCallStart { tool_call_id, .. }
                | MessageStreamEvent::ToolCallDelta { tool_call_id, .. } => {
                    Some(tool_call_id.as_str())
                },
                _ => None,
            })
            .collect();
        assert!(!ids.is_empty());
        assert!(
            ids.iter().all(|&id| id == "call_9"),
            "All tool events must use the final id: {ids:?}"
        );

        let deltas: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                MessageStreamEvent::ToolCallDelta {
                    input_text_delta, ..
                } => Some(input_text_delta.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(
            deltas.join(""),
            "{\"queries\":[\"rust\"]}",
            "Start must carry the full buffered arguments"
        );
    }
}
