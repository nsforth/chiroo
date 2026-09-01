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
}
