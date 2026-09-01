use uuid::Uuid;

use crate::domain::models::knowledge_base::KnowledgeBase;

pub struct KbLogView<'a> {
    items: Vec<(Uuid, &'a str)>,
}

impl std::fmt::Display for KbLogView<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let items = self
            .items
            .iter()
            .map(|(id, name)| format!("{id} ({name})"))
            .collect::<Vec<_>>()
            .join(", ");
        write!(f, "{items}")
    }
}

pub trait KnowledgeBaseLogView {
    fn log_view(&self) -> KbLogView<'_>;
}

impl KnowledgeBaseLogView for [KnowledgeBase] {
    fn log_view(&self) -> KbLogView<'_> {
        KbLogView {
            items: self.iter().map(|kb| (kb.id.0, kb.name.as_str())).collect(),
        }
    }
}
