use async_openai::types::chat::{
    ChatCompletionToolChoiceOption, ReasoningEffort, ToolChoiceOptions,
};

use crate::domain::models::sampling::{SamplingParams, SamplingPreset};

use super::{RoundDefinition, ScenarioDefinition};

pub fn deep_research() -> ScenarioDefinition {
    ScenarioDefinition {
        name: "deep_research",
        sampling_preset: Some(SamplingPreset::THINKING),
        rounds: vec![
            RoundDefinition {
                prompt_template: "deep_research/search.md".into(),
                tool_names: vec!["search_knowledge_base".into()],
                max_llm_calls: 1,
                reasoning_effort: Some(ReasoningEffort::Medium),
                sampling: SamplingParams::default(),
                extra_body: None,
                tool_choice: Some(ChatCompletionToolChoiceOption::Mode(
                    ToolChoiceOptions::Required,
                )),
                enable_thinking: true,
                preserve_thinking: true,
                hidden: true,
            },
            RoundDefinition {
                prompt_template: "deep_research/explore.md".into(),
                tool_names: vec!["search_knowledge_base".into(), "extract_document".into()],
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
                tool_names: vec![],
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
