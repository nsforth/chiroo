use crate::domain::models::chat::ChatId;
use crate::domain::models::messages::Message;
use crate::domain::models::source::Source;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct ScenarioId(pub Uuid);

impl ScenarioId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl ScenarioStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ScenarioStatus::Pending => "pending",
            ScenarioStatus::Running => "running",
            ScenarioStatus::Completed => "completed",
            ScenarioStatus::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "running" => ScenarioStatus::Running,
            "completed" => ScenarioStatus::Completed,
            "failed" => ScenarioStatus::Failed,
            _ => ScenarioStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub id: ScenarioId,
    pub chat_id: ChatId,
    pub name: String,
    pub status: ScenarioStatus,
    pub rounds: Vec<Round>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl Scenario {
    /// Build an unsaved Scenario with a generated id and the user message as rounds[0].
    pub fn new(
        chat_id: ChatId,
        name: String,
        status: ScenarioStatus,
        user_message: Message,
    ) -> Self {
        let now = chrono::Utc::now().naive_utc();
        Self {
            id: ScenarioId::new(),
            chat_id,
            name,
            status,
            rounds: vec![Round {
                message: user_message,
                sources: Vec::new(),
                hidden: false,
            }],
            created_at: now,
            updated_at: now,
        }
    }

    pub fn user_message(&self) -> Option<&Message> {
        self.rounds.first().map(|r| &r.message)
    }

    pub fn is_complete(&self) -> bool {
        self.status == ScenarioStatus::Completed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Round {
    pub message: Message,
    pub sources: Vec<Source>,
    /// Hidden rounds are not shown to non-admin users.
    pub hidden: bool,
}

#[cfg(test)]
mod tests {
    use crate::domain::models::messages::Message;

    use super::*;

    fn chat_id() -> ChatId {
        ChatId(Uuid::new_v4())
    }

    fn user_message() -> Message {
        Message::user(chat_id(), "hello".into())
    }

    #[test]
    fn status_as_str_round_trips() {
        for status in [
            ScenarioStatus::Pending,
            ScenarioStatus::Running,
            ScenarioStatus::Completed,
            ScenarioStatus::Failed,
        ] {
            assert_eq!(ScenarioStatus::from_str(status.as_str()), status);
        }
    }

    #[test]
    fn status_from_str_unknown_falls_back_to_pending() {
        assert_eq!(ScenarioStatus::from_str("bogus"), ScenarioStatus::Pending);
        assert_eq!(ScenarioStatus::from_str(""), ScenarioStatus::Pending);
    }

    #[test]
    fn new_scenario_has_user_message_as_first_round() {
        let msg = user_message();
        let msg_id = msg.id.0;
        let scenario = Scenario::new(chat_id(), "model-x".into(), ScenarioStatus::Pending, msg);
        assert_eq!(scenario.rounds.len(), 1);
        assert!(!scenario.rounds[0].hidden);
        assert_eq!(scenario.user_message().map(|m| m.id.0), Some(msg_id));
        assert!(!scenario.is_complete());
    }

    #[test]
    fn is_complete_only_for_completed_status() {
        let mut scenario = Scenario::new(
            chat_id(),
            "m".into(),
            ScenarioStatus::Completed,
            user_message(),
        );
        assert!(scenario.is_complete());
        scenario.status = ScenarioStatus::Failed;
        assert!(!scenario.is_complete());
    }

    #[test]
    fn user_message_empty_rounds_is_none() {
        let scenario = Scenario {
            id: ScenarioId::new(),
            chat_id: chat_id(),
            name: "m".into(),
            status: ScenarioStatus::Pending,
            rounds: vec![],
            created_at: chrono::NaiveDateTime::MIN,
            updated_at: chrono::NaiveDateTime::MIN,
        };
        assert!(scenario.user_message().is_none());
    }

    #[test]
    fn scenario_status_serde_is_snake_case() {
        assert_eq!(
            serde_json::to_string(&ScenarioStatus::Completed).unwrap(),
            "\"completed\""
        );
        let status: ScenarioStatus = serde_json::from_str("\"running\"").unwrap();
        assert_eq!(status, ScenarioStatus::Running);
    }
}
