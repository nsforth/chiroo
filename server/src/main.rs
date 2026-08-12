use deadpool_diesel::postgres::{Manager, Pool};
use deadpool_diesel::Runtime;
use diesel_migrations::MigrationHarness;
use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use std::net::SocketAddr;
use tracing_subscriber::prelude::*;

mod config;
mod domain;
mod errors;
mod handlers;
mod infra;
mod middleware;
mod operation_result;
mod routes;
mod routes_internal;
mod services;
mod state;
mod utils;

use crate::config::{config, Config};
use crate::errors::AppError;
use crate::infra::clients::embedding_client::EmbeddingClient;
use crate::infra::clients::qdrant_client::QdrantClient;
use crate::infra::clients::rerank_client::RerankClient;
use crate::infra::repositories::session_repository::SessionRepository;
use crate::routes::app_router;
use crate::routes_internal::internal_api_router;
use crate::services::ai_service::AiService;
use crate::services::auth_service::AuthService;
use crate::services::chat_service::ChatService;
use crate::services::chunking_service::ChunkingService;
use crate::services::knowledge_service::KnowledgeService;
use crate::services::message_service::MessageService;
use crate::services::prompts::PromptRenderer;
use crate::services::vote_service::VoteService;
use crate::state::AppState;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/");

#[tokio::main]
async fn main() {
    init_tracing();

    let app_config = config().await;
    tracing::info!("application configuration is {:?}", app_config);

    let manager = Manager::new(app_config.db_url().to_string(), Runtime::Tokio1);
    let pool = Pool::builder(manager)
        .build()
        .expect("Failed to create connection pool");

    if let Err(err) = run_migrations(&pool).await {
        tracing::error!("Failed to run migrations: {:?}", err);
        return;
    }

    let host = app_config.server_host();
    let port = app_config.server_port();
    let address = format!("{host}:{port}");
    let socket_addr: SocketAddr = address.parse().expect("Unable to parse socket address");

    tracing::info!("listening on http://{}", socket_addr);

    let listener = tokio::net::TcpListener::bind(socket_addr)
        .await
        .expect("Failed to bind");

    let state = build_state(app_config, pool);

    let app = app_router(state.clone());

    spawn_internal_api(state).await;

    axum::serve(listener, app)
        .await
        .expect("Server failed to run");
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

async fn run_migrations(pool: &Pool) -> Result<(), AppError> {
    let conn = pool
        .get()
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    conn.interact(|conn_inner| conn_inner.run_pending_migrations(MIGRATIONS).map(|_| ()))
        .await
        .map_err(|e| AppError::DatabaseError(e.to_string()))?
        .map_err(|e| AppError::DatabaseError(e.to_string()))?;
    Ok(())
}

fn build_state(app_config: &'static Config, pool: Pool) -> AppState {
    let session_repo = SessionRepository::new(app_config.db_encryption_key());

    let auth_service =
        AuthService::new(session_repo, app_config.oidc_config().clone(), pool.clone());
    auth_service
        .clone()
        .start_background_jobs(app_config.oidc_config().cleanup_interval_secs());

    let message_service = MessageService::new(pool.clone());

    let prompt_renderer = PromptRenderer::new("prompts").expect("Failed to load prompt templates");

    let ai_service = AiService::new(&app_config.openai, prompt_renderer, message_service.clone());

    let qdrant_client = QdrantClient::new(
        app_config.qdrant.url(),
        app_config.qdrant.api_key(),
        app_config.qdrant.collection_prefix().to_string(),
    )
    .expect("Failed to create Qdrant client");

    let embedding_client = EmbeddingClient::new(&app_config.embedding);

    let rerank_client =
        RerankClient::new(&app_config.rerank).expect("Failed to create Rerank client");

    let chunking_service = ChunkingService::new(app_config.chunking.max_chunk_size);

    let embedding_dimensions = app_config.embedding.dimensions();

    AppState {
        chat_service: ChatService::new(pool.clone(), message_service.clone()),
        message_service,
        vote_service: VoteService::new(pool.clone()),
        ai_service,
        auth_service,
        knowledge_service: KnowledgeService::new(
            pool,
            chunking_service.clone(),
            qdrant_client.clone(),
            embedding_client.clone(),
            embedding_dimensions,
        ),
        qdrant_client,
        embedding_client,
        rerank_client,
        chunking_service,
        config: app_config,
    }
}

async fn spawn_internal_api(state: AppState) {
    let app = internal_api_router(state.clone());

    let host = state.config.internal_api_host().to_string();
    let port = state.config.internal_api_port();
    let address = format!("{host}:{port}");
    let socket_addr: SocketAddr = address
        .parse()
        .expect("Unable to parse internal API address");

    tracing::info!("internal API listening on http://{}", socket_addr);

    let listener = tokio::net::TcpListener::bind(socket_addr)
        .await
        .expect("Failed to bind internal API");

    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("Internal API server failed to run");
    });
}
