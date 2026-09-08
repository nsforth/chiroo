use async_openai::types::chat::{ChatCompletionToolChoiceOption, ReasoningEffort};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

use crate::config::{load_static_asset, StaticAsset};
use crate::domain::models::messages::{Message, MessageId};
use crate::domain::models::sampling::SamplingParams;
use crate::domain::models::tool::LLMTool;
use crate::infra::clients::llm_client::RunRoundParams;
use crate::services::knowledge_service::KnowledgeService;
use crate::services::search_service::SearchService;
use crate::services::tools::knowledge_base::{ExtractDocumentTool, SearchKnowledgeBaseTool};

#[derive(Clone)]
pub struct RoundDefinition {
    pub prompt_template: String,
    pub tool_names: Vec<String>,
    pub max_llm_calls: usize,
    pub reasoning_effort: Option<ReasoningEffort>,
    pub sampling: SamplingParams,
    pub extra_body: Option<Value>,
    pub tool_choice: Option<ChatCompletionToolChoiceOption>,
    pub enable_thinking: bool,
    pub preserve_thinking: bool,
    /// Intermediate rounds are hidden from non-admin users; only the final
    /// answer round is shown. Admins still see every round.
    pub hidden: bool,
}

impl RoundDefinition {
    /// Build the LLM round params for this round: own behavior fields + run context.
    pub fn to_run_params(
        &self,
        system_prompt: String,
        history: Vec<Message>,
        tools: Vec<Box<dyn LLMTool>>,
        scenario_sampling: SamplingParams,
        round_index: usize,
        assistant_message_id: MessageId,
    ) -> RunRoundParams {
        RunRoundParams {
            system_prompt,
            history,
            tools,
            max_llm_calls: self.max_llm_calls,
            reasoning_effort: self.reasoning_effort.clone(),
            sampling: scenario_sampling.merge(&self.sampling),
            extra_body: self.extra_body.clone(),
            tool_choice: self.tool_choice.clone(),
            preserve_thinking: self.enable_thinking && self.preserve_thinking,
            hidden: self.hidden,
            message_id_override: (round_index == 0).then_some(assistant_message_id),
        }
    }
}

#[derive(Clone)]
pub struct ScenarioDefinition {
    pub name: &'static str,
    pub sampling_preset: Option<&'static str>,
    pub rounds: Vec<RoundDefinition>,
}

mod deep_research;
mod simple;

pub use deep_research::deep_research;
pub use simple::simple;

#[derive(Clone, Copy)]
pub struct ScenarioInfo {
    pub id: &'static str,
    pub icon: &'static str,
    pub icon_file: Option<&'static str>,
}

const SCENARIOS: &[ScenarioInfo] = &[
    ScenarioInfo {
        id: "simple",
        icon: "sparkles",
        icon_file: None,
    },
    ScenarioInfo {
        id: "deep_research",
        icon: "compass",
        icon_file: None,
    },
];

pub fn list_scenarios() -> &'static [ScenarioInfo] {
    SCENARIOS
}

pub fn load_scenario_icons(dir: Option<&Path>) -> HashMap<String, StaticAsset> {
    let Some(dir) = dir else {
        return HashMap::new();
    };

    SCENARIOS
        .iter()
        .filter_map(|s| Some((s.id, s.icon_file?)))
        .filter_map(|(id, file)| {
            load_static_asset(&dir.join(file)).map(|asset| (id.to_string(), asset))
        })
        .collect()
}

pub fn scenario_by_name(name: &str) -> Option<ScenarioDefinition> {
    match name {
        "simple" => Some(simple()),
        "deep_research" => Some(deep_research()),
        _ => None,
    }
}

pub fn create_tools(
    names: &[String],
    search_service: &SearchService,
    knowledge_service: &KnowledgeService,
    limit: usize,
) -> Vec<Box<dyn LLMTool>> {
    names
        .iter()
        .filter_map(|name| create_tool(name, search_service, knowledge_service, limit))
        .collect()
}

fn create_tool(
    name: &str,
    search_service: &SearchService,
    knowledge_service: &KnowledgeService,
    limit: usize,
) -> Option<Box<dyn LLMTool>> {
    match name {
        "search_knowledge_base" => Some(Box::new(SearchKnowledgeBaseTool::new(
            search_service.clone(),
            knowledge_service.clone(),
            limit,
        ))),
        "extract_document" => Some(Box::new(ExtractDocumentTool::new(
            knowledge_service.clone(),
        ))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deadpool_diesel::postgres::{Manager, Pool};
    use deadpool_diesel::Runtime;
    use secrecy::SecretString;

    use crate::domain::models::sampling::preset_by_name;

    use crate::config::{EmbeddingConfig, RerankConfig};
    use crate::infra::clients::embedding_client::EmbeddingClient;
    use crate::infra::clients::qdrant_client::QdrantClient;
    use crate::infra::clients::rerank_client::RerankClient;
    use crate::services::chunking_service::ChunkingService;

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
        assert!(r1.tool_names.contains(&"search_knowledge_base".into()));
        assert!(r1.reasoning_effort.is_none());
        assert_eq!(r1.max_llm_calls, 1);
        assert!(r1.sampling.is_empty());
        assert!(!r1.enable_thinking);
        assert!(!r1.preserve_thinking);
        assert!(r1.hidden);
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
        assert!(r2.tool_names.contains(&"extract_document".into()));
        assert_eq!(r2.max_llm_calls, 1);
        assert!(r2.hidden);
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
        assert!(!r3.hidden);
        assert_eq!(
            r3.tool_choice,
            Some(
                async_openai::types::chat::ChatCompletionToolChoiceOption::Mode(
                    async_openai::types::chat::ToolChoiceOptions::None
                )
            )
        );
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
        assert_eq!(r1.tool_names, vec!["search_knowledge_base"]);
        assert!(r1.reasoning_effort.is_some());
        assert_eq!(r1.max_llm_calls, 1);
        assert!(r1.enable_thinking);
        assert!(r1.preserve_thinking);
        assert!(r1.hidden);
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
        assert!(r2.tool_names.contains(&"search_knowledge_base".into()));
        assert!(r2.tool_names.contains(&"extract_document".into()));
        assert!(r2.tool_choice.is_some());
        assert!(r2.enable_thinking);
        assert!(r2.preserve_thinking);
        assert!(r2.hidden);
    }

    #[test]
    fn deep_research_round_3_is_answer_no_tools() {
        let scenario = deep_research();
        let r3 = &scenario.rounds[2];
        assert_eq!(r3.prompt_template, "deep_research/answer.md");
        assert!(r3.tool_names.is_empty());
        assert!(!r3.hidden);
        assert_eq!(
            r3.tool_choice,
            Some(
                async_openai::types::chat::ChatCompletionToolChoiceOption::Mode(
                    async_openai::types::chat::ToolChoiceOptions::None
                )
            )
        );
    }

    #[test]
    fn scenarios_reference_defined_sampling_presets() {
        for scenario in [simple(), deep_research()] {
            let Some(name) = scenario.sampling_preset else {
                panic!(
                    "scenario '{}' must reference a sampling preset",
                    scenario.name
                );
            };
            assert!(
                preset_by_name(name).is_some(),
                "scenario '{}' must reference a defined preset, found '{}'",
                scenario.name,
                name
            );
        }
    }

    #[test]
    fn scenario_rounds_inherit_scenario_sampling_by_default() {
        let simple = simple();
        assert!(simple.rounds.iter().all(|r| r.sampling.is_empty()));
        let deep = deep_research();
        assert!(deep.rounds.iter().all(|r| r.sampling.is_empty()));
    }

    #[test]
    fn scenario_by_name_resolves_known_ids() {
        assert_eq!(scenario_by_name("simple").unwrap().name, "simple");
        assert_eq!(
            scenario_by_name("deep_research").unwrap().name,
            "deep_research"
        );
    }

    #[test]
    fn scenario_by_name_unknown_returns_none() {
        assert!(scenario_by_name("Qwen3.6-27B").is_none());
        assert!(scenario_by_name("deep-research-v1").is_none());
        assert!(scenario_by_name("").is_none());
    }

    #[test]
    fn list_scenarios_returns_metadata() {
        let scenarios = list_scenarios();
        assert_eq!(scenarios.len(), 2);
        let simple = scenarios
            .iter()
            .find(|s| s.id == "simple")
            .expect("simple scenario must be listed");
        assert_eq!(simple.id, "simple");
        assert_eq!(simple.icon, "sparkles");
    }

    #[test]
    fn create_tools_returns_correct_instances() {
        let search = make_search_service();
        let knowledge = make_knowledge_service();
        let tools = create_tools(
            &["search_knowledge_base".into(), "extract_document".into()],
            &search,
            &knowledge,
            10,
        );
        assert_eq!(tools.len(), 2);
    }

    #[test]
    fn create_tools_skips_unknown() {
        let search = make_search_service();
        let knowledge = make_knowledge_service();
        let tools = create_tools(
            &["search_knowledge_base".into(), "nonexistent".into()],
            &search,
            &knowledge,
            10,
        );
        assert_eq!(tools.len(), 1);
    }

    #[test]
    fn create_tools_empty() {
        let search = make_search_service();
        let knowledge = make_knowledge_service();
        let tools = create_tools(&[], &search, &knowledge, 10);
        assert!(tools.is_empty());
    }

    fn make_pool() -> Pool {
        let manager = Manager::new(
            "postgres://chiroo:1234@localhost/chiroo_tests".to_string(),
            Runtime::Tokio1,
        );
        Pool::builder(manager)
            .build()
            .expect("failed to build pool")
    }

    fn make_embedding_client() -> EmbeddingClient {
        EmbeddingClient::new(&EmbeddingConfig {
            base_url: "http://localhost:11434/v1".into(),
            api_key: SecretString::new("not-needed".to_string().into_boxed_str()),
            model: "EMBEDDINGS".into(),
            dimensions: 1024,
        })
    }

    fn make_qdrant_client() -> QdrantClient {
        QdrantClient::new(
            "http://localhost:6334",
            None,
            "test".to_string(),
            std::time::Duration::from_secs(30),
        )
        .expect("failed to create Qdrant client")
    }

    fn make_knowledge_service() -> KnowledgeService {
        KnowledgeService::new(
            make_pool(),
            ChunkingService::new(512),
            make_qdrant_client(),
            make_embedding_client(),
            1024,
        )
    }

    fn make_search_service() -> SearchService {
        SearchService::new(
            make_embedding_client(),
            make_qdrant_client(),
            RerankClient::new(&RerankConfig {
                base_url: "http://localhost:11434/v1".into(),
                api_key: SecretString::new("".to_string().into_boxed_str()),
                model: "RERANKER".into(),
            })
            .expect("failed to create Rerank client"),
            0.5,
            30,
        )
    }
}
