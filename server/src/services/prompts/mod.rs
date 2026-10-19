use std::sync::Arc;

use serde::Serialize;
use tera::{Context, Tera};

use crate::domain::models::kb_index::KbIndex;
use crate::domain::models::knowledge_base::KnowledgeBase;

/// Knowledge base entry rendered into the system prompt.
#[derive(Clone, Debug, Serialize)]
pub struct KbContext {
    pub id: String,
    pub name: String,
    pub description: String,
    pub language: Option<String>,
    pub indexes: Vec<KbIndex>,
}

impl From<&KnowledgeBase> for KbContext {
    fn from(kb: &KnowledgeBase) -> Self {
        let description = kb
            .render_description()
            .unwrap_or_else(|_| kb.description.clone());
        Self {
            id: kb.id.0.to_string(),
            name: kb.name.clone(),
            description,
            language: kb.language.clone(),
            indexes: kb.indexes.entries().to_vec(),
        }
    }
}

#[derive(Clone)]
pub struct TemplateContext {
    pub assistant_name: String,
    pub user_name: String,
    pub current_date: String,
    pub question: String,
    pub knowledge_bases: Vec<KbContext>,
}

#[derive(Clone)]
pub struct PromptRenderer {
    tera: Arc<Tera>,
}

impl PromptRenderer {
    pub fn new(templates_dir: &str) -> Result<Self, tera::Error> {
        let pattern = format!("{templates_dir}/**/*.md");
        let tera = Arc::new(Tera::new(&pattern)?);
        Ok(Self { tera })
    }

    pub fn render(&self, template: &str, context: &TemplateContext) -> Result<String, tera::Error> {
        let mut ctx = Context::new();
        ctx.insert("assistant_name", &context.assistant_name);
        ctx.insert("user_name", &context.user_name);
        ctx.insert("current_date", &context.current_date);
        ctx.insert("question", &context.question);
        ctx.insert("knowledge_bases", &context.knowledge_bases);
        self.tera.render(template, &ctx)
    }

    pub fn render_global_system(&self, context: &TemplateContext) -> Result<String, tera::Error> {
        self.render("global_system.md", context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::kb_index::IndexType;

    fn test_kb() -> KbContext {
        KbContext {
            id: "11111111-2222-3333-4444-555555555555".into(),
            name: "Company Docs".into(),
            description: "Internal company documentation and policies".into(),
            language: Some("english".into()),
            indexes: vec![
                KbIndex {
                    field: "author".into(),
                    r#type: IndexType::Keyword,
                    description: Some("Who wrote the document".into()),
                },
                KbIndex {
                    field: "updated_at".into(),
                    r#type: IndexType::Datetime,
                    description: None,
                },
            ],
        }
    }

    fn test_context() -> TemplateContext {
        TemplateContext {
            assistant_name: "Chiroo".into(),
            user_name: "Alice".into(),
            current_date: "2026-07-27".into(),
            question: "What is Rust?".into(),
            knowledge_bases: vec![test_kb()],
        }
    }

    fn renderer() -> PromptRenderer {
        PromptRenderer::new("prompts").expect("Failed to load templates")
    }

    #[test]
    fn renders_global_system_with_all_variables() {
        let result = renderer()
            .render_global_system(&test_context())
            .expect("Failed to render");
        assert!(result.contains("Chiroo"));
        assert!(result.contains("2026-07-27"));
        assert!(result.contains("Alice"));
    }

    #[test]
    fn renders_global_system_with_empty_user_name() {
        let ctx = TemplateContext {
            user_name: String::new(),
            ..test_context()
        };
        let result = renderer()
            .render_global_system(&ctx)
            .expect("Failed to render");
        assert!(!result.contains("Alice"));
    }

    #[test]
    fn renders_global_system_with_knowledge_bases() {
        let result = renderer()
            .render_global_system(&test_context())
            .expect("Failed to render");
        assert!(result.contains("Company Docs"));
        assert!(result.contains("11111111-2222-3333-4444-555555555555"));
        assert!(result.contains("Internal company documentation and policies"));
        assert!(result.contains("english"));
    }

    #[test]
    fn renders_global_system_with_multiple_knowledge_bases() {
        let mut ctx = test_context();
        ctx.knowledge_bases.push(KbContext {
            id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into(),
            name: "Support Articles".into(),
            description: "Customer support knowledge base".into(),
            language: None,
            indexes: vec![],
        });
        let result = renderer()
            .render_global_system(&ctx)
            .expect("Failed to render");
        assert!(result.contains("Company Docs"));
        assert!(result.contains("Support Articles"));
        // Only one KB declares a language.
        assert_eq!(result.matches("Primary language:").count(), 1);
    }

    #[test]
    fn renders_global_system_with_no_knowledge_bases() {
        let ctx = TemplateContext {
            knowledge_bases: vec![],
            ..test_context()
        };
        let result = renderer()
            .render_global_system(&ctx)
            .expect("Failed to render");
        assert!(result.contains("No knowledge bases are configured"));
        assert!(!result.contains("Company Docs"));
        assert!(!result.contains("Fields available for `search_in_all_kbs` filters"));
    }

    #[test]
    fn renders_kb_filterable_indexes() {
        let result = renderer()
            .render_global_system(&test_context())
            .expect("Failed to render");
        assert!(result.contains("Filterable metadata fields:"));
        assert!(result.contains("- author (keyword): Who wrote the document"));
        assert!(result.contains("- updated_at (datetime)"));
    }

    #[test]
    fn omits_filterable_indexes_block_when_kb_has_none() {
        let ctx = TemplateContext {
            knowledge_bases: vec![KbContext {
                indexes: vec![],
                ..test_kb()
            }],
            ..test_context()
        };
        let result = renderer()
            .render_global_system(&ctx)
            .expect("Failed to render");
        assert!(!result.contains("Filterable metadata fields:"));
        assert!(!result.contains("- author (keyword)"));
    }

    #[test]
    fn renders_new_search_tools() {
        let result = renderer()
            .render_global_system(&test_context())
            .expect("Failed to render");
        assert!(result.contains("search_in_all_kbs` — searches all knowledge bases"));
        assert!(result.contains("search_in_kb` — searches ONE knowledge base"));
    }

    #[test]
    fn renders_simple_search() {
        let result = renderer()
            .render("simple/search.md", &test_context())
            .expect("Failed to render");
        assert!(result.contains("search_in_all_kbs"));
        assert!(result.contains("What is Rust?"));
    }

    #[test]
    fn renders_simple_extract() {
        let result = renderer()
            .render("simple/extract.md", &test_context())
            .expect("Failed to render");
        assert!(result.contains("extract_document"));
    }

    #[test]
    fn renders_simple_answer() {
        let result = renderer()
            .render("simple/answer.md", &test_context())
            .expect("Failed to render");
        assert!(result.contains("comprehensive"));
    }

    #[test]
    fn renders_deep_search() {
        let result = renderer()
            .render("deep_research/search.md", &test_context())
            .expect("Failed to render");
        assert!(result.contains("search_in_all_kbs"));
    }

    #[test]
    fn renders_deep_explore() {
        let result = renderer()
            .render("deep_research/explore.md", &test_context())
            .expect("Failed to render");
        assert!(result.contains("deep research"));
    }

    #[test]
    fn renders_deep_answer() {
        let result = renderer()
            .render("deep_research/answer.md", &test_context())
            .expect("Failed to render");
        assert!(result.contains("synthesize"));
    }

    #[test]
    fn unknown_template_returns_error() {
        let result = renderer().render("nonexistent.md", &test_context());
        assert!(result.is_err());
    }
}
