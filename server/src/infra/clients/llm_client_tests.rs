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
        tool_name: "search_rag".into(),
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
    assert_eq!(json[0]["tool_calls"][0]["function"]["name"], "search_rag");
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
        tool_name: "extract_page".into(),
        input: serde_json::json!({ "page_ids": ["p1"] }),
        output: None,
        error: Some("page not found".into()),
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
            .contains("page not found"),
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

fn collect_tool_events(chunks: Vec<ChatCompletionMessageToolCallChunk>) -> Vec<MessageStreamEvent> {
    let mut acc = Accumulator::new(ChatId(Uuid::new_v4()));
    acc.process_delta(&delta_with_tools(chunks))
}

#[test]
fn accumulator_emits_consistent_tool_call_ids_when_id_comes_first() {
    let events = collect_tool_events(vec![
        tool_chunk(0, Some("call_1"), Some("search_rag"), Some("{\"queries\":")),
        tool_chunk(0, None, None, Some("[\"rust\"]}")),
    ]);

    let starts: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            MessageStreamEvent::ToolCallStart { tool_call_id, .. } => Some(tool_call_id.as_str()),
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
        tool_chunk(0, None, Some("search_rag"), Some("{\"queries\":")),
        tool_chunk(0, Some("call_9"), None, Some("[\"rust\"]}")),
    ]);

    let starts: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            MessageStreamEvent::ToolCallStart { tool_call_id, .. } => Some(tool_call_id.as_str()),
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
            | MessageStreamEvent::ToolCallDelta { tool_call_id, .. } => Some(tool_call_id.as_str()),
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
