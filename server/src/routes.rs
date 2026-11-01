use crate::handlers::auth::{login, oidc_callback};
use crate::handlers::branding::{get_branding, get_branding_asset};
use crate::handlers::chats::{
    create_chat, delete_chat, get_chat, get_chats, get_votes, send_message, update_title,
    update_visibility, vote,
};
use crate::handlers::localization::get_localizations;
use crate::handlers::models::{get_scenario_icon, list_models};
use crate::handlers::session::get_session;
use crate::middleware::auth::auth_middleware;
use crate::middleware::csrf::csrf_middleware;
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

    let branding_router = Router::new()
        .route("/", get(get_branding))
        .route("/assets/{kind}", get(get_branding_asset));

    let chat_router = chat_router();

    let protected = Router::new()
        .route("/api/session", get(get_session))
        .route("/api/models", get(list_models))
        .route("/api/scenarios/{id}/icon", get(get_scenario_icon))
        .nest("/api/chats", chat_router)
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            csrf_middleware,
        ))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let openai_router = if state.config.openai_api_enabled() {
        crate::handlers::openai::openai_router().route_layer(middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::openai_auth::openai_auth_middleware,
        ))
    } else {
        Router::new()
    };

    Router::new()
        .route("/", get(root))
        .nest("/api/auth", auth_router)
        .nest("/api/branding", branding_router)
        .route("/api/localizations", get(get_localizations))
        .merge(protected)
        .nest("/api/openai/v1", openai_router)
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
