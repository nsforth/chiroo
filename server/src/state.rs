use crate::services::ai_service::AiService;
use crate::services::chat_service::ChatService;
use crate::services::message_service::MessageService;
use crate::services::vote_service::VoteService;

#[derive(Clone)]
pub struct AppState {
    pub chat_service: ChatService,
    pub message_service: MessageService,
    pub vote_service: VoteService,
    pub ai_service: AiService,
}
