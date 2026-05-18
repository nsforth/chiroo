use deadpool_diesel::postgres::Pool;

use crate::services::chat_service::ChatService;

#[derive(Clone)]
pub struct AppState {
    pub chat_service: ChatService,
}
