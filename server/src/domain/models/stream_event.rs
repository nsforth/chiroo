use crate::domain::models::messages::Message;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Error,
    Other,
}

pub enum MessageStreamEvent {
    ReasoningStart {
        id: String,
    },
    ReasoningDelta {
        id: String,
        delta: String,
    },
    ReasoningEnd {
        id: String,
    },

    TextStart {
        id: String,
    },
    TextDelta {
        id: String,
        delta: String,
    },
    TextEnd {
        id: String,
    },

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
        finish_reason: FinishReason,
        message: Message,
    },

    // TODO: Make errors more specific, may bew simple HTTP codes
    Error {
        error_text: String,
    },
}
