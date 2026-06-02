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
mod routes;
mod services;
mod state;
mod utils;

use crate::config::config;
use crate::errors::{internal_error, AppError};
use crate::routes::app_router;
use crate::services::ai_service::AiService;
use crate::services::auth_service::AuthService;
use crate::services::chat_service::ChatService;
use crate::services::message_service::MessageService;
use crate::services::vote_service::VoteService;
use crate::state::AppState;

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/");

#[tokio::main]
async fn main() {
    init_tracing();

    let app_config = config().await;

    let manager = Manager::new(app_config.db_url().to_string(), Runtime::Tokio1);
    let pool = Pool::builder(manager)
        .build()
        .expect("Failed to create connection pool");

    if let Err(err) = run_migrations(&pool).await {
        tracing::error!("Failed to run migrations: {:?}", err);
        return;
    }

    let auth_service = AuthService::new(app_config.oidc_config().clone(), pool.clone());
    auth_service.clone().start_background_jobs(app_config.oidc_config().cleanup_interval_secs());

    let state = AppState {
        chat_service: ChatService::new(pool.clone(), MessageService::new(pool.clone())),
        message_service: MessageService::new(pool.clone()),
        vote_service: VoteService::new(pool.clone()),
        ai_service: AiService::new(&app_config.openai),
        auth_service,
    };

    let app = app_router(state);

    let host = app_config.server_host();
    let port = app_config.server_port();
    let address = format!("{host}:{port}");
    let socket_addr: SocketAddr = address.parse().expect("Unable to parse socket address");

    tracing::info!("listening on http://{}", socket_addr);

    let listener = tokio::net::TcpListener::bind(socket_addr)
        .await
        .expect("Failed to bind");

    axum::serve(listener, app)
        .await
        .expect("Server failed to run");
}

fn init_tracing() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

async fn run_migrations(pool: &Pool) -> Result<(), AppError> {
    let conn = pool.get().await.map_err(internal_error)?;
    conn.interact(|conn_inner| conn_inner.run_pending_migrations(MIGRATIONS).map(|_| ()))
        .await
        .map_err(internal_error)?
        .map_err(internal_error)?;
    Ok(())
}
