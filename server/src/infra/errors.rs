use std::fmt;

use deadpool_diesel::InteractError;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use tracing::debug;

#[derive(Debug)]
pub enum InfraError {
    InternalServerError,
    NotFound,
    Conflict,
}

impl fmt::Display for InfraError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "Not found"),
            Self::InternalServerError => write!(f, "Internal server error"),
            Self::Conflict => write!(f, "Conflict"),
        }
    }
}

impl From<DieselError> for InfraError {
    fn from(error: DieselError) -> Self {
        match error {
            DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)
            | DieselError::DatabaseError(DatabaseErrorKind::ForeignKeyViolation, _) => {
                Self::Conflict
            },
            DieselError::NotFound => Self::NotFound,
            _ => Self::InternalServerError,
        }
    }
}

impl From<deadpool_diesel::PoolError> for InfraError {
    fn from(_error: deadpool_diesel::PoolError) -> Self {
        Self::InternalServerError
    }
}

impl From<InteractError> for InfraError {
    fn from(_error: InteractError) -> Self {
        Self::InternalServerError
    }
}

pub(crate) fn map_infra_error<E>(e: E) -> InfraError
where
    E: fmt::Display,
    InfraError: From<E>,
{
    debug!(error = e.to_string(), status = "failed");
    InfraError::from(e)
}
