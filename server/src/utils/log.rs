use crate::domain::models::knowledge_base::KnowledgeBase;
use crate::domain::models::messages::Message;
use crate::domain::models::scenario::Round;

/// A compact, log-friendly rendering of a value that is too large or noisy
/// to record with plain `#[instrument]` auto-capture.
pub trait LogView {
    fn log_view(&self) -> String;
}

impl LogView for [KnowledgeBase] {
    fn log_view(&self) -> String {
        self.iter()
            .map(|kb| format!("{} ({})", kb.id.0, kb.name))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl LogView for Message {
    fn log_view(&self) -> String {
        format!(
            "{} ({:?}, {} parts)",
            self.id.0,
            self.role,
            self.parts.len()
        )
    }
}

impl LogView for Round {
    fn log_view(&self) -> String {
        format!(
            "{} ({:?}, {} parts, {} sources)",
            self.message.id.0,
            self.message.role,
            self.message.parts.len(),
            self.sources.len()
        )
    }
}
