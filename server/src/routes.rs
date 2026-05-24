use crate::handlers::chats::{
    create_chat, delete_chat, delete_messages_after, get_chat, get_chats, get_votes, send_message,
    update_title, update_visibility, vote,
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
    let chat_router = chat_router();

    Router::new()
        .route("/", get(root))
        .route("/api/session", get(get_session))
        .nest("/api/chats", chat_router)
        .fallback(handler_404)
        .with_state(state)
}

fn chat_router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_chats))
        .route("/", post(create_chat))
        .route("/{id}", get(get_chat))
        .route("/{id}", post(send_message))
        .route("/{id}", delete(delete_chat))
        .route("/title/{id}", patch(update_title))
        .route("/visibility/{id}", patch(update_visibility))
        .route("/votes/{id}", get(get_votes))
        .route("/votes/{id}", post(vote))
        .route("/messages/{id}", delete(delete_messages_after))
}

async fn root() -> &'static str {
    "OK"
}

async fn handler_404() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        "The requested resource was not found",
    )
}
