use async_openai::types::chat::{ChatCompletionToolChoiceOption, ToolChoiceOptions};

use crate::domain::models::sampling::{SamplingParams, SamplingPreset};

use super::{RoundDefinition, ScenarioDefinition};

pub fn simple() -> ScenarioDefinition {
    ScenarioDefinition {
        name: "simple",
        sampling_preset: Some(SamplingPreset::INSTRUCT),
        rounds: vec![
            RoundDefinition {
                prompt_template: "simple/search.md".into(),
                tool_names: vec!["search_in_all_kbs".into()],
                max_llm_calls: 1,
                reasoning_effort: None,
                sampling: SamplingParams::default(),
                extra_body: None,
                tool_choice: Some(ChatCompletionToolChoiceOption::Mode(
                    ToolChoiceOptions::Required,
                )),
                enable_thinking: false,
                preserve_thinking: false,
                hidden: true,
            },
            RoundDefinition {
                prompt_template: "simple/extract.md".into(),
                tool_names: vec!["extract_document".into()],
                max_llm_calls: 1,
                reasoning_effort: None,
                sampling: SamplingParams::default(),
                extra_body: None,
                tool_choice: Some(ChatCompletionToolChoiceOption::Mode(
                    ToolChoiceOptions::Required,
                )),
                enable_thinking: false,
                preserve_thinking: false,
                hidden: true,
            },
            RoundDefinition {
                prompt_template: "simple/answer.md".into(),
                tool_names: vec![],
                max_llm_calls: 1,
                reasoning_effort: None,
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
