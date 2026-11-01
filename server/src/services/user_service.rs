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

    /// Create or update a user by SSO identity (upsert on users.sso_id).
    /// Users are always created as non-admins; the admin flag is granted
    /// exclusively through `set_admin`.
    #[instrument(skip(self, sso_id, username))]
    pub async fn create_user(
        &self,
        sso_id: &str,
        sso_provider: &str,
        username: &str,
        name: Option<&str>,
        email: Option<&str>,
    ) -> Result<User, AppError> {
        let result: Result<User, AppError> = match user_repository::upsert(
            &self.pool,
            sso_id,
            sso_provider,
            name,
            username,
            email,
            None,
        )
        .await
        {
            Ok(db) => Ok(User::from(db)),
            Err(error) => Err(error.into()),
        };
        match &result {
            Ok(user) => info!(user_id = %user.id.0, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    /// Delete a user; with `purge_chats` also delete the user's chats first, in one
    /// transaction. Without it, chats are kept and only `sessions` cascade away
    /// with the user.
    #[instrument(skip(self))]
    pub async fn delete_user(&self, user_id: UserId, purge_chats: bool) -> Result<(), AppError> {
        let deleted = if purge_chats {
            user_repository::delete_user_with_chats(&self.pool, user_id.0).await
        } else {
            user_repository::delete_user(&self.pool, user_id.0).await
        };
        let result: Result<(), AppError> = match deleted {
            Ok(true) => Ok(()),
            Ok(false) => Err(AppError::NotFound("User not found".into())),
            Err(_) => Err(AppError::InternalServerError),
        };
        match &result {
            Ok(()) => info!(purge_chats = purge_chats, status = result.status()),
            Err(e) => info!(
                purge_chats = purge_chats,
                error = e.to_string(),
                status = result.status()
            ),
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

#[cfg(test)]
mod integration {
    use deadpool_diesel::postgres::{Manager, Pool};
    use deadpool_diesel::Runtime;
    use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
    use tokio::sync::OnceCell;
    use uuid::Uuid;

    use crate::domain::models::chat::ChatId;
    use crate::infra::repositories::{chat_repository, user_repository};

    use super::*;

    const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/");

    static TEST_POOL: OnceCell<Pool> = OnceCell::const_new();

    fn test_db_url() -> String {
        std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://chiroo:1234@127.0.0.1/chiroo_tests".to_string())
    }

    async fn get_or_create_test_pool() -> Pool {
        TEST_POOL
            .get_or_init(|| async {
                let manager = Manager::new(test_db_url(), Runtime::Tokio1);
                let pool = Pool::builder(manager)
                    .build()
                    .expect("Failed to create test pool");

                let conn = pool.get().await.expect("Failed to get connection");
                conn.interact(|conn| conn.run_pending_migrations(MIGRATIONS).map(|_| ()))
                    .await
                    .expect("interact failed")
                    .expect("migrations failed");

                pool
            })
            .await
            .clone()
    }

    fn make_service(pool: &Pool) -> UserService {
        UserService::new(pool.clone())
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn create_user_upserts_by_sso_id() {
        let pool = get_or_create_test_pool().await;
        let service = make_service(&pool);
        let sso_id = format!("test-{}", Uuid::new_v4());

        let user = service
            .create_user(
                &sso_id,
                "oidc",
                "jane",
                Some("Jane Doe"),
                Some("jane@example.com"),
            )
            .await
            .expect("create_user");
        assert_eq!(user.username, "jane");
        assert_eq!(user.name.as_deref(), Some("Jane Doe"));
        assert_eq!(user.email.as_deref(), Some("jane@example.com"));
        assert!(!user.admin, "create_user must create a non-admin");

        let updated = service
            .create_user(&sso_id, "oidc", "jane-doe", None, None)
            .await
            .expect("upsert");
        assert_eq!(
            updated.id.0, user.id.0,
            "upsert should keep the same user id"
        );
        assert_eq!(
            updated.username, "jane-doe",
            "second call should update username"
        );
        assert_eq!(updated.name, None, "second call should update name");
        assert!(!updated.admin, "upsert must not change the admin flag");

        let elevated = service.set_admin(user.id, true).await.expect("set_admin");
        assert!(elevated.admin, "set_admin must grant admin access");

        let after_upsert = service
            .create_user(&sso_id, "oidc", "jane-doe", None, None)
            .await
            .expect("upsert after elevation");
        assert!(
            after_upsert.admin,
            "an upsert (e.g. login) must not revoke admin access"
        );

        service
            .delete_user(user.id, false)
            .await
            .expect("delete_user");
        assert!(
            user_repository::find_by_id(&pool, user.id.0)
                .await
                .expect("find_by_id")
                .is_none(),
            "user should be gone after cleanup"
        );
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn delete_user_without_purge_keeps_chats() {
        let pool = get_or_create_test_pool().await;
        let service = make_service(&pool);
        let sso_id = format!("test-{}", Uuid::new_v4());

        let user = service
            .create_user(&sso_id, "oidc", "jane", None, None)
            .await
            .expect("create_user");

        let chat = chat_repository::insert(
            &pool,
            chat_repository::NewChatDb::new(user.id.0, Some("Chat".into())),
        )
        .await
        .expect("insert chat");

        service
            .delete_user(user.id, false)
            .await
            .expect("delete_user");
        assert!(
            user_repository::find_by_id(&pool, user.id.0)
                .await
                .expect("find_by_id")
                .is_none(),
            "user should be gone"
        );
        assert!(
            chat_repository::get_by_id(&pool, ChatId(chat.id), user.id)
                .await
                .expect("get_by_id")
                .is_some(),
            "chat should survive a non-purge delete"
        );

        let _ = chat_repository::delete(&pool, ChatId(chat.id), user.id).await;
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn delete_user_with_purge_removes_chats() {
        let pool = get_or_create_test_pool().await;
        let service = make_service(&pool);
        let sso_id = format!("test-{}", Uuid::new_v4());

        let user = service
            .create_user(&sso_id, "oidc", "jane", None, None)
            .await
            .expect("create_user");

        let chat = chat_repository::insert(
            &pool,
            chat_repository::NewChatDb::new(user.id.0, Some("Chat".into())),
        )
        .await
        .expect("insert chat");

        service
            .delete_user(user.id, true)
            .await
            .expect("delete_user");
        assert!(
            user_repository::find_by_id(&pool, user.id.0)
                .await
                .expect("find_by_id")
                .is_none(),
            "user should be gone"
        );
        assert!(
            chat_repository::get_by_id(&pool, ChatId(chat.id), user.id)
                .await
                .expect("get_by_id")
                .is_none(),
            "chat should be removed with purge"
        );
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn delete_unknown_user_not_found() {
        let pool = get_or_create_test_pool().await;
        let service = make_service(&pool);
        let unknown = UserId(Uuid::new_v4());

        assert!(
            matches!(
                service.delete_user(unknown, false).await,
                Err(AppError::NotFound(_))
            ),
            "deleting an unknown user should be NotFound"
        );
        assert!(
            matches!(
                service.delete_user(unknown, true).await,
                Err(AppError::NotFound(_))
            ),
            "deleting an unknown user with purge should be NotFound"
        );
    }
}
