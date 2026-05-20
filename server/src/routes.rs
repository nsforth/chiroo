use crate::handlers::chats::{
    create_chat, delete_chat, delete_messages_after, delete_vote, get_chat, get_chats,
    get_messages, get_votes, stream_chat, upsert_vote, update_title, update_visibility,
};
use crate::handlers::session::get_session;
use crate::AppState;
use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, patch, post},
    Router,
};

pub fn app_router(state: AppState) -> Router {
    let chat_router = Router::new()
        .route("/", get(get_chats))
        .route("/", post(create_chat))
        .route("/{id}", get(get_chat))
        .route("/{id}", post(stream_chat))
        .route("/{id}", delete(delete_chat))
        .route("/title/{id}", patch(update_title))
        .route("/visibility/{id}", patch(update_visibility))
        .route("/votes/{id}", get(get_votes))
        .route("/votes/{id}", post(upsert_vote))
        .route("/messages/{id}", delete(delete_messages_after));

    Router::new()
        .route("/", get(root))
        .route("/api/session", get(get_session))
        .nest("/api/chats", chat_router)
        .fallback(handler_404)
        .with_state(state)
}

async fn root() -> &'static str {
    "OK"
}

async fn handler_404() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "The requested resource was not found")
}
