use std::fmt;

use deadpool_diesel::InteractError;

#[derive(Debug)]
pub enum InfraError {
    InternalServerError,
    NotFound,
}

impl fmt::Display for InfraError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "Not found"),
            Self::InternalServerError => write!(f, "Internal server error"),
        }
    }
}

impl From<diesel::result::Error> for InfraError {
    fn from(error: diesel::result::Error) -> Self {
        match error {
            diesel::result::Error::NotFound => Self::NotFound,
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
