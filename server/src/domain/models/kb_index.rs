use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::errors::AppError;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IndexType {
    Keyword,
    Integer,
    Float,
    Bool,
    Datetime,
    Uuid,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct KbIndex {
    pub field: String,
    #[serde(rename = "type")]
    pub r#type: IndexType,
    pub description: Option<String>,
}

#[derive(Deserialize)]
struct IndexSpec {
    #[serde(rename = "type")]
    r#type: String,
    #[serde(default)]
    description: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct KbIndexes {
    entries: Vec<KbIndex>,
}

impl KbIndexes {
    pub fn parse(value: &Value) -> Result<Self, AppError> {
        let map = value.as_object().ok_or_else(|| {
            AppError::BadRequest("invalid knowledge base indexes: must be a JSON object".into())
        })?;

        let mut entries = Vec::with_capacity(map.len());
        for (field, spec) in map {
            if field.is_empty() {
                return Err(AppError::BadRequest(format!(
                    "invalid knowledge base index \"{field}\": field name must not be empty"
                )));
            }
            if field.contains('.') {
                return Err(AppError::BadRequest(format!(
                    "invalid knowledge base index \"{field}\": field name must not contain '.'"
                )));
            }
            if !spec.is_object() {
                return Err(AppError::BadRequest(format!(
                    "invalid knowledge base index \"{field}\": spec must be an object"
                )));
            }
            let parsed = serde_json::from_value::<IndexSpec>(spec.clone()).map_err(|_| {
                AppError::BadRequest(format!(
                    "invalid knowledge base index \"{field}\": \"type\" must be a string"
                ))
            })?;
            let r#type = match parsed.r#type.as_str() {
                "keyword" => IndexType::Keyword,
                "integer" => IndexType::Integer,
                "float" => IndexType::Float,
                "bool" => IndexType::Bool,
                "datetime" => IndexType::Datetime,
                "uuid" => IndexType::Uuid,
                other => {
                    return Err(AppError::BadRequest(format!(
                        "invalid knowledge base index \"{field}\": unknown type \"{other}\" (expected one of: keyword, integer, float, bool, datetime, uuid)"
                    )))
                }
            };
            entries.push(KbIndex {
                field: field.clone(),
                r#type,
                description: parsed.description,
            });
        }
        entries.sort_by(|a, b| a.field.cmp(&b.field));

        Ok(Self { entries })
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn entries(&self) -> &[KbIndex] {
        &self.entries
    }

    pub fn find(&self, field: &str) -> Option<&KbIndex> {
        self.entries.iter().find(|e| e.field == field)
    }

    pub fn to_value(&self) -> Option<Value> {
        if self.is_empty() {
            return None;
        }
        let mut map = serde_json::Map::with_capacity(self.entries.len());
        for entry in &self.entries {
            let mut spec = serde_json::Map::new();
            spec.insert(
                "type".into(),
                serde_json::to_value(entry.r#type).expect("IndexType serializes to a string"),
            );
            if let Some(description) = &entry.description {
                spec.insert("description".into(), Value::String(description.clone()));
            }
            map.insert(entry.field.clone(), Value::Object(spec));
        }
        Some(Value::Object(map))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn bad_request_message(err: AppError) -> String {
        match err {
            AppError::BadRequest(msg) => msg,
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    #[test]
    fn parse_accepts_valid_indexes() {
        let value = json!({
            "zeta": { "type": "keyword" },
            "alpha": { "type": "integer", "description": "an integer" },
            "mid": { "type": "uuid" },
            "beta": { "type": "float" },
            "flag": { "type": "bool", "description": "a flag" },
            "when": { "type": "datetime" },
        });
        let indexes = KbIndexes::parse(&value).expect("valid indexes must parse");

        assert_eq!(indexes.len(), 6);
        assert!(
            indexes.entries()
                == [
                    KbIndex {
                        field: "alpha".into(),
                        r#type: IndexType::Integer,
                        description: Some("an integer".into())
                    },
                    KbIndex {
                        field: "beta".into(),
                        r#type: IndexType::Float,
                        description: None
                    },
                    KbIndex {
                        field: "flag".into(),
                        r#type: IndexType::Bool,
                        description: Some("a flag".into())
                    },
                    KbIndex {
                        field: "mid".into(),
                        r#type: IndexType::Uuid,
                        description: None
                    },
                    KbIndex {
                        field: "when".into(),
                        r#type: IndexType::Datetime,
                        description: None
                    },
                    KbIndex {
                        field: "zeta".into(),
                        r#type: IndexType::Keyword,
                        description: None
                    },
                ]
        );
        assert_eq!(
            indexes.find("alpha"),
            Some(&KbIndex {
                field: "alpha".into(),
                r#type: IndexType::Integer,
                description: Some("an integer".into()),
            })
        );
        assert_eq!(indexes.find("missing"), None);
    }

    #[test]
    fn parse_rejects_non_object() {
        assert!(KbIndexes::parse(&Value::Null).is_err());
        assert!(KbIndexes::parse(&Value::String("keyword".into())).is_err());
    }

    #[test]
    fn parse_rejects_unknown_type() {
        let value = json!({ "x": { "type": "foo" } });
        let err = KbIndexes::parse(&value).expect_err("unknown type must be rejected");
        let msg = bad_request_message(err);
        assert!(msg.contains("\"x\""), "error must name the field: {msg}");
        assert!(msg.contains("unknown type"), "{msg}");
    }

    #[test]
    fn parse_rejects_missing_type() {
        assert!(KbIndexes::parse(&json!({ "x": { "description": "no type" } })).is_err());
    }

    #[test]
    fn parse_rejects_dotted_field() {
        let err = KbIndexes::parse(&json!({ "a.b": { "type": "keyword" } }))
            .expect_err("dotted field must be rejected");
        let msg = bad_request_message(err);
        assert!(msg.contains("\"a.b\""), "error must name the field: {msg}");
    }

    #[test]
    fn parse_rejects_spec_that_is_not_object() {
        assert!(KbIndexes::parse(&json!({ "foo": "keyword" })).is_err());
    }

    #[test]
    fn to_value_empty_is_none() {
        assert_eq!(KbIndexes::default().to_value(), None);
    }

    #[test]
    fn to_value_with_description() {
        let indexes = KbIndexes::parse(&json!({ "a": { "type": "keyword", "description": "d" } }))
            .expect("valid indexes must parse");
        assert_eq!(
            indexes.to_value(),
            Some(json!({ "a": { "type": "keyword", "description": "d" } }))
        );
    }

    #[test]
    fn to_value_omits_missing_description() {
        let indexes = KbIndexes::parse(&json!({ "a": { "type": "keyword" } }))
            .expect("valid indexes must parse");
        let value = indexes.to_value().expect("non-empty must produce a value");
        assert_eq!(value["a"]["type"], "keyword");
        assert!(value["a"].get("description").is_none());
    }

    #[test]
    fn to_value_preserves_sorted_order_and_keys_by_field() {
        let indexes = KbIndexes::parse(&json!({
            "zeta": { "type": "uuid" },
            "alpha": { "type": "integer", "description": "an integer" },
            "mid": { "type": "bool" },
        }))
        .expect("valid indexes must parse");
        assert_eq!(
            indexes.to_value(),
            Some(json!({
                "alpha": { "type": "integer", "description": "an integer" },
                "mid": { "type": "bool" },
                "zeta": { "type": "uuid" },
            }))
        );
    }
}
