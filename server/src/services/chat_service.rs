use crate::domain::models::chat::{Chat, ChatId};
use crate::domain::models::messages::Message;
use crate::domain::models::scenario::{Scenario, ScenarioStatus};
use crate::domain::models::user::UserId;
use crate::errors::AppError;
use crate::infra::repositories::chat_repository;
use crate::operation_result::OperationResult;
use crate::services::scenario_service::ScenarioService;
use deadpool_diesel::postgres::Pool;
use tracing::{info, instrument};

#[derive(Clone)]
pub struct ChatService {
    pool: Pool,
    scenario_service: ScenarioService,
}

impl ChatService {
    pub fn new(pool: Pool, scenario_service: ScenarioService) -> Self {
        Self {
            pool,
            scenario_service,
        }
    }

    /// Get all chats owned by the user.
    #[instrument(skip(self))]
    pub async fn get_chats(&self, user_id: UserId) -> Result<Vec<Chat>, AppError> {
        let r = chat_repository::get_all(&self.pool, user_id).await;
        let result = match r {
            Ok(chats) => Ok(chats.into_iter().map(Into::into).collect::<Vec<Chat>>()),
            Err(_) => Err(AppError::InternalServerError),
        };
        match &result {
            Ok(chats) => info!(chats_len = chats.len(), status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    /// Get a single chat by ID.
    #[instrument(skip(self))]
    pub async fn get_chat(&self, chat_id: ChatId, user_id: UserId) -> Result<Chat, AppError> {
        let result: Result<Chat, AppError> =
            match chat_repository::get_by_id(&self.pool, chat_id, user_id).await {
                Ok(Some(chat)) => Ok(chat.into()),
                Ok(None) => Err(AppError::NotFound("Chat not found".into())),
                Err(_) => Err(AppError::InternalServerError),
            };
        match &result {
            Ok(_) => info!(status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    /// Create a new chat with an optional first message.
    /// The chat title is derived from the first message (first 15 chars + "...").
    #[instrument(skip(self))]
    pub async fn create_chat(
        &self,
        first_message: Option<String>,
        model: String,
        user_id: UserId,
    ) -> Result<Chat, AppError> {
        let title = first_message.as_deref().map(Message::make_title);
        let new_chat = chat_repository::NewChatDb::new(user_id.0, title.clone());
        let result: Result<Chat, AppError> = chat_repository::insert(&self.pool, new_chat)
            .await
            .map(Into::into)
            .map_err(|_| AppError::InternalServerError);
        let result = match result {
            Ok(chat) => {
                if let Some(text) = first_message {
                    let user_message = Message::user(chat.id, text);
                    let scenario =
                        Scenario::new(chat.id, model, ScenarioStatus::Pending, user_message);
                    let _ = self.scenario_service.insert_scenario(&scenario).await;
                }
                Ok(chat)
            },
            Err(e) => Err(e),
        };
        match &result {
            Ok(chat) => info!(title = ?chat.title, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    /// Create a chat whose title derives from `first_message` but WITHOUT inserting a
    /// scenario. The OpenAI-compatible API starts its own scenario via `AiService::complete`.
    #[instrument(skip(self, first_message))]
    pub async fn create_chat_shell(
        &self,
        first_message: String,
        user_id: UserId,
    ) -> Result<Chat, AppError> {
        let title = Message::make_title(&first_message);
        let new_chat = chat_repository::NewChatDb::new(user_id.0, Some(title.clone()));
        let result: Result<Chat, AppError> = chat_repository::insert(&self.pool, new_chat)
            .await
            .map(Into::into)
            .map_err(|_| AppError::InternalServerError);
        match &result {
            Ok(chat) => info!(title = ?chat.title, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    /// Delete a chat by ID (only owner can delete).
    #[instrument(skip(self))]
    pub async fn delete_chat(&self, chat_id: &ChatId, user_id: &UserId) -> Result<(), AppError> {
        let result = match chat_repository::delete(&self.pool, *chat_id, *user_id).await {
            Ok(false) => Err(AppError::NotFound("Chat not found".into())),
            Ok(true) => Ok(()),
            Err(_) => Err(AppError::InternalServerError),
        };
        match &result {
            Ok(()) => info!(status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    /// Update chat title.
    #[instrument(skip(self))]
    pub async fn update_title(
        &self,
        chat_id: &ChatId,
        user_id: &UserId,
        title: String,
    ) -> Result<Chat, AppError> {
        let result: Result<Chat, AppError> = match chat_repository::update_title(
            &self.pool,
            *chat_id,
            *user_id,
            title.clone(),
        )
        .await
        {
            Ok(chat) => Ok(chat.into()),
            Err(crate::infra::errors::InfraError::NotFound) => {
                Err(AppError::NotFound("Chat not found".into()))
            },
            Err(_) => Err(AppError::InternalServerError),
        };
        match &result {
            Ok(chat) => info!(title = ?chat.title, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    /// Verify that the user owns the chat. Returns `NotFound` if not.
    #[instrument(skip(self))]
    pub async fn check_ownership(&self, chat_id: ChatId, user_id: UserId) -> Result<(), AppError> {
        let result = match chat_repository::is_owner(&self.pool, chat_id, user_id).await {
            Ok(true) => Ok(()),
            Ok(false) => Err(AppError::NotFound("Chat not found".into())),
            Err(_) => Err(AppError::InternalServerError),
        };
        match &result {
            Ok(()) => info!(is_owner = true, status = result.status()),
            Err(e) => info!(
                error = e.to_string(),
                is_owner = false,
                status = result.status()
            ),
        }
        result
    }

    /// Update chat visibility (public/private).
    #[instrument(skip(self))]
    pub async fn update_visibility(
        &self,
        chat_id: &ChatId,
        user_id: &UserId,
        is_public: bool,
    ) -> Result<Chat, AppError> {
        let result: Result<Chat, AppError> =
            match chat_repository::update_visibility(&self.pool, *chat_id, *user_id, is_public)
                .await
            {
                Ok(chat) => Ok(chat.into()),
                Err(crate::infra::errors::InfraError::NotFound) => {
                    Err(AppError::NotFound("Chat not found".into()))
                },
                Err(_) => Err(AppError::InternalServerError),
            };
        match &result {
            Ok(chat) => info!(is_public = chat.public, status = result.status()),
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

    fn make_service(pool: &Pool) -> ChatService {
        let scenario_service = ScenarioService::new(pool.clone());
        ChatService::new(pool.clone(), scenario_service)
    }

    fn make_user() -> UserId {
        UserId(Uuid::new_v4())
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn create_chat_with_first_message_creates_pending_scenario() {
        let pool = get_or_create_test_pool().await;
        let service = make_service(&pool);
        let user = make_user();

        let chat = service
            .create_chat(Some("Hello there!".into()), "test-model".into(), user)
            .await
            .expect("create_chat");

        assert_eq!(chat.owner, user);
        assert_eq!(chat.title.as_deref(), Some("Hello there!"));

        let got = service.get_chat(chat.id, user).await.expect("get_chat");
        assert_eq!(got.id.0, chat.id.0);

        let scenarios = service
            .scenario_service
            .get_scenarios_for_chat(chat.id)
            .await
            .expect("get_scenarios_for_chat");
        assert_eq!(
            scenarios.len(),
            1,
            "first message should create one scenario"
        );
        let scenario = &scenarios[0];
        assert_eq!(scenario.name, "test-model");
        assert!(
            !scenario.is_complete(),
            "fresh scenario should not be completed"
        );
        let user_msg = scenario
            .user_message()
            .expect("rounds[0] is the user message");
        assert_eq!(user_msg.text(), "Hello there!");

        service
            .delete_chat(&chat.id, &user)
            .await
            .expect("delete_chat");
        assert!(
            matches!(
                service.get_chat(chat.id, user).await,
                Err(AppError::NotFound(_))
            ),
            "chat should be gone after delete"
        );
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn create_chat_truncates_long_title() {
        let pool = get_or_create_test_pool().await;
        let service = make_service(&pool);
        let user = make_user();

        let chat = service
            .create_chat(
                Some("A much longer first message than fifteen characters".into()),
                "test-model".into(),
                user,
            )
            .await
            .expect("create_chat");

        assert_eq!(
            chat.title.as_deref(),
            Some("A much longer f..."),
            "title should be the first 15 chars plus ellipsis"
        );

        service
            .delete_chat(&chat.id, &user)
            .await
            .expect("delete_chat");
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn create_chat_without_first_message_has_no_scenario() {
        let pool = get_or_create_test_pool().await;
        let service = make_service(&pool);
        let user = make_user();

        let chat = service
            .create_chat(None, "test-model".into(), user)
            .await
            .expect("create_chat");
        assert_eq!(chat.title, None);

        let scenarios = service
            .scenario_service
            .get_scenarios_for_chat(chat.id)
            .await
            .expect("get_scenarios_for_chat");
        assert!(scenarios.is_empty(), "no scenario without a first message");

        service
            .delete_chat(&chat.id, &user)
            .await
            .expect("delete_chat");
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn ownership_checks_reject_non_owner() {
        let pool = get_or_create_test_pool().await;
        let service = make_service(&pool);
        let owner = make_user();
        let intruder = make_user();

        let chat = service
            .create_chat(None, "test-model".into(), owner)
            .await
            .expect("create_chat");

        assert!(
            matches!(
                service.check_ownership(chat.id, intruder).await,
                Err(AppError::NotFound(_))
            ),
            "non-owner should not pass ownership check"
        );
        assert!(
            matches!(
                service.get_chat(chat.id, intruder).await,
                Err(AppError::NotFound(_))
            ),
            "non-owner should not see the chat"
        );
        assert!(
            service.delete_chat(&chat.id, &intruder).await.is_err(),
            "non-owner should not delete the chat"
        );

        service
            .delete_chat(&chat.id, &owner)
            .await
            .expect("delete_chat");
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn get_chats_returns_only_own_chats() {
        let pool = get_or_create_test_pool().await;
        let service = make_service(&pool);
        let alice = make_user();
        let bob = make_user();

        let alice_chat = service
            .create_chat(None, "test-model".into(), alice)
            .await
            .expect("create_chat");
        service
            .create_chat(None, "test-model".into(), bob)
            .await
            .expect("create_chat");

        let chats = service.get_chats(alice).await.expect("get_chats");
        assert_eq!(
            chats.len(),
            1,
            "each user should see exactly their own chat"
        );
        assert_eq!(chats[0].id.0, alice_chat.id.0);

        service
            .delete_chat(&alice_chat.id, &alice)
            .await
            .expect("delete_chat");
    }
}
