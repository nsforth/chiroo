use serde::Deserialize;
use serde_json::Value;

use crate::domain::models::messages::Role;

use super::error::OpenAiApiError;

#[derive(Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<OpenAiMessage>,
    #[serde(default)]
    pub stream: Option<bool>,
    pub tools: Option<Value>,
    pub tool_choice: Option<Value>,
    pub functions: Option<Value>,
    pub function_call: Option<Value>,
    pub response_format: Option<Value>,
    pub n: Option<u32>,
    pub logprobs: Option<Value>,
    pub top_logprobs: Option<Value>,
}

#[derive(Deserialize)]
pub struct OpenAiMessage {
    pub role: String,
    pub content: Value,
}

#[derive(Debug)]
pub struct ConvertedMessages {
    pub client_system: Option<String>,
    pub history: Vec<(Role, String)>,
    pub question: String,
}

impl ChatCompletionRequest {
    pub fn convert(&self) -> Result<ConvertedMessages, OpenAiApiError> {
        if self.stream == Some(true) {
            return Err(OpenAiApiError::bad_request("Streaming is not supported"));
        }
        if self.tools.is_some() {
            return Err(OpenAiApiError::bad_request(
                "Client tools are not supported",
            ));
        }
        if self.tool_choice.is_some() {
            return Err(OpenAiApiError::bad_request(
                "Client tools are not supported",
            ));
        }
        if self.functions.is_some() {
            return Err(OpenAiApiError::bad_request(
                "Client functions are not supported",
            ));
        }
        if self.function_call.is_some() {
            return Err(OpenAiApiError::bad_request(
                "Client functions are not supported",
            ));
        }
        if self.response_format.is_some() {
            return Err(OpenAiApiError::bad_request(
                "response_format is not supported",
            ));
        }
        if self.logprobs.is_some() {
            return Err(OpenAiApiError::bad_request("logprobs is not supported"));
        }
        if self.top_logprobs.is_some() {
            return Err(OpenAiApiError::bad_request("top_logprobs is not supported"));
        }
        if let Some(n) = self.n {
            if n != 1 {
                return Err(OpenAiApiError::bad_request("Only n=1 is supported"));
            }
        }
        if self.messages.is_empty() {
            return Err(OpenAiApiError::bad_request("messages must not be empty"));
        }
        let last = self
            .messages
            .last()
            .expect("messages checked non-empty above");
        if last.role != "user" {
            return Err(OpenAiApiError::bad_request(
                "The last message must be from the user",
            ));
        }

        let mut client_systems = Vec::new();
        let mut history = Vec::new();
        let mut question = String::new();

        for (i, message) in self.messages.iter().enumerate() {
            let text = extract_content(&message.content)?;
            match message.role.as_str() {
                "system" | "developer" => client_systems.push(text),
                "user" | "assistant" => {
                    if i == self.messages.len() - 1 {
                        question = text;
                    } else {
                        let role = if message.role == "user" {
                            Role::User
                        } else {
                            Role::Assistant
                        };
                        history.push((role, text));
                    }
                },
                other => {
                    return Err(OpenAiApiError::bad_request(format!(
                        "Unsupported message role: {other}"
                    )))
                },
            }
        }

        Ok(ConvertedMessages {
            client_system: (!client_systems.is_empty()).then(|| client_systems.join("\n\n")),
            history,
            question,
        })
    }
}

fn extract_content(content: &Value) -> Result<String, OpenAiApiError> {
    match content {
        Value::String(text) => Ok(text.clone()),
        Value::Array(parts) => {
            let mut texts = Vec::with_capacity(parts.len());
            for part in parts {
                let part_type = part.get("type").and_then(Value::as_str).ok_or_else(|| {
                    OpenAiApiError::bad_request("Unsupported content part: missing \"type\"")
                })?;
                if part_type != "text" {
                    return Err(OpenAiApiError::bad_request(format!(
                        "Unsupported content part type: {part_type}"
                    )));
                }
                let text = part.get("text").and_then(Value::as_str).ok_or_else(|| {
                    OpenAiApiError::bad_request("Unsupported content part: missing \"text\"")
                })?;
                texts.push(text);
            }
            Ok(texts.join(""))
        },
        _ => Err(OpenAiApiError::bad_request(
            "Unsupported message content type",
        )),
    }
}

#[cfg(test)]
mod tests {
    use axum::response::IntoResponse;
    use serde_json::json;

    use super::*;

    fn msg(role: &str, content: Value) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("role".to_string(), json!(role));
        map.insert("content".to_string(), content);
        Value::Object(map)
    }

    fn chat_request(messages: Vec<Value>) -> Value {
        let mut map = serde_json::Map::new();
        map.insert("model".to_string(), json!("simple"));
        map.insert("messages".to_string(), Value::Array(messages));
        Value::Object(map)
    }

    fn with_field(mut value: Value, field: &str, content: Value) -> Value {
        value
            .as_object_mut()
            .unwrap()
            .insert(field.to_string(), content);
        value
    }

    fn request(value: Value) -> ChatCompletionRequest {
        serde_json::from_value(value).expect("request should deserialize")
    }

    fn convert(value: Value) -> Result<ConvertedMessages, OpenAiApiError> {
        request(value).convert()
    }

    fn ok(value: Value) -> ConvertedMessages {
        convert(value).expect("request should convert")
    }

    fn is_400(error: OpenAiApiError) {
        assert_eq!(error.into_response().status(), 400);
    }

    #[test]
    fn stream_true_is_rejected() {
        let value = with_field(
            chat_request(vec![msg("user", json!("hi"))]),
            "stream",
            json!(true),
        );
        is_400(convert(value).unwrap_err());
    }

    #[test]
    fn rejected_fields_are_rejected() {
        for field in [
            "tools",
            "tool_choice",
            "functions",
            "function_call",
            "response_format",
            "logprobs",
            "top_logprobs",
        ] {
            let value = with_field(
                chat_request(vec![msg("user", json!("hi"))]),
                field,
                json!([]),
            );
            is_400(convert(value).unwrap_err());
        }
    }

    #[test]
    fn n_other_than_one_is_rejected() {
        let value = with_field(chat_request(vec![msg("user", json!("hi"))]), "n", json!(2));
        is_400(convert(value).unwrap_err());
    }

    #[test]
    fn n_one_is_allowed() {
        let value = with_field(chat_request(vec![msg("user", json!("hi"))]), "n", json!(1));
        assert!(convert(value).is_ok());
    }

    #[test]
    fn empty_messages_are_rejected() {
        is_400(convert(chat_request(Vec::new())).unwrap_err());
    }

    #[test]
    fn last_message_must_be_from_user() {
        let value = chat_request(vec![
            msg("user", json!("hi")),
            msg("assistant", json!("hello")),
        ]);
        is_400(convert(value).unwrap_err());
    }

    #[test]
    fn tool_and_function_roles_are_rejected() {
        for role in ["tool", "function"] {
            let value = chat_request(vec![msg(role, json!("x")), msg("user", json!("hi"))]);
            is_400(convert(value).unwrap_err());
        }
    }

    #[test]
    fn image_content_part_is_rejected() {
        let part = json!({ "type": "image_url", "image_url": {} });
        let value = chat_request(vec![msg("user", json!([part]))]);
        is_400(convert(value).unwrap_err());
    }

    #[test]
    fn malformed_content_parts_are_rejected() {
        let missing_type = json!({ "text": "x" });
        let non_string_text = json!({ "type": "text", "text": 42 });
        let number_content = json!(42);
        let plain_string_part = json!("plain");
        for content in [
            json!([missing_type]),
            json!([non_string_text]),
            json!([plain_string_part]),
            number_content,
        ] {
            let value = chat_request(vec![msg("user", content)]);
            is_400(convert(value).unwrap_err());
        }
    }

    #[test]
    fn system_messages_are_joined_into_client_system() {
        let value = chat_request(vec![
            msg("system", json!("rule one")),
            msg("system", json!("rule two")),
            msg("user", json!("hi")),
        ]);
        let converted = ok(value);
        assert_eq!(
            converted.client_system.as_deref(),
            Some("rule one\n\nrule two")
        );
        assert!(converted.history.is_empty());
        assert_eq!(converted.question, "hi");
    }

    #[test]
    fn developer_is_treated_as_system() {
        let value = chat_request(vec![
            msg("developer", json!("be brief")),
            msg("user", json!("hi")),
        ]);
        let converted = ok(value);
        assert_eq!(converted.client_system.as_deref(), Some("be brief"));
    }

    #[test]
    fn no_system_messages_gives_none_client_system() {
        let value = chat_request(vec![msg("user", json!("hi"))]);
        let converted = ok(value);
        assert!(converted.client_system.is_none());
        assert!(converted.history.is_empty());
    }

    #[test]
    fn history_keeps_order_and_drops_system_and_last_user() {
        let value = chat_request(vec![
            msg("system", json!("sys")),
            msg("user", json!("q1")),
            msg("assistant", json!("a1")),
            msg("user", json!("q2")),
        ]);
        let converted = ok(value);
        let roles: Vec<Role> = converted.history.iter().map(|(r, _)| r.clone()).collect();
        assert_eq!(roles.len(), 2);
        assert!(matches!(roles[0], Role::User));
        assert!(matches!(roles[1], Role::Assistant));
        let texts: Vec<&str> = converted.history.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(texts, vec!["q1", "a1"]);
        assert_eq!(converted.question, "q2");
    }

    #[test]
    fn string_content_is_used_as_is() {
        let value = chat_request(vec![msg("user", json!("plain string"))]);
        let converted = ok(value);
        assert_eq!(converted.question, "plain string");
    }

    #[test]
    fn array_of_text_parts_is_joined() {
        let part_one = json!({ "type": "text", "text": "part one " });
        let part_two = json!({ "type": "text", "text": "part two" });
        let value = chat_request(vec![msg("user", json!([part_one, part_two]))]);
        let converted = ok(value);
        assert_eq!(converted.question, "part one part two");
    }

    #[test]
    fn unknown_top_level_fields_are_ignored() {
        let value = with_field(
            with_field(
                chat_request(vec![msg("user", json!("hi"))]),
                "temperature",
                json!(0.7),
            ),
            "user",
            json!("someone"),
        );
        let converted = ok(value);
        assert_eq!(converted.question, "hi");
    }
}
