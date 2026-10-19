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

#[cfg(test)]
mod tests {
    use diesel::result::{DatabaseErrorKind, Error as DieselError};

    use super::*;

    struct TestDbInfo;

    impl fmt::Debug for TestDbInfo {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("TestDbInfo").finish()
        }
    }

    impl diesel::result::DatabaseErrorInformation for TestDbInfo {
        fn message(&self) -> &str {
            "test"
        }

        fn details(&self) -> Option<&str> {
            None
        }

        fn hint(&self) -> Option<&str> {
            None
        }

        fn table_name(&self) -> Option<&str> {
            None
        }

        fn column_name(&self) -> Option<&str> {
            None
        }

        fn constraint_name(&self) -> Option<&str> {
            None
        }

        fn statement_position(&self) -> Option<i32> {
            None
        }
    }

    fn db_error(kind: DatabaseErrorKind) -> DieselError {
        DieselError::DatabaseError(kind, Box::new(TestDbInfo))
    }

    #[test]
    fn unique_violation_maps_to_conflict() {
        let err: InfraError = db_error(DatabaseErrorKind::UniqueViolation).into();
        assert!(matches!(err, InfraError::Conflict));
    }

    #[test]
    fn foreign_key_violation_maps_to_conflict() {
        let err: InfraError = db_error(DatabaseErrorKind::ForeignKeyViolation).into();
        assert!(matches!(err, InfraError::Conflict));
    }

    #[test]
    fn not_found_maps_to_not_found() {
        let err: InfraError = DieselError::NotFound.into();
        assert!(matches!(err, InfraError::NotFound));
    }

    #[test]
    fn other_db_errors_map_to_internal() {
        let err: InfraError = db_error(DatabaseErrorKind::NotNullViolation).into();
        assert!(matches!(err, InfraError::InternalServerError));
    }

    #[test]
    fn pool_error_maps_to_internal() {
        let err: InfraError = deadpool_diesel::PoolError::Closed.into();
        assert!(matches!(err, InfraError::InternalServerError));
    }

    #[test]
    fn map_infra_error_converts_and_returns_infra_error() {
        let err = map_infra_error(DieselError::NotFound);
        assert!(matches!(err, InfraError::NotFound));
    }
}
