use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub mod chat;
pub mod error;
pub mod messages;
pub mod models;
pub mod response;

pub fn openai_router() -> Router<AppState> {
    Router::new()
        .route("/models", get(models::list_models))
        .route("/models/{model}", get(models::get_model))
        .route("/chat/completions", post(chat::create_chat_completion))
}
