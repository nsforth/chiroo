use serde_json::Value;
use uuid::Uuid;

use crate::domain::models::kb_index::{IndexType, KbIndexes};
use crate::errors::AppError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterOp {
    Eq,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchFilterCondition {
    pub field: String,
    pub op: FilterOp,
    pub value: Value,
    pub index_type: Option<IndexType>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SearchFilter {
    conditions: Vec<SearchFilterCondition>,
}

impl SearchFilter {
    pub fn from_json_value(value: &Value) -> Result<Self, AppError> {
        let map = value
            .as_object()
            .ok_or_else(|| AppError::BadRequest("invalid filters: must be a JSON object".into()))?;

        let mut conditions = Vec::new();
        for (field, v) in map {
            match v {
                Value::Object(ops) => {
                    if ops.is_empty() {
                        return Err(AppError::BadRequest(format!(
                            "invalid filter \"{}\": no operators provided (expected one of: eq, gt, gte, lt, lte)",
                            field
                        )));
                    }
                    for op in ops.keys() {
                        if !matches!(op.as_str(), "eq" | "gt" | "gte" | "lt" | "lte") {
                            return Err(AppError::BadRequest(format!(
                                "invalid filter \"{}\": unknown operator \"{}\" (expected one of: eq, gt, gte, lt, lte)",
                                field, op
                            )));
                        }
                    }
                    for (op, key) in [
                        (FilterOp::Eq, "eq"),
                        (FilterOp::Gt, "gt"),
                        (FilterOp::Gte, "gte"),
                        (FilterOp::Lt, "lt"),
                        (FilterOp::Lte, "lte"),
                    ] {
                        if let Some(v) = ops.get(key) {
                            push_condition(&mut conditions, field, op, v)?;
                        }
                    }
                },
                v if is_scalar(v) => push_condition(&mut conditions, field, FilterOp::Eq, v)?,
                _ => {
                    return Err(AppError::BadRequest(format!(
                        "invalid filter \"{}\": value must be a string, number or boolean",
                        field
                    )))
                },
            }
        }

        Ok(Self { conditions })
    }

    pub fn is_empty(&self) -> bool {
        self.conditions.is_empty()
    }

    pub fn conditions(&self) -> &[SearchFilterCondition] {
        &self.conditions
    }

    pub fn validate(&mut self, allowed: &KbIndexes) -> Result<(), AppError> {
        for condition in &mut self.conditions {
            let field = &condition.field;
            let entry = allowed.find(field).ok_or_else(|| {
                if allowed.is_empty() {
                    AppError::BadRequest(format!(
                        "filter field \"{}\" is not indexable (this knowledge base has no filterable fields)",
                        field
                    ))
                } else {
                    let available = allowed
                        .entries()
                        .iter()
                        .map(|e| e.field.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    AppError::BadRequest(format!(
                        "filter field \"{}\" is not indexable (available: {})",
                        field, available
                    ))
                }
            })?;
            let t = entry.r#type;

            if condition.op != FilterOp::Eq
                && !matches!(
                    t,
                    IndexType::Integer | IndexType::Float | IndexType::Datetime
                )
            {
                return Err(AppError::BadRequest(format!(
                    "filter field \"{}\": operator {:?} is not supported for type {:?} (only \"eq\" is supported)",
                    field, condition.op, t
                )));
            }

            match t {
                IndexType::Keyword => {
                    if condition.value.as_str().is_none() {
                        return Err(AppError::BadRequest(format!(
                            "filter field \"{}\": value must be a string",
                            field
                        )));
                    }
                },
                IndexType::Uuid => {
                    let normalized = condition
                        .value
                        .as_str()
                        .and_then(|s| Uuid::parse_str(s).ok())
                        .map(|uuid| uuid.to_string())
                        .ok_or_else(|| {
                            AppError::BadRequest(format!(
                                "filter field \"{}\": value is not a valid UUID",
                                field
                            ))
                        })?;
                    condition.value = Value::String(normalized);
                },
                IndexType::Bool => {
                    if condition.value.as_bool().is_none() {
                        return Err(AppError::BadRequest(format!(
                            "filter field \"{}\": value must be a boolean",
                            field
                        )));
                    }
                },
                IndexType::Integer => {
                    if condition.value.as_i64().is_none() {
                        return Err(AppError::BadRequest(format!(
                            "filter field \"{}\": value must be an integer",
                            field
                        )));
                    }
                },
                IndexType::Float => {
                    if condition.value.as_f64().is_none() {
                        return Err(AppError::BadRequest(format!(
                            "filter field \"{}\": value must be a number",
                            field
                        )));
                    }
                },
                IndexType::Datetime => {
                    let ok = condition
                        .value
                        .as_str()
                        .is_some_and(|s| chrono::DateTime::parse_from_rfc3339(s).is_ok());
                    if !ok {
                        return Err(AppError::BadRequest(format!(
                            "filter field \"{}\": value is not a valid RFC 3339 datetime",
                            field
                        )));
                    }
                },
            }
        }

        Ok(())
    }

    pub fn scope(&self, kb: &KbIndexes) -> Self {
        let conditions = self
            .conditions
            .iter()
            .filter_map(|condition| {
                kb.find(&condition.field)
                    .map(|entry| SearchFilterCondition {
                        field: condition.field.clone(),
                        op: condition.op,
                        value: condition.value.clone(),
                        index_type: Some(entry.r#type),
                    })
            })
            .collect();
        Self { conditions }
    }
}

fn is_scalar(value: &Value) -> bool {
    matches!(value, Value::String(_) | Value::Number(_) | Value::Bool(_))
}

fn push_condition(
    conditions: &mut Vec<SearchFilterCondition>,
    field: &str,
    op: FilterOp,
    value: &Value,
) -> Result<(), AppError> {
    if !is_scalar(value) {
        return Err(AppError::BadRequest(format!(
            "invalid filter \"{}\": value must be a string, number or boolean",
            field
        )));
    }
    conditions.push(SearchFilterCondition {
        field: field.to_string(),
        op,
        value: value.clone(),
        index_type: None,
    });
    Ok(())
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

    fn indexes(value: &Value) -> KbIndexes {
        KbIndexes::parse(value).expect("valid indexes must parse")
    }

    #[test]
    fn from_llm_value_scalar_is_eq() {
        let filter = SearchFilter::from_json_value(&json!({ "reporter": "alice" }))
            .expect("scalar must parse");
        assert_eq!(
            filter.conditions(),
            &[SearchFilterCondition {
                field: "reporter".into(),
                op: FilterOp::Eq,
                value: json!("alice"),
                index_type: None,
            }]
        );
    }

    #[test]
    fn from_llm_value_object_operators() {
        let filter = SearchFilter::from_json_value(&json!({
            "updated_at": {
                "gte": "2026-01-01T00:00:00Z",
                "lte": "2026-06-30T00:00:00Z",
            }
        }))
        .expect("operator object must parse");
        assert_eq!(filter.conditions().len(), 2);
        assert_eq!(filter.conditions()[0].op, FilterOp::Gte);
        assert_eq!(filter.conditions()[1].op, FilterOp::Lte);
        assert_eq!(filter.conditions()[0].value, json!("2026-01-01T00:00:00Z"));
        assert_eq!(filter.conditions()[1].value, json!("2026-06-30T00:00:00Z"));
        assert!(filter.conditions().iter().all(|c| c.index_type.is_none()));
    }

    #[test]
    fn from_llm_value_unknown_operator_rejected() {
        let err = SearchFilter::from_json_value(&json!({ "x": { "neq": 1 } }))
            .expect_err("unknown operator must be rejected");
        let msg = bad_request_message(err);
        assert!(msg.contains("\"x\""), "error must name the field: {msg}");
        assert!(msg.contains("neq"), "error must name the operator: {msg}");
    }

    #[test]
    fn from_llm_value_empty_operators_object_rejected() {
        let err = SearchFilter::from_json_value(&json!({ "reporter": {} }))
            .expect_err("empty operators object must be rejected");
        let msg = bad_request_message(err);
        assert!(msg.contains("reporter"), "error must name the field: {msg}");

        let err = SearchFilter::from_json_value(&json!({ "a": { "eq": "x" }, "b": {} }))
            .expect_err("empty operators object alongside valid one must be rejected");
        let msg = bad_request_message(err);
        assert!(msg.contains("b"), "error must name the field: {msg}");
    }

    #[test]
    fn from_llm_value_non_object_rejected() {
        assert!(SearchFilter::from_json_value(&json!(["x"])).is_err());
        assert!(SearchFilter::from_json_value(&json!("x")).is_err());
        assert!(SearchFilter::from_json_value(&json!(1)).is_err());
    }

    #[test]
    fn from_llm_value_null_value_rejected() {
        let err = SearchFilter::from_json_value(&json!({ "x": null }))
            .expect_err("null value must be rejected");
        let msg = bad_request_message(err);
        assert!(msg.contains("\"x\""), "error must name the field: {msg}");

        let err = SearchFilter::from_json_value(&json!({ "x": { "eq": [1] } }))
            .expect_err("array value must be rejected");
        let msg = bad_request_message(err);
        assert!(msg.contains("\"x\""), "error must name the field: {msg}");
    }

    #[test]
    fn from_llm_value_empty_object_is_empty() {
        let filter = SearchFilter::from_json_value(&json!({})).expect("empty object must parse");
        assert!(filter.is_empty());
    }

    #[test]
    fn validate_unknown_field_rejected() {
        let allowed = indexes(&json!({ "topic": { "type": "keyword" } }));
        let mut filter = SearchFilter::from_json_value(&json!({ "missing": "x" })).expect("parse");
        let err = filter
            .validate(&allowed)
            .expect_err("unknown field must be rejected");
        let msg = bad_request_message(err);
        assert!(
            msg.contains("\"missing\""),
            "error must name the field: {msg}"
        );
        assert!(
            msg.contains("topic"),
            "error must list available fields: {msg}"
        );
    }

    #[test]
    fn validate_no_fields_in_kb_rejected() {
        let allowed = indexes(&json!({}));
        let mut filter = SearchFilter::from_json_value(&json!({ "x": 1 })).expect("parse");
        let err = filter
            .validate(&allowed)
            .expect_err("field on empty KB must be rejected");
        let msg = bad_request_message(err);
        assert!(msg.contains("\"x\""), "error must name the field: {msg}");
        assert!(
            msg.contains("no filterable fields"),
            "error must explain the KB has no fields: {msg}"
        );
    }

    #[test]
    fn validate_range_on_keyword_rejected() {
        let allowed = indexes(&json!({ "topic": { "type": "keyword" } }));
        let mut filter =
            SearchFilter::from_json_value(&json!({ "topic": { "gt": "x" } })).expect("parse");
        let err = filter
            .validate(&allowed)
            .expect_err("range on keyword must be rejected");
        let msg = bad_request_message(err);
        assert!(
            msg.contains("\"topic\""),
            "error must name the field: {msg}"
        );
    }

    #[test]
    fn validate_uuid_format_checked() {
        let allowed = indexes(&json!({ "id": { "type": "uuid" } }));
        let mut bad = SearchFilter::from_json_value(&json!({ "id": "not-a-uuid" })).expect("parse");
        let msg = bad_request_message(
            bad.validate(&allowed)
                .expect_err("invalid uuid must be rejected"),
        );
        assert!(msg.contains("\"id\""), "error must name the field: {msg}");

        let mut good =
            SearchFilter::from_json_value(&json!({ "id": "0f37c00d-9f18-45b9-b37c-3ab17f4b8b6a" }))
                .expect("parse");
        assert!(good.validate(&allowed).is_ok());
    }

    #[test]
    fn validate_uppercase_uuid_normalized_to_lowercase() {
        let allowed = indexes(&json!({ "id": { "type": "uuid" } }));
        let mut filter =
            SearchFilter::from_json_value(&json!({ "id": "0F37C00D-9F18-45B9-B37C-3AB17F4B8B6A" }))
                .expect("parse");
        filter
            .validate(&allowed)
            .expect("uppercase uuid must be accepted");
        assert_eq!(filter.conditions().len(), 1);
        assert_eq!(
            filter.conditions()[0].value,
            json!("0f37c00d-9f18-45b9-b37c-3ab17f4b8b6a")
        );
    }

    #[test]
    fn validate_datetime_format_checked() {
        let allowed = indexes(&json!({ "when": { "type": "datetime" } }));
        let mut bad =
            SearchFilter::from_json_value(&json!({ "when": "yesterday" })).expect("parse");
        let msg = bad_request_message(
            bad.validate(&allowed)
                .expect_err("invalid datetime must be rejected"),
        );
        assert!(msg.contains("\"when\""), "error must name the field: {msg}");

        let mut good = SearchFilter::from_json_value(&json!({ "when": "2026-01-01T00:00:00Z" }))
            .expect("parse");
        assert!(good.validate(&allowed).is_ok());
    }

    #[test]
    fn validate_integer_rejects_float_value() {
        let allowed = indexes(&json!({ "count": { "type": "integer" } }));
        let mut filter = SearchFilter::from_json_value(&json!({ "count": 1.5 })).expect("parse");
        let msg = bad_request_message(
            filter
                .validate(&allowed)
                .expect_err("float value on integer field must be rejected"),
        );
        assert!(
            msg.contains("\"count\""),
            "error must name the field: {msg}"
        );

        let mut good = SearchFilter::from_json_value(&json!({ "count": 3 })).expect("parse");
        assert!(good.validate(&allowed).is_ok());
    }

    #[test]
    fn validate_accepts_valid_mixed() {
        let allowed = indexes(&json!({
            "topic": { "type": "keyword" },
            "count": { "type": "integer" },
            "ratio": { "type": "float" },
            "flag": { "type": "bool" },
            "when": { "type": "datetime" },
            "id": { "type": "uuid" },
        }));
        let mut filter = SearchFilter::from_json_value(&json!({
            "topic": "rust",
            "count": { "gte": 1, "lte": 10 },
            "ratio": { "gt": 0.5 },
            "flag": true,
            "when": { "gte": "2026-01-01T00:00:00Z" },
            "id": "0f37c00d-9f18-45b9-b37c-3ab17f4b8b6a",
        }))
        .expect("parse");
        assert!(filter.validate(&allowed).is_ok());
    }

    #[test]
    fn scope_drops_unknown_fields_and_fills_index_type() {
        let filter = SearchFilter::from_json_value(
            &json!({ "topic": "rust", "count": { "gte": 1 }, "other": 5 }),
        )
        .expect("parse");
        let kb = indexes(&json!({
            "topic": { "type": "keyword" },
            "count": { "type": "integer" },
        }));
        let scoped = filter.scope(&kb);

        assert_eq!(scoped.conditions().len(), 2);
        assert!(scoped.conditions().iter().all(|c| c.field != "other"));
        let topic = scoped
            .conditions()
            .iter()
            .find(|c| c.field == "topic")
            .expect("topic must be kept");
        let count = scoped
            .conditions()
            .iter()
            .find(|c| c.field == "count")
            .expect("count must be kept");
        assert_eq!(topic.index_type, Some(IndexType::Keyword));
        assert_eq!(topic.op, FilterOp::Eq);
        assert_eq!(count.index_type, Some(IndexType::Integer));
        assert_eq!(count.op, FilterOp::Gte);
    }
}
