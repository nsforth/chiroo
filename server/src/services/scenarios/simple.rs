use async_openai::types::chat::{
    ChatCompletionNamedToolChoice, ChatCompletionToolChoiceOption, FunctionName, ReasoningEffort,
    ToolChoiceOptions,
};

use crate::{
    domain::models::sampling::{SamplingParams, SamplingPreset},
    services::scenarios::all_tools,
};

use super::{RoundDefinition, ScenarioDefinition};

pub fn simple() -> ScenarioDefinition {
    ScenarioDefinition {
        name: "simple",
        sampling_preset: Some(SamplingPreset::INSTRUCT),
        rounds: vec![
            RoundDefinition {
                prompt_template: "simple/search.md".into(),
                tool_names: all_tools(),
                max_llm_calls: 1,
                reasoning_effort: Some(ReasoningEffort::None),
                sampling: SamplingParams::default(),
                extra_body: None,
                tool_choice: Some(ChatCompletionToolChoiceOption::Function(
                    ChatCompletionNamedToolChoice {
                        function: FunctionName {
                            name: "search_in_all_kbs".into(),
                        },
                    },
                )),
                enable_thinking: false,
                preserve_thinking: false,
                hidden: true,
            },
            RoundDefinition {
                prompt_template: "simple/extract.md".into(),
                tool_names: all_tools(),
                max_llm_calls: 1,
                reasoning_effort: Some(ReasoningEffort::None),
                sampling: SamplingParams::default(),
                extra_body: None,
                tool_choice: Some(ChatCompletionToolChoiceOption::Function(
                    ChatCompletionNamedToolChoice {
                        function: FunctionName {
                            name: "extract_document".into(),
                        },
                    },
                )),
                enable_thinking: false,
                preserve_thinking: false,
                hidden: true,
            },
            RoundDefinition {
                prompt_template: "simple/answer.md".into(),
                tool_names: all_tools(),
                max_llm_calls: 1,
                reasoning_effort: Some(ReasoningEffort::None),
                sampling: SamplingParams::default(),
                extra_body: None,
                tool_choice: Some(ChatCompletionToolChoiceOption::Mode(
                    ToolChoiceOptions::None,
                )),
                enable_thinking: false,
                preserve_thinking: false,
                hidden: false,
            },
        ],
    }
}
