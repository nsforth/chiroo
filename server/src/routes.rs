use crate::handlers::auth::{login, oidc_callback};
use crate::handlers::chats::{
    create_chat, delete_chat, delete_messages_after, get_chat, get_chats, get_votes, send_message,
    update_title, update_visibility, vote,
};
use crate::handlers::session::get_session;
use crate::middleware::auth::auth_middleware;
use crate::AppState;
use axum::{
    http::StatusCode,
    middleware,
    response::IntoResponse,
    routing::{delete, get, patch, post},
    Router,
};
use tower_http::trace::TraceLayer;

pub fn app_router(state: AppState) -> Router {
    let auth_router = Router::new()
        .route("/login", post(login))
        .route("/oidc/callback", get(oidc_callback));

    let chat_router = chat_router();

    let protected = Router::new()
        .route("/api/session", get(get_session))
        .nest("/api/chats", chat_router)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    Router::new()
        .route("/", get(root))
        .nest("/api/auth", auth_router)
        .merge(protected)
        .fallback(handler_404)
        .layer(TraceLayer::new_for_http())
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
