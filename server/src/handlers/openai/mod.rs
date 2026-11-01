use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::routing::{get, post};
use axum::Router;

use crate::domain::models::api_principal::ApiPrincipal;
use crate::state::AppState;

pub mod chat;
pub mod error;
pub mod messages;
pub mod models;
pub mod response;

impl<S: Send + Sync> FromRequestParts<S> for ApiPrincipal {
    type Rejection = error::OpenAiApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<ApiPrincipal>()
            .cloned()
            .ok_or_else(|| error::OpenAiApiError::unauthenticated("Authentication required"))
    }
}

pub fn openai_router() -> Router<AppState> {
    Router::new()
        .route("/models", get(models::list_models))
        .route("/models/{model}", get(models::get_model))
        .route("/chat/completions", post(chat::create_chat_completion))
}
