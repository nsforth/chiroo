use std::sync::Arc;

use tera::{Context, Tera};

#[derive(Clone)]
pub struct TemplateContext {
    pub assistant_name: String,
    pub user_name: String,
    pub current_date: String,
    pub question: String,
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
        self.tera.render(template, &ctx)
    }

    pub fn render_global_system(&self, context: &TemplateContext) -> Result<String, tera::Error> {
        self.render("global_system.md", context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context() -> TemplateContext {
        TemplateContext {
            assistant_name: "Chiroo".into(),
            user_name: "Alice".into(),
            current_date: "2026-07-27".into(),
            question: "What is Rust?".into(),
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
    fn renders_simple_search() {
        let result = renderer()
            .render("simple/search.md", &test_context())
            .expect("Failed to render");
        assert!(result.contains("search_knowledge_base"));
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
        assert!(result.contains("search_knowledge_base"));
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
