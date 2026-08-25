use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, patch, post, put},
    Router,
};

use crate::handlers::internal::documents::{
    create_document, delete_document, get_document, get_document_data, list_document_versions,
    list_documents, reindex_document, update_document, update_document_metadata,
};
use crate::handlers::internal::knowledge_bases::{
    create_kb, delete_kb, get_kb, list_kbs, update_kb,
};
use crate::handlers::internal::users::{get_user_by_username, update_user_admin};
use crate::state::AppState;

pub fn internal_api_router(state: AppState) -> Router {
    let kb_router = Router::new()
        .route("/", post(create_kb))
        .route("/", get(list_kbs))
        .route("/{id}", get(get_kb))
        .route("/{id}", put(update_kb))
        .route("/{id}", delete(delete_kb));

    let doc_router = Router::new()
        .route("/", post(create_document))
        .route("/", get(list_documents))
        .route("/versions", get(list_document_versions))
        .route("/{id}/data", get(get_document_data))
        .route("/{id}/reindex", post(reindex_document))
        .route("/{id}", get(get_document))
        .route("/{id}", put(update_document))
        .route("/{id}", patch(update_document_metadata))
        .route("/{id}", delete(delete_document));

    let user_router = Router::new()
        .route("/", get(get_user_by_username))
        .route("/{id}", patch(update_user_admin));

    Router::new()
        .nest("/internal/api/knowledge-bases", kb_router)
        .nest("/internal/api/documents", doc_router)
        .nest("/internal/api/users", user_router)
        .fallback(handler_404)
        .with_state(state)
}

async fn handler_404() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        "The requested resource was not found",
    )
}
