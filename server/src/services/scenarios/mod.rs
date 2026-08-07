use async_openai::types::chat::{ChatCompletionToolChoiceOption, ReasoningEffort};
use serde_json::Value;

use crate::domain::models::tool::LLMTool;
use crate::services::tools::rag::{ExtractPageTool, SearchRagTool};

#[derive(Clone)]
pub struct RoundConfig {
    pub prompt_template: String,
    pub tool_names: Vec<String>,
    pub max_llm_calls: usize,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub extra_body: Option<Value>,
    pub tool_choice: Option<ChatCompletionToolChoiceOption>,
    pub enable_thinking: bool,
    pub preserve_thinking: bool,
}

#[derive(Clone)]
pub struct Scenario {
    pub name: &'static str,
    pub rounds: Vec<RoundConfig>,
}

mod deep_research;
mod simple;

pub use deep_research::deep_research;
pub use simple::simple;

pub fn scenario_for_model(model: &str) -> Scenario {
    let model_lower = model.to_lowercase();
    if model_lower.contains("deep") || model_lower.contains("research") {
        deep_research()
    } else {
        simple()
    }
}

pub fn create_tools(names: &[String]) -> Vec<Box<dyn LLMTool>> {
    names.iter().filter_map(|name| create_tool(name)).collect()
}

fn create_tool(name: &str) -> Option<Box<dyn LLMTool>> {
    match name {
        "search_rag" => Some(Box::new(SearchRagTool)),
        "extract_page" => Some(Box::new(ExtractPageTool)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_scenario_has_three_rounds() {
        let scenario = simple();
        assert_eq!(scenario.name, "simple");
        assert_eq!(scenario.rounds.len(), 3);
    }

    #[test]
    fn simple_round_1_is_search() {
        let scenario = simple();
        let r1 = &scenario.rounds[0];
        assert_eq!(r1.prompt_template, "simple/search.md");
        assert!(r1.tool_names.contains(&"search_rag".into()));
        assert!(r1.reasoning_effort.is_none());
        assert_eq!(r1.max_llm_calls, 1);
        assert!(!r1.enable_thinking);
        assert!(!r1.preserve_thinking);
        assert_eq!(
            r1.tool_choice,
            Some(
                async_openai::types::chat::ChatCompletionToolChoiceOption::Mode(
                    async_openai::types::chat::ToolChoiceOptions::Required
                )
            )
        );
    }

    #[test]
    fn simple_round_2_is_extract() {
        let scenario = simple();
        let r2 = &scenario.rounds[1];
        assert_eq!(r2.prompt_template, "simple/extract.md");
        assert!(r2.tool_names.contains(&"extract_page".into()));
        assert_eq!(r2.max_llm_calls, 1);
        assert_eq!(
            r2.tool_choice,
            Some(
                async_openai::types::chat::ChatCompletionToolChoiceOption::Mode(
                    async_openai::types::chat::ToolChoiceOptions::Required
                )
            )
        );
    }

    #[test]
    fn simple_round_3_is_answer_no_tools() {
        let scenario = simple();
        let r3 = &scenario.rounds[2];
        assert_eq!(r3.prompt_template, "simple/answer.md");
        assert!(r3.tool_names.is_empty());
        assert!(r3.tool_choice.is_none());
    }

    #[test]
    fn deep_research_scenario_has_three_rounds() {
        let scenario = deep_research();
        assert_eq!(scenario.name, "deep_research");
        assert_eq!(scenario.rounds.len(), 3);
    }

    #[test]
    fn deep_research_round_1_is_mandatory_search() {
        let scenario = deep_research();
        let r1 = &scenario.rounds[0];
        assert_eq!(r1.prompt_template, "deep_research/search.md");
        assert_eq!(r1.tool_names, vec!["search_rag"]);
        assert!(r1.reasoning_effort.is_some());
        assert_eq!(r1.max_llm_calls, 1);
        assert!(r1.enable_thinking);
        assert!(r1.preserve_thinking);
        assert_eq!(
            r1.tool_choice,
            Some(
                async_openai::types::chat::ChatCompletionToolChoiceOption::Mode(
                    async_openai::types::chat::ToolChoiceOptions::Required
                )
            )
        );
    }

    #[test]
    fn deep_research_round_2_has_both_tools() {
        let scenario = deep_research();
        let r2 = &scenario.rounds[1];
        assert_eq!(r2.prompt_template, "deep_research/explore.md");
        assert!(r2.tool_names.contains(&"search_rag".into()));
        assert!(r2.tool_names.contains(&"extract_page".into()));
        assert!(r2.tool_choice.is_none());
        assert!(r2.enable_thinking);
        assert!(r2.preserve_thinking);
    }

    #[test]
    fn deep_research_round_3_is_answer_no_tools() {
        let scenario = deep_research();
        let r3 = &scenario.rounds[2];
        assert_eq!(r3.prompt_template, "deep_research/answer.md");
        assert!(r3.tool_names.is_empty());
        assert!(r3.tool_choice.is_none());
    }

    #[test]
    fn scenario_for_model_defaults_to_simple() {
        assert_eq!(scenario_for_model("Qwen3.6-27B").name, "simple");
        assert_eq!(scenario_for_model("gpt-4").name, "simple");
    }

    #[test]
    fn scenario_for_model_deep_research() {
        assert_eq!(scenario_for_model("deep-research-v1").name, "deep_research");
        assert_eq!(scenario_for_model("Qwen-Research").name, "deep_research");
    }

    #[test]
    fn create_tools_returns_correct_instances() {
        let tools = create_tools(&["search_rag".into(), "extract_page".into()]);
        assert_eq!(tools.len(), 2);
    }

    #[test]
    fn create_tools_skips_unknown() {
        let tools = create_tools(&["search_rag".into(), "nonexistent".into()]);
        assert_eq!(tools.len(), 1);
    }

    #[test]
    fn create_tools_empty() {
        let tools = create_tools(&[]);
        assert!(tools.is_empty());
    }
}
