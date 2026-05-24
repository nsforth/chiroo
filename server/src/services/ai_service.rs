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
use crate::domain::models::stream_event::StreamEvent;

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

struct Accumulator {
    chat_id: ChatId,
    text_buffer: String,
    text_id: String,
    has_text: bool,
    reasoning_buffer: String,
    reasoning_id: String,
    has_reasoning: bool,
    tool_calls: HashMap<u32, PendingToolCall>,
}

struct PendingToolCall {
    id: String,
    name: String,
    arguments: String,
    has_started: bool,
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
    ) -> impl futures::Stream<Item = StreamEvent> + Send {
        let client = self.client.clone();
        let model = Arc::clone(&self.model);
        let user_message = user_message.to_string();
        let chat_id_val = chat_id;
        let (tx, rx) = mpsc::channel(64);

        tokio::spawn(async move {
            if let Err(e) = run_stream(client, model, user_message, chat_id_val, tx.clone()).await {
                tracing::error!("AI stream error: {e:?}");
                let _ = tx
                    .send(StreamEvent::Error {
                        error_text: e.to_string(),
                    })
                    .await;
            }
        });

        ReceiverStream::new(rx)
    }
}

async fn run_stream(
    client: Client<OpenAIConfig>,
    model: Arc<String>,
    user_message: String,
    chat_id: ChatId,
    tx: mpsc::Sender<StreamEvent>,
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
        let _ = tx.send(StreamEvent::LlmCallStart).await;

        let request = CreateChatCompletionRequestArgs::default()
            .reasoning_effort(ReasoningEffort::High)
            .model((*model).clone())
            .messages(messages.clone())
            .tools(tools.clone())
            .stream(true)
            .build()?;

        let mut stream: CustomStream = client.chat().create_stream_byot(request).await?;

        let mut acc = Accumulator::new(chat_id);

        while let Some(result) = stream.next().await {
            let response = result?;

            for choice in response.choices {
                println!("{:?}", choice);
                // ── reasoning ──
                if let Some(rc) = &choice.delta.reasoning_content {
                    if !rc.is_empty() {
                        if !acc.has_reasoning {
                            acc.reasoning_id = Uuid::new_v4().to_string();
                            let _ = tx
                                .send(StreamEvent::ReasoningStart {
                                    id: acc.reasoning_id.clone(),
                                })
                                .await;
                            acc.has_reasoning = true;
                        }
                        acc.reasoning_buffer.push_str(rc);
                        let _ = tx
                            .send(StreamEvent::ReasoningDelta {
                                id: acc.reasoning_id.clone(),
                                delta: rc.clone(),
                            })
                            .await;
                    }
                }

                // ── текст ──
                if let Some(content) = &choice.delta.content {
                    if !content.is_empty() {
                        // finish reasoning before first text
                        if acc.has_reasoning {
                            let _ = tx
                                .send(StreamEvent::ReasoningEnd {
                                    id: acc.reasoning_id.clone(),
                                })
                                .await;
                            acc.has_reasoning = false;
                        }
                        if !acc.has_text {
                            acc.text_id = Uuid::new_v4().to_string();
                            let _ = tx
                                .send(StreamEvent::TextStart {
                                    id: acc.text_id.clone(),
                                })
                                .await;
                            acc.has_text = true;
                        }
                        acc.text_buffer.push_str(content);
                        let _ = tx
                            .send(StreamEvent::TextDelta {
                                id: acc.text_id.clone(),
                                delta: content.clone(),
                            })
                            .await;
                    }
                }

                // ── тулы ──
                if let Some(tool_chunks) = choice.delta.tool_calls {
                    for chunk in tool_chunks {
                        let entry =
                            acc.tool_calls
                                .entry(chunk.index)
                                .or_insert_with(|| PendingToolCall {
                                    id: chunk.id.clone().unwrap_or_default(),
                                    name: String::new(),
                                    arguments: String::new(),
                                    has_started: false,
                                });

                        if let Some(id) = &chunk.id {
                            entry.id = id.clone();
                        }
                        if let Some(func) = &chunk.function {
                            if let Some(name) = &func.name {
                                entry.name = name.clone();
                            }
                            if let Some(args) = &func.arguments {
                                if !entry.has_started {
                                    let _ = tx
                                        .send(StreamEvent::ToolCallStart {
                                            tool_call_id: entry.id.clone(),
                                            tool_name: entry.name.clone(),
                                            title: Some(format!("Calling {}", entry.name)),
                                        })
                                        .await;
                                    entry.has_started = true;
                                }
                                entry.arguments.push_str(args);
                                let _ = tx
                                    .send(StreamEvent::ToolCallDelta {
                                        tool_call_id: entry.id.clone(),
                                        input_text_delta: args.clone(),
                                    })
                                    .await;
                            }
                        }
                    }
                }

                // ── финиш LLM-вызова ──
                if let Some(reason) = choice.finish_reason {
                    if acc.has_reasoning {
                        let _ = tx
                            .send(StreamEvent::ReasoningEnd {
                                id: acc.reasoning_id.clone(),
                            })
                            .await;
                        acc.has_reasoning = false;
                    }
                    if acc.has_text {
                        let _ = tx
                            .send(StreamEvent::TextEnd {
                                id: acc.text_id.clone(),
                            })
                            .await;
                    }

                    let message = acc.finish(chat_id, &reason);

                    // Emit ToolCallAvailable for each completed tool
                    let completed_tools: Vec<ChatCompletionMessageToolCall> = acc
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

                    for tc in &completed_tools {
                        let input: serde_json::Value =
                            serde_json::from_str(&tc.function.arguments).unwrap_or_default();
                        let _ = tx
                            .send(StreamEvent::ToolCallAvailable {
                                tool_call_id: tc.id.clone(),
                                tool_name: tc.function.name.clone(),
                                input,
                            })
                            .await;
                    }

                    match reason {
                        FinishReason::ToolCalls => {
                            // сообщение assistant сохраняем в контекст
                            let assistant_msg =
                                assistant_message_with_tools(&completed_tools, &message);
                            messages.push(assistant_msg);

                            // выполняем тулы замоканно
                            for tc in &completed_tools {
                                match execute_mock_tool(&tc.function.name, &tc.function.arguments) {
                                    Ok(output) => {
                                        messages.push(tool_result_message(&tc.id, &output));
                                        let _ = tx
                                            .send(StreamEvent::ToolResult {
                                                tool_call_id: tc.id.clone(),
                                                output,
                                            })
                                            .await;
                                    },
                                    Err(err_text) => {
                                        let _ = tx
                                            .send(StreamEvent::ToolError {
                                                tool_call_id: tc.id.clone(),
                                                error_text: err_text.clone(),
                                            })
                                            .await;
                                        messages.push(
                                            ChatCompletionRequestToolMessageArgs::default()
                                                .tool_call_id(tc.id.clone())
                                                .content(err_text)
                                                .build()?
                                                .into(),
                                        );
                                    },
                                }
                            }
                            // продолжить цикл LLM-вызовов
                            continue;
                        },
                        FinishReason::Stop => {
                            let assistant_msg = assistant_message_without_tools(&message);
                            messages.push(assistant_msg);
                            // сообщение готово
                            let _ = tx
                                .send(StreamEvent::LlmCallFinish {
                                    finish_reason: "stop".into(),
                                    message,
                                })
                                .await;
                            return Ok(());
                        },
                        _ => {
                            let _ = tx
                                .send(StreamEvent::Error {
                                    error_text: format!("Unexpected finish: {reason:?}"),
                                })
                                .await;
                            return Ok(());
                        },
                    }
                }
            }
        }

        // Если стрим закончился без finish_reason — что-то пошло не так
        return Ok(());
    }

    Ok(())
}

impl Accumulator {
    fn new(chat_id: ChatId) -> Self {
        Self {
            chat_id,
            text_buffer: String::new(),
            text_id: String::new(),
            has_text: false,
            reasoning_buffer: String::new(),
            reasoning_id: String::new(),
            has_reasoning: false,
            tool_calls: HashMap::new(),
        }
    }

    fn finish(&mut self, chat_id: ChatId, reason: &FinishReason) -> Message {
        let mut message = Message::new(chat_id, Role::Assistant);

        if !self.reasoning_buffer.is_empty() {
            message.parts.push(MessagePart::Reasoning(ReasoningBlock {
                text: std::mem::take(&mut self.reasoning_buffer),
            }));
        }

        if self.has_text && !self.text_buffer.is_empty() {
            message.parts.push(MessagePart::Text(TextBlock {
                text: std::mem::take(&mut self.text_buffer),
            }));
            self.has_text = false;
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
                    state: ToolCallState::InputAvailable,
                }));
            }
        }

        message
    }
}

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
