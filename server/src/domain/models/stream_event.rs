use crate::domain::models::messages::Message;
use crate::domain::models::source::Source;
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

impl From<async_openai::types::chat::FinishReason> for FinishReason {
    fn from(r: async_openai::types::chat::FinishReason) -> Self {
        use async_openai::types::chat::FinishReason as OaiFinish;
        match r {
            OaiFinish::Stop => Self::Stop,
            OaiFinish::Length => Self::Length,
            OaiFinish::ToolCalls => Self::ToolCalls,
            OaiFinish::ContentFilter => Self::ContentFilter,
            _ => Self::Other,
        }
    }
}

#[derive(Debug)]
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

    SourceUrl(Source),

    LlmCallStart,
    LlmCallFinish {
        finish_reason: FinishReason,
        message: Message,
    },

    /// Marks the start of a round; carries whether the round is hidden from
    /// non-admin users. Emitted once per round, after the first LlmCallStart.
    RoundMarker {
        hidden: bool,
    },

    Error {
        error_text: String,
    },
}

#[cfg(test)]
mod tests {
    use async_openai::types::chat::FinishReason as OaiFinish;

    use super::FinishReason;

    #[test]
    fn oai_finish_reason_mapped() {
        assert_eq!(FinishReason::from(OaiFinish::Stop), FinishReason::Stop);
        assert_eq!(FinishReason::from(OaiFinish::Length), FinishReason::Length);
        assert_eq!(
            FinishReason::from(OaiFinish::ToolCalls),
            FinishReason::ToolCalls
        );
        assert_eq!(
            FinishReason::from(OaiFinish::ContentFilter),
            FinishReason::ContentFilter
        );
    }

    #[test]
    fn oai_finish_reason_unknown_variants_map_to_other() {
        assert_eq!(
            FinishReason::from(OaiFinish::FunctionCall),
            FinishReason::Other
        );
    }
}
