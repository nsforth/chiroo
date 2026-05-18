use crate::handlers::{chats::create_chat, chats::get_chats, session::get_session};
use crate::AppState;
use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};

pub fn app_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/api/session", get(get_session))
        .route("/api/chats", get(get_chats))
        .route("/api/chats", post(create_chat))
        //.nest("/v1/posts", posts_routes(state.clone()))
        .fallback(handler_404)
        .with_state(state)
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

fn posts_routes(state: AppState) -> Router<AppState> {
    Router::new()
        //.route("/", post(create_post))
        //.route("/", get(list_posts))
        //.route("/{id}", get(get_post))
        .with_state(state)
}
