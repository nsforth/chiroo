use async_openai::types::chat::{
    ChatCompletionNamedToolChoice, ChatCompletionToolChoiceOption, FunctionName, ReasoningEffort,
    ToolChoiceOptions,
};

use crate::{
    domain::models::sampling::{SamplingParams, SamplingPreset},
    services::scenarios::all_tools,
};

use super::{RoundDefinition, ScenarioDefinition};

pub fn deep_research() -> ScenarioDefinition {
    ScenarioDefinition {
        name: "deep_research",
        sampling_preset: Some(SamplingPreset::THINKING),
        rounds: vec![
            RoundDefinition {
                prompt_template: "deep_research/search.md".into(),
                tool_names: all_tools(),
                max_llm_calls: 1,
                reasoning_effort: Some(ReasoningEffort::Medium),
                sampling: SamplingParams::default(),
                extra_body: None,
                tool_choice: Some(ChatCompletionToolChoiceOption::Function(
                    ChatCompletionNamedToolChoice {
                        function: FunctionName {
                            name: "search_in_all_kbs".into(),
                        },
                    },
                )),
                enable_thinking: true,
                preserve_thinking: true,
                hidden: true,
            },
            RoundDefinition {
                prompt_template: "deep_research/explore.md".into(),
                tool_names: all_tools(),
                max_llm_calls: 10,
                reasoning_effort: Some(ReasoningEffort::Medium),
                sampling: SamplingParams::default(),
                extra_body: None,
                tool_choice: Some(ChatCompletionToolChoiceOption::Mode(
                    ToolChoiceOptions::Auto,
                )),
                enable_thinking: true,
                preserve_thinking: true,
                hidden: true,
            },
            RoundDefinition {
                prompt_template: "deep_research/answer.md".into(),
                tool_names: all_tools(),
                max_llm_calls: 1,
                reasoning_effort: Some(ReasoningEffort::Medium),
                sampling: SamplingParams::default(),
                extra_body: None,
                tool_choice: Some(ChatCompletionToolChoiceOption::Mode(
                    ToolChoiceOptions::None,
                )),
                enable_thinking: true,
                preserve_thinking: true,
                hidden: false,
            },
        ],
    }
}
