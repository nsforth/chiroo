use async_openai::config::OpenAIConfig;
use async_openai::types::chat::{
    ChatCompletionMessageToolCall, ChatCompletionMessageToolCallChunk,
    ChatCompletionMessageToolCalls, ChatCompletionRequestAssistantMessageArgs,
    ChatCompletionRequestMessage, ChatCompletionRequestToolMessageArgs,
    ChatCompletionRequestUserMessageArgs, ChatCompletionTool, ChatCompletionTools,
    CreateChatCompletionRequestArgs, FinishReason, FunctionObject, ReasoningEffort,
};
use async_openai::Client;
use futures::StreamExt;
use secrecy::ExposeSecret;
use serde::Deserialize;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::config::OpenAiConfig;
use crate::domain::models::chat::ChatId;
use crate::domain::models::messages::{
    Message, MessagePart, ReasoningBlock, Role, TextBlock, ToolCallBlock, ToolCallState,
};
use crate::domain::models::stream_event::MessageStreamEvent;

#[derive(Debug, Deserialize)]
struct CustomDelta {
    content: Option<String>,
    #[serde(rename = "reasoning_content", default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ChatCompletionMessageToolCallChunk>>,
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

#[derive(Clone)]
pub struct AiService {
    client: Client<OpenAIConfig>,
    model: Arc<String>,
}

impl AiService {
    pub fn new(config: &OpenAiConfig) -> Self {
        let openai_config = OpenAIConfig::new()
            .with_api_base(config.base_url())
            .with_api_key(config.api_key().expose_secret());

        Self {
            client: Client::with_config(openai_config),
            model: Arc::new(config.model()),
        }
    }

    pub fn stream(
        &self,
        chat_id: ChatId,
        user_message: &str,
    ) -> impl futures::Stream<Item = MessageStreamEvent> + Send {
        let client = self.client.clone();
        let model = self.model.to_string();
        let user_message = user_message.to_string();
        let (tx, rx) = mpsc::channel(64);

        tokio::spawn(async move {
            if let Err(e) = run_stream(client, model, user_message, chat_id, tx.clone()).await {
                tracing::error!("AI stream error: {e:?}");
                let _ = tx
                    .send(MessageStreamEvent::Error {
                        error_text: e.to_string(),
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
        let mut events = Vec::new();

        if let Some(rc) = &delta.reasoning_content {
            if !rc.is_empty() {
                self.enter_reasoning(&mut events);
                self.reasoning.push_str(rc);
                if let AccState::Reasoning { id } = &self.state {
                    events.push(MessageStreamEvent::ReasoningDelta {
                        id: id.clone(),
                        delta: rc.clone(),
                    });
                }
            }
        }

        if let Some(content) = &delta.content {
            if !content.is_empty() {
                self.enter_text(&mut events);
                self.text.push_str(content);
                if let AccState::Text { id } = &self.state {
                    events.push(MessageStreamEvent::TextDelta {
                        id: id.clone(),
                        delta: content.clone(),
                    });
                }
            }
        }

        if let Some(tool_chunks) = &delta.tool_calls {
            self.enter_tool(&mut events);
            for chunk in tool_chunks {
                let entry = self
                    .tool_calls
                    .entry(chunk.index)
                    .or_insert_with(|| PendingToolCall {
                        id: chunk.id.clone().unwrap_or_default(),
                        name: String::new(),
                        arguments: String::new(),
                    });

                if let Some(id) = &chunk.id {
                    entry.id = id.clone();
                }
                if let Some(func) = &chunk.function {
                    if let Some(name) = &func.name {
                        entry.name = name.clone();
                    }
                    if let Some(args) = &func.arguments {
                        let is_new = entry.arguments.is_empty();
                        entry.arguments.push_str(args);
                        if is_new {
                            events.push(MessageStreamEvent::ToolCallStart {
                                tool_call_id: entry.id.clone(),
                                tool_name: entry.name.clone(),
                                title: Some(format!("Calling {}", entry.name)),
                            });
                        }
                        events.push(MessageStreamEvent::ToolCallDelta {
                            tool_call_id: entry.id.clone(),
                            input_text_delta: args.clone(),
                        });
                    }
                }
            }
        }

        events
    }

    fn finish(&mut self, reason: &FinishReason) -> (Vec<MessageStreamEvent>, AccumulatorResult) {
        let mut events = Vec::new();

        match &self.state {
            AccState::Reasoning { id } => {
                events.push(MessageStreamEvent::ReasoningEnd { id: id.clone() });
            }
            AccState::Text { id } => {
                events.push(MessageStreamEvent::TextEnd { id: id.clone() });
            }
            AccState::Initial | AccState::Tool => {}
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
        if matches!(reason, FinishReason::ToolCalls) {
            for tc in self.tool_calls.values() {
                let input: serde_json::Value =
                    serde_json::from_str(&tc.arguments).unwrap_or_default();
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

        (events, AccumulatorResult {
            message,
            completed_tools,
        })
    }

    fn enter_reasoning(&mut self, events: &mut Vec<MessageStreamEvent>) {
        match &self.state {
            AccState::Reasoning { .. } => return,
            AccState::Text { id } => {
                events.push(MessageStreamEvent::TextEnd { id: id.clone() });
            }
            AccState::Tool | AccState::Initial => {}
        }
        let id = Uuid::new_v4().to_string();
        events.push(MessageStreamEvent::ReasoningStart { id: id.clone() });
        self.state = AccState::Reasoning { id };
    }

    fn enter_text(&mut self, events: &mut Vec<MessageStreamEvent>) {
        match &self.state {
            AccState::Text { .. } => return,
            AccState::Reasoning { id } => {
                events.push(MessageStreamEvent::ReasoningEnd { id: id.clone() });
            }
            AccState::Tool | AccState::Initial => {}
        }
        let id = Uuid::new_v4().to_string();
        events.push(MessageStreamEvent::TextStart { id: id.clone() });
        self.state = AccState::Text { id };
    }

    fn enter_tool(&mut self, events: &mut Vec<MessageStreamEvent>) {
        match &self.state {
            AccState::Tool => return,
            AccState::Reasoning { id } => {
                events.push(MessageStreamEvent::ReasoningEnd { id: id.clone() });
            }
            AccState::Text { id } => {
                events.push(MessageStreamEvent::TextEnd { id: id.clone() });
            }
            AccState::Initial => {}
        }
        self.state = AccState::Tool;
    }
}

// ─── Stream orchestration ───

enum StepResult {
    Stop { message: Message },
    ToolCalls {
        message: Message,
        completed_tools: Vec<ChatCompletionMessageToolCall>,
    },
}

async fn send_or_bail(
    tx: &mpsc::Sender<MessageStreamEvent>,
    event: MessageStreamEvent,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tx.send(event)
        .await
        .map_err(|_| "client disconnected")?;
    Ok(())
}

async fn run_stream(
    client: Client<OpenAIConfig>,
    model: String,
    user_message: String,
    chat_id: ChatId,
    tx: mpsc::Sender<MessageStreamEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut messages: Vec<ChatCompletionRequestMessage> =
        vec![ChatCompletionRequestUserMessageArgs::default()
            .content(user_message)
            .build()?
            .into()];

    let tools = vec![ChatCompletionTools::Function(ChatCompletionTool {
        function: FunctionObject {
            name: "weather".into(),
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
    })];

    let max_steps: usize = 5;

    for _step in 0..max_steps {
        send_or_bail(&tx, MessageStreamEvent::LlmCallStart).await?;

        match run_llm_step(client.clone(), model.clone(), &messages, &tools, chat_id, &tx).await {
            Ok(StepResult::Stop { message }) => {
                messages.push(assistant_message_without_tools(&message));
                send_or_bail(
                    &tx,
                    MessageStreamEvent::LlmCallFinish {
                        finish_reason: "stop".into(),
                        message,
                    },
                )
                .await?;
                return Ok(());
            }
            Ok(StepResult::ToolCalls {
                message,
                completed_tools,
            }) => {
                messages.push(assistant_message_with_tools(&completed_tools, &message));
                send_or_bail(
                    &tx,
                    MessageStreamEvent::LlmCallFinish {
                        finish_reason: "tool-calls".into(),
                        message,
                    },
                )
                .await?;

                for tc in &completed_tools {
                    match execute_mock_tool(&tc.function.name, &tc.function.arguments) {
                        Ok(output) => {
                            messages.push(tool_result_message(&tc.id, &output));
                            send_or_bail(
                                &tx,
                                MessageStreamEvent::ToolResult {
                                    tool_call_id: tc.id.clone(),
                                    output,
                                },
                            )
                            .await?;
                        }
                        Err(err_text) => {
                            send_or_bail(
                                &tx,
                                MessageStreamEvent::ToolError {
                                    tool_call_id: tc.id.clone(),
                                    error_text: err_text.clone(),
                                },
                            )
                            .await?;
                            messages.push(
                                ChatCompletionRequestToolMessageArgs::default()
                                    .tool_call_id(tc.id.clone())
                                    .content(err_text)
                                    .build()?
                                    .into(),
                            );
                        }
                    }
                }
            }
            Err(e) => {
                let _ = tx
                    .send(MessageStreamEvent::Error {
                        error_text: e.to_string(),
                    })
                    .await;
                return Ok(());
            }
        }
    }

    let _ = tx
        .send(MessageStreamEvent::Error {
            error_text: "Max steps reached".into(),
        })
        .await;

    Ok(())
}

async fn run_llm_step(
    client: Client<OpenAIConfig>,
    model: String,
    messages: &[ChatCompletionRequestMessage],
    tools: &[ChatCompletionTools],
    chat_id: ChatId,
    tx: &mpsc::Sender<MessageStreamEvent>,
) -> Result<StepResult, Box<dyn std::error::Error + Send + Sync>> {
    let request = CreateChatCompletionRequestArgs::default()
        .reasoning_effort(ReasoningEffort::High)
        .model(model)
        .messages(messages.to_vec())
        .tools(tools.to_vec())
        .stream(true)
        .build()?;

    let mut stream: CustomStream = client.chat().create_stream_byot(request).await?;
    let mut acc = Accumulator::new(chat_id);

    while let Some(result) = stream.next().await {
        let response = result?;

        for choice in response.choices {
            for event in acc.process_delta(&choice.delta) {
                send_or_bail(tx, event).await?;
            }

            if let Some(reason) = &choice.finish_reason {
                let (end_events, result) = acc.finish(reason);
                for event in end_events {
                    send_or_bail(tx, event).await?;
                }

                for tc in &result.completed_tools {
                    let input: serde_json::Value =
                        serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                    send_or_bail(
                        tx,
                        MessageStreamEvent::ToolCallAvailable {
                            tool_call_id: tc.id.clone(),
                            tool_name: tc.function.name.clone(),
                            input,
                        },
                    )
                    .await?;
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
                        return Err(
                            format!("Unexpected finish reason: {reason:?}").into(),
                        );
                    }
                });
            }
        }
    }

    Err("Stream ended without finish_reason".into())
}

// ─── OpenAI request helpers ───

fn assistant_message_with_tools(
    tool_calls: &[ChatCompletionMessageToolCall],
    _message: &Message,
) -> ChatCompletionRequestMessage {
    let tool_calls: Vec<ChatCompletionMessageToolCalls> = tool_calls
        .iter()
        .map(|tc| ChatCompletionMessageToolCalls::Function(tc.clone()))
        .collect();

    ChatCompletionRequestAssistantMessageArgs::default()
        .tool_calls(tool_calls)
        .build()
        .unwrap()
        .into()
}

fn assistant_message_without_tools(message: &Message) -> ChatCompletionRequestMessage {
    let content = message
        .parts
        .iter()
        .filter_map(|p| match p {
            MessagePart::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    ChatCompletionRequestAssistantMessageArgs::default()
        .content(content)
        .build()
        .unwrap()
        .into()
}

fn tool_result_message(
    tool_call_id: &str,
    output: &serde_json::Value,
) -> ChatCompletionRequestMessage {
    ChatCompletionRequestToolMessageArgs::default()
        .tool_call_id(tool_call_id)
        .content(output.to_string())
        .build()
        .unwrap()
        .into()
}

fn execute_mock_tool(name: &str, arguments: &str) -> Result<serde_json::Value, String> {
    if name != "weather" {
        return Err(format!("Unknown tool: {name}"));
    }

    let _input: serde_json::Value =
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
