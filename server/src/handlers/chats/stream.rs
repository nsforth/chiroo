use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::{self, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::time::Duration;
use uuid::Uuid;

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

    #[serde(rename = "source-url")]
    SourceUrl {
        #[serde(rename = "sourceId")]
        source_id: String,
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    #[serde(rename = "source-document")]
    SourceDocument {
        #[serde(rename = "sourceId")]
        source_id: String,
        #[serde(rename = "mediaType")]
        media_type: String,
        title: String,
    },

    #[serde(rename = "file")]
    File {
        url: String,
        #[serde(rename = "mediaType")]
        media_type: String,
    },

    #[serde(rename = "data-chat-title")]
    DataChatTitle {
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        data: serde_json::Value,
    },

    #[serde(rename = "finish")]
    Finish {
        #[serde(skip_serializing_if = "Option::is_none", rename = "finishReason")]
        finish_reason: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    #[allow(dead_code)]
    pub id: Uuid,
    #[allow(dead_code)]
    pub messages: Vec<serde_json::Value>,
    #[allow(dead_code)]
    #[serde(default)]
    pub trigger: Option<String>,
    #[allow(dead_code)]
    #[serde(default, rename = "messageId")]
    pub message_id: Option<Uuid>,
    #[allow(dead_code)]
    #[serde(default)]
    pub model: Option<String>,
}

fn ms(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

fn chunk(chunk: SseChunk, delay_ms: u64) -> (Duration, SseChunk) {
    (ms(delay_ms), chunk)
}

fn build_mock_response() -> Vec<(Duration, SseChunk)> {
    vec![
        chunk(SseChunk::Start { message_id: None }, 100),
        chunk(SseChunk::StartStep, 200),
        // --- reasoning ---
        chunk(
            SseChunk::ReasoningStart {
                id: "rsn_0".into(),
            },
            300,
        ),
        chunk(
            SseChunk::ReasoningDelta {
                id: "rsn_0".into(),
                delta: "I should look up the weather".into(),
            },
            200,
        ),
        chunk(
            SseChunk::ReasoningDelta {
                id: "rsn_0".into(),
                delta: " and then summarize the results.".into(),
            },
            200,
        ),
        chunk(SseChunk::ReasoningEnd { id: "rsn_0".into() }, 100),
        // --- text step 0 ---
        chunk(
            SseChunk::TextStart {
                id: "txt_0".into(),
            },
            200,
        ),
        chunk(
            SseChunk::TextDelta {
                id: "txt_0".into(),
                delta: "Let me ".into(),
            },
            150,
        ),
        chunk(
            SseChunk::TextDelta {
                id: "txt_0".into(),
                delta: "check that for you.".into(),
            },
            150,
        ),
        chunk(SseChunk::TextEnd { id: "txt_0".into() }, 100),
        chunk(SseChunk::FinishStep, 200),
        // --- step 1: weather tool ---
        chunk(SseChunk::StartStep, 200),
        chunk(
            SseChunk::ToolInputStart {
                tool_call_id: "call_w".into(),
                tool_name: "weather".into(),
                title: Some("Fetching weather".into()),
            },
            300,
        ),
        chunk(
            SseChunk::ToolInputDelta {
                tool_call_id: "call_w".into(),
                input_text_delta: r#"{"loc"#.into(),
            },
            100,
        ),
        chunk(
            SseChunk::ToolInputDelta {
                tool_call_id: "call_w".into(),
                input_text_delta: r#"ation":"London"}"#.into(),
            },
            100,
        ),
        chunk(
            SseChunk::ToolInputAvailable {
                tool_call_id: "call_w".into(),
                tool_name: "weather".into(),
                input: serde_json::json!({"location": "London"}),
            },
            200,
        ),
        chunk(
            SseChunk::ToolOutputAvailable {
                tool_call_id: "call_w".into(),
                output: serde_json::json!({
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
                        {"day": "Today", "high": 22, "low": 14, "condition": {"text": "Cloudy", "icon": "i-lucide-cloud"}},
                        {"day": "Thu", "high": 24, "low": 16, "condition": {"text": "Sunny", "icon": "i-lucide-sun"}},
                        {"day": "Fri", "high": 20, "low": 11, "condition": {"text": "Rainy", "icon": "i-lucide-cloud-rain"}}
                    ]
                }),
            },
            500,
        ),
        chunk(SseChunk::FinishStep, 200),
        // --- step 2: final answer + sources + file ---
        chunk(SseChunk::StartStep, 200),
        chunk(
            SseChunk::TextStart {
                id: "txt_1".into(),
            },
            200,
        ),
        chunk(
            SseChunk::TextDelta {
                id: "txt_1".into(),
                delta: "The weather in ".into(),
            },
            150,
        ),
        chunk(
            SseChunk::TextDelta {
                id: "txt_1".into(),
                delta: "London is 18°C ".into(),
            },
            150,
        ),
        chunk(
            SseChunk::TextDelta {
                id: "txt_1".into(),
                delta: "and cloudy with 72% humidity.".into(),
            },
            150,
        ),
        chunk(SseChunk::TextEnd { id: "txt_1".into() }, 100),
        chunk(
            SseChunk::SourceUrl {
                source_id: "src_1".into(),
                url: "https://weather.example.com/london".into(),
                title: Some("Weather Service".into()),
            },
            150,
        ),
        chunk(
            SseChunk::SourceDocument {
                source_id: "doc_1".into(),
                media_type: "text/html".into(),
                title: "London Weather Report".into(),
            },
            150,
        ),
        chunk(
            SseChunk::File {
                url: "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='100' height='100'%3E%3Crect fill='%234285f4' width='100' height='100'/%3E%3Ctext y='.9em' font-size='90'%3E%F0%9F%8C%A4%EF%B8%8F%3C/text%3E%3C/svg%3E".into(),
                media_type: "image/svg+xml".into(),
            },
            150,
        ),
        chunk(
            SseChunk::DataChatTitle {
                id: Some("title_1".into()),
                data: serde_json::json!({"title": "Weather in London"}),
            },
            150,
        ),
        chunk(SseChunk::FinishStep, 100),
        chunk(
            SseChunk::Finish {
                finish_reason: Some("stop".into()),
            },
            50,
        ),
    ]
}

#[axum::debug_handler]
pub async fn send_message(
    State(_state): State<AppState>,
    PathExtractor(_chat_id): PathExtractor<Uuid>,
    JsonExtractor(_body): JsonExtractor<SendMessageRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>> + Send> {
    let events = build_mock_response();

    let stream = stream::iter(events).then(|(delay, chunk)| async move {
        tokio::time::sleep(delay).await;
        let json = serde_json::to_string(&chunk).unwrap_or_default();
        Ok(Event::default().data(json))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
