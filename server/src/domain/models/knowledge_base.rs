use chrono::NaiveDateTime;
use serde::Deserialize;
use serde_json::Value;
use tera::Context;
use uuid::Uuid;

use crate::domain::models::kb_index::KbIndexes;
use crate::errors::AppError;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Hash)]
pub struct KnowledgeBaseId(pub Uuid);

#[derive(Debug)]
pub struct CreateKnowledgeBase {
    pub name: String,
    pub description: String,
    pub public: bool,
    pub metadata: Value,
    pub language: Option<String>,
    pub indexes: Option<Value>,
}

#[derive(Clone, Debug)]
pub struct KnowledgeBase {
    pub id: KnowledgeBaseId,
    pub name: String,
    pub description: String,
    pub public: bool,
    pub metadata: Value,
    pub language: Option<String>,
    pub indexes: KbIndexes,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl KnowledgeBase {
    pub fn render_description(&self) -> Result<String, AppError> {
        tera::Tera::one_off(
            &self.description,
            &Context::from_value(self.metadata.clone()).unwrap_or_default(),
            false,
        )
        .map_err(|_| AppError::InternalServerError)
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDateTime;
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    fn kb(description: &str, metadata: Value) -> KnowledgeBase {
        KnowledgeBase {
            id: KnowledgeBaseId(Uuid::new_v4()),
            name: "Test KB".into(),
            description: description.into(),
            public: false,
            metadata,
            language: None,
            indexes: KbIndexes::default(),
            created_at: NaiveDateTime::MIN,
            updated_at: NaiveDateTime::MIN,
        }
    }

    fn rendered(kb: &KnowledgeBase) -> String {
        kb.render_description().expect("description must render")
    }

    #[test]
    fn render_plain_description_is_unchanged() {
        let kb = kb("Internal docs", json!({}));
        assert_eq!(rendered(&kb), "Internal docs");
    }

    #[test]
    fn render_substitutes_metadata_variables() {
        let kb = kb("Docs for the {{ team }} team", json!({ "team": "ml" }));
        assert_eq!(rendered(&kb), "Docs for the ml team");
    }

    #[test]
    fn render_joins_metadata_list() {
        let kb = kb(
            "Confluence spaces: {{ spaces|join(sep=\", \") }}",
            json!({ "spaces": ["DEV", "OPS", "HR"] }),
        );
        assert_eq!(rendered(&kb), "Confluence spaces: DEV, OPS, HR");
    }

    #[test]
    fn render_loops_over_metadata_list() {
        let kb = kb(
            "{% for s in spaces %}{{ s }}{% if not loop.last %}, {% endif %}{% endfor %}",
            json!({ "spaces": ["A", "B"] }),
        );
        assert_eq!(rendered(&kb), "A, B");
    }

    #[test]
    fn render_unknown_variable_is_error() {
        let kb = kb("Hello {{ missing }}", json!({}));
        assert!(kb.render_description().is_err());
    }

    #[test]
    fn render_non_object_metadata_with_plain_description_is_unchanged() {
        let kb = kb("Plain text", json!(["a", "b"]));
        assert_eq!(rendered(&kb), "Plain text");
    }

    #[test]
    fn render_non_object_metadata_with_variables_is_error() {
        let kb = kb("Hello {{ a }}", json!(["a"]));
        assert!(kb.render_description().is_err());
    }
}
