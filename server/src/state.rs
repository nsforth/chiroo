use deadpool_diesel::postgres::Pool;

use crate::services::chat_service::ChatService;
use crate::services::message_service::MessageService;
use crate::services::vote_service::VoteService;

#[derive(Clone)]
pub struct AppState {
    pub chat_service: ChatService,
    pub message_service: MessageService,
    pub vote_service: VoteService,
}
