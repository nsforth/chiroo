use crate::domain::models::messages::Message;
use serde_json::Value;

pub enum StreamEvent {
    // ── Reasoning ──
    ReasoningStart { id: String },
    ReasoningDelta { id: String, delta: String },
    ReasoningEnd { id: String },

    // ── Text ──
    TextStart { id: String },
    TextDelta { id: String, delta: String },
    TextEnd { id: String },

    ToolCallStart {
        tool_call_id: String,
        tool_name: String,
        title: Option<String>,
    },
    ToolCallDelta {
        tool_call_id: String,
        input_text_delta: String,
    },
    ToolCallAvailable {
        tool_call_id: String,
        tool_name: String,
        input: Value,
    },
    ToolResult {
        tool_call_id: String,
        output: Value,
    },
    ToolError {
        tool_call_id: String,
        error_text: String,
    },

    LlmCallStart,
    LlmCallFinish {
        finish_reason: String,
        message: Message,
    },

    Error {
        error_text: String,
    },
}
