use async_openai::types::chat::{ChatCompletionToolChoiceOption, ToolChoiceOptions};

use super::{RoundConfig, Scenario};

pub fn simple() -> Scenario {
    Scenario {
        name: "simple",
        rounds: vec![
            RoundConfig {
                prompt_template: "simple/search.md".into(),
                tool_names: vec!["search_knowledge_base".into()],
                max_llm_calls: 1,
                reasoning_effort: None,
                extra_body: None,
                tool_choice: Some(ChatCompletionToolChoiceOption::Mode(
                    ToolChoiceOptions::Required,
                )),
                enable_thinking: false,
                preserve_thinking: false,
            },
            RoundConfig {
                prompt_template: "simple/extract.md".into(),
                tool_names: vec!["extract_document".into()],
                max_llm_calls: 1,
                reasoning_effort: None,
                extra_body: None,
                tool_choice: Some(ChatCompletionToolChoiceOption::Mode(
                    ToolChoiceOptions::Required,
                )),
                enable_thinking: false,
                preserve_thinking: false,
            },
            RoundConfig {
                prompt_template: "simple/answer.md".into(),
                tool_names: vec![],
                max_llm_calls: 1,
                reasoning_effort: None,
                extra_body: None,
                tool_choice: None,
                enable_thinking: false,
                preserve_thinking: false,
            },
        ],
    }
}
