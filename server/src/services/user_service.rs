use tracing::{info, instrument};

use deadpool_diesel::postgres::Pool;

use crate::domain::models::user::{User, UserId};
use crate::errors::AppError;
use crate::infra::repositories::user_repository;
use crate::operation_result::OperationResult;

#[derive(Clone)]
pub struct UserService {
    pool: Pool,
}

impl UserService {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    #[instrument(skip(self))]
    pub async fn get_user(&self, user_id: UserId) -> Result<User, AppError> {
        let result = user_repository::find_by_id(&self.pool, user_id.0)
            .await?
            .map(Into::into)
            .ok_or_else(|| AppError::NotFound("User not found".into()));
        match &result {
            Ok(_) => info!(status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    #[instrument(skip(self))]
    pub async fn get_user_by_username(&self, username: &str) -> Result<User, AppError> {
        let result = user_repository::find_by_username(&self.pool, username)
            .await?
            .map(Into::into)
            .ok_or_else(|| AppError::NotFound("User not found".into()));
        match &result {
            Ok(_) => info!(status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    #[instrument(skip(self))]
    pub async fn set_admin(&self, user_id: UserId, admin: bool) -> Result<User, AppError> {
        let result = user_repository::set_admin(&self.pool, user_id.0, admin)
            .await?
            .map(Into::into)
            .ok_or_else(|| AppError::NotFound("User not found".into()));
        match &result {
            Ok(_) => info!(admin = admin, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }
}
