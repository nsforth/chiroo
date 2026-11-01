use chrono::Utc;
use serde::Serialize;

use super::error::OpenAiApiError;
use crate::domain::models::scenario::Scenario;
use crate::domain::models::source::Source;

#[derive(Debug, Serialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: &'static str,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: CompletionUsage,
    pub sources: Vec<Source>,
    pub chiroo: ChirooMeta,
}

#[derive(Debug, Serialize)]
pub struct Choice {
    pub index: u8,
    pub message: ChoiceMessage,
    pub logprobs: Option<serde_json::Value>,
    pub finish_reason: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ChoiceMessage {
    pub role: &'static str,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct CompletionUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<PromptTokensDetails>,
}

#[derive(Debug, Serialize)]
pub struct PromptTokensDetails {
    pub cached_tokens: u32,
}

#[derive(Debug, Serialize)]
pub struct ChirooMeta {
    pub chat_id: String,
    pub scenario_id: String,
}

impl ChatCompletionResponse {
    pub fn from_scenario(scenario: &Scenario) -> Result<Self, OpenAiApiError> {
        let content = scenario
            .rounds
            .iter()
            .enumerate()
            .filter(|(i, round)| *i >= 1 && !round.hidden)
            .last()
            .map(|(_, round)| round.message.text())
            .ok_or_else(|| OpenAiApiError::internal("Scenario has no visible assistant round"))?;

        let mut sources: Vec<Source> = Vec::new();
        for round in &scenario.rounds {
            for source in &round.sources {
                if !sources.iter().any(|s| s.source_id == source.source_id) {
                    sources.push(source.clone());
                }
            }
        }

        let usage = match scenario.usage.as_ref() {
            Some(u) => CompletionUsage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
                prompt_tokens_details: (u.cached_prompt_tokens > 0).then(|| PromptTokensDetails {
                    cached_tokens: u.cached_prompt_tokens,
                }),
            },
            None => CompletionUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
                prompt_tokens_details: None,
            },
        };

        Ok(Self {
            id: format!("chatcmpl-{}", scenario.id.0),
            object: "chat.completion",
            created: Utc::now().timestamp(),
            model: scenario.name.clone(),
            choices: vec![Choice {
                index: 0,
                message: ChoiceMessage {
                    role: "assistant",
                    content,
                },
                logprobs: None,
                finish_reason: "stop",
            }],
            usage,
            sources,
            chiroo: ChirooMeta {
                chat_id: scenario.chat_id.0.to_string(),
                scenario_id: scenario.id.0.to_string(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use axum::response::IntoResponse;
    use uuid::Uuid;

    use super::*;
    use crate::domain::models::chat::ChatId;
    use crate::domain::models::messages::{Message, MessagePart, Role, TextBlock};
    use crate::domain::models::scenario::{Round, ScenarioStatus};
    use crate::domain::models::token_usage::TokenUsage;

    fn chat_id() -> ChatId {
        ChatId(Uuid::new_v4())
    }

    fn assistant_round(text: &str, hidden: bool, sources: Vec<Source>) -> Round {
        let mut message = Message::new(chat_id(), Role::Assistant);
        message.parts.push(MessagePart::Text(TextBlock {
            text: text.to_string(),
        }));
        Round {
            message,
            sources,
            hidden,
            usage: None,
        }
    }

    fn source(id: &str) -> Source {
        Source {
            source_id: id.to_string(),
            url: format!("https://example.com/{id}"),
            title: Some(format!("doc {id}")),
            score: Some(0.9),
        }
    }

    /// rounds: user, hidden (sources a, b), hidden (source a again), visible, visible answer.
    fn finished_scenario(usage: Option<TokenUsage>) -> Scenario {
        let mut scenario = Scenario::new(
            chat_id(),
            "simple".to_string(),
            ScenarioStatus::Completed,
            Message::user(chat_id(), "question".to_string()),
        );
        scenario.rounds.push(assistant_round(
            "searching",
            true,
            vec![source("a"), source("b")],
        ));
        scenario
            .rounds
            .push(assistant_round("extracting", true, vec![source("a")]));
        scenario
            .rounds
            .push(assistant_round("mid answer", false, Vec::new()));
        scenario
            .rounds
            .push(assistant_round("final answer", false, Vec::new()));
        scenario.usage = usage;
        scenario
    }

    fn value(response: &ChatCompletionResponse) -> serde_json::Value {
        serde_json::to_value(response).expect("serialize response")
    }

    #[test]
    fn content_is_the_last_visible_round() {
        let scenario = finished_scenario(None);
        let response = ChatCompletionResponse::from_scenario(&scenario).expect("convert");
        assert_eq!(response.choices[0].message.content, "final answer");
        assert_eq!(response.choices[0].message.role, "assistant");
        assert_eq!(response.choices[0].index, 0);
        assert_eq!(response.choices[0].finish_reason, "stop");
    }

    #[test]
    fn scenario_without_visible_round_is_an_error() {
        let scenario = Scenario::new(
            chat_id(),
            "simple".to_string(),
            ScenarioStatus::Completed,
            Message::user(chat_id(), "question".to_string()),
        );
        let error = ChatCompletionResponse::from_scenario(&scenario).unwrap_err();
        assert_eq!(error.into_response().status(), 500);
    }

    #[test]
    fn sources_are_deduped_first_occurrence_order() {
        let scenario = finished_scenario(None);
        let response = ChatCompletionResponse::from_scenario(&scenario).expect("convert");
        let ids: Vec<&str> = response
            .sources
            .iter()
            .map(|s| s.source_id.as_str())
            .collect();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn usage_is_mapped_including_cached_details() {
        let scenario = finished_scenario(Some(TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            cached_prompt_tokens: 40,
        }));
        let response = ChatCompletionResponse::from_scenario(&scenario).expect("convert");
        let value = value(&response);
        assert_eq!(value["usage"]["prompt_tokens"], 100);
        assert_eq!(value["usage"]["completion_tokens"], 50);
        assert_eq!(value["usage"]["total_tokens"], 150);
        assert_eq!(value["usage"]["prompt_tokens_details"]["cached_tokens"], 40);
    }

    #[test]
    fn usage_omits_details_without_cached_tokens() {
        let scenario = finished_scenario(Some(TokenUsage {
            prompt_tokens: 1,
            completion_tokens: 2,
            total_tokens: 3,
            cached_prompt_tokens: 0,
        }));
        let response = ChatCompletionResponse::from_scenario(&scenario).expect("convert");
        let value = value(&response);
        assert_eq!(value["usage"].as_object().unwrap().len(), 3);
    }

    #[test]
    fn usage_is_zeroed_when_scenario_has_no_usage() {
        let scenario = finished_scenario(None);
        let response = ChatCompletionResponse::from_scenario(&scenario).expect("convert");
        let value = value(&response);
        assert_eq!(value["usage"]["prompt_tokens"], 0);
        assert_eq!(value["usage"]["completion_tokens"], 0);
        assert_eq!(value["usage"]["total_tokens"], 0);
        assert!(!value["usage"]
            .as_object()
            .unwrap()
            .contains_key("prompt_tokens_details"));
    }

    #[test]
    fn envelope_fields_match_openai_shape() {
        let scenario = finished_scenario(None);
        let chat_id = scenario.chat_id;
        let scenario_id = scenario.id;
        let response = ChatCompletionResponse::from_scenario(&scenario).expect("convert");
        let value = value(&response);
        assert_eq!(value["object"], "chat.completion");
        assert!(value["id"].as_str().unwrap().starts_with("chatcmpl-"));
        assert_eq!(value["model"], "simple");
        assert_eq!(value["choices"][0]["logprobs"], serde_json::Value::Null);
        assert_eq!(value["chiroo"]["chat_id"], chat_id.0.to_string());
        assert_eq!(value["chiroo"]["scenario_id"], scenario_id.0.to_string());
        assert!(value["created"].as_i64().unwrap() > 0);
    }
}
