use async_openai::types::chat::{ChatCompletionToolChoiceOption, ReasoningEffort};
use serde_json::Value;

use crate::domain::models::tool::LLMTool;
use crate::services::knowledge_service::KnowledgeService;
use crate::services::search_service::SearchService;
use crate::services::tools::knowledge_base::{ExtractDocumentTool, SearchKnowledgeBaseTool};

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

#[derive(Clone, Copy)]
pub struct LocalizedText {
    pub en: &'static str,
    pub ru: &'static str,
}

impl LocalizedText {
    pub fn get(&self, lang: &str) -> &'static str {
        match lang {
            "ru" => self.ru,
            _ => self.en,
        }
    }
}

#[derive(Clone, Copy)]
pub struct ScenarioInfo {
    pub id: &'static str,
    pub icon: &'static str,
    pub label: LocalizedText,
    pub description: LocalizedText,
}

const SCENARIOS: &[ScenarioInfo] = &[
    ScenarioInfo {
        id: "simple",
        icon: "sparkles",
        label: LocalizedText {
            en: "Simple",
            ru: "Простой",
        },
        description: LocalizedText {
            en: "A single search, one extraction, then a final answer.",
            ru: "Один поиск, одно извлечение, затем итоговый ответ.",
        },
    },
    ScenarioInfo {
        id: "deep_research",
        icon: "compass",
        label: LocalizedText {
            en: "Deep research",
            ru: "Глубокое исследование",
        },
        description: LocalizedText {
            en: "Iterative search and extraction for a thorough answer.",
            ru: "Итеративный поиск и извлечение для подробного ответа.",
        },
    },
];

pub fn list_scenarios() -> &'static [ScenarioInfo] {
    SCENARIOS
}

pub fn scenario_by_name(name: &str) -> Option<Scenario> {
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
) -> Vec<Box<dyn LLMTool>> {
    names
        .iter()
        .filter_map(|name| create_tool(name, search_service, knowledge_service))
        .collect()
}

fn create_tool(
    name: &str,
    search_service: &SearchService,
    knowledge_service: &KnowledgeService,
) -> Option<Box<dyn LLMTool>> {
    match name {
        "search_knowledge_base" => Some(Box::new(SearchKnowledgeBaseTool::new(
            search_service.clone(),
            knowledge_service.clone(),
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

    use crate::config::{EmbeddingConfig, RerankConfig, SearchConfig};
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
        assert!(r2.tool_names.contains(&"extract_document".into()));
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
    fn list_scenarios_localizes_labels() {
        let scenarios = list_scenarios();
        assert_eq!(scenarios.len(), 2);
        let simple = scenarios
            .iter()
            .find(|s| s.id == "simple")
            .expect("simple scenario must be listed");
        assert_eq!(simple.label.get("en"), "Simple");
        assert_eq!(simple.label.get("ru"), "Простой");
        assert_eq!(
            simple.label.get("fr"),
            "Simple",
            "unknown lang falls back to en"
        );
    }

    #[test]
    fn create_tools_returns_correct_instances() {
        let search = make_search_service();
        let knowledge = make_knowledge_service();
        let tools = create_tools(
            &["search_knowledge_base".into(), "extract_document".into()],
            &search,
            &knowledge,
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
        );
        assert_eq!(tools.len(), 1);
    }

    #[test]
    fn create_tools_empty() {
        let search = make_search_service();
        let knowledge = make_knowledge_service();
        let tools = create_tools(&[], &search, &knowledge);
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
        QdrantClient::new("http://localhost:6334", None, "test".to_string())
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
            SearchConfig {
                limit: 10,
                score_threshold: 0.5,
                candidate_limit: 30,
            },
        )
    }
}
