pub mod config;
pub mod domain;
pub mod errors;
pub mod handlers;
pub mod infra;
pub mod middleware;
pub mod operation_result;
pub mod routes;
pub mod routes_internal;
pub mod services;
pub mod state;
pub mod utils;

pub use state::AppState;
