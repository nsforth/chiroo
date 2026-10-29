use crate::domain::models::chat::ChatId;
use crate::domain::models::scenario::{Round, Scenario, ScenarioId};
use crate::errors::AppError;
use crate::infra::repositories::scenario_repository;
use crate::operation_result::OperationResult;
use crate::utils::LogView;
use chrono::NaiveDateTime;
use deadpool_diesel::postgres::Pool;
use tracing::{info, instrument};

#[derive(Clone)]
pub struct ScenarioService {
    pool: Pool,
}

impl ScenarioService {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    #[instrument(skip(self))]
    pub async fn get_scenarios_for_chat(&self, chat_id: ChatId) -> Result<Vec<Scenario>, AppError> {
        let result = scenario_repository::get_scenarios_for_chat(&self.pool, chat_id)
            .await
            .map_err(Into::into);
        match &result {
            Ok(s) => info!(count = s.len(), status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    #[instrument(skip(self))]
    pub async fn get_scenario(
        &self,
        chat_id: ChatId,
        scenario_id: ScenarioId,
    ) -> Result<Option<Scenario>, AppError> {
        let result = scenario_repository::get_scenario(&self.pool, chat_id, scenario_id)
            .await
            .map_err(Into::into);
        match &result {
            Ok(r) => info!(found = r.is_some(), status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    #[instrument(skip(self, scenario), fields(
        scenario_id = %scenario.id.0,
        name = %scenario.name,
        status = %scenario.status.as_str(),
        rounds = scenario.rounds.len()
    ))]
    pub async fn insert_scenario(&self, scenario: &Scenario) -> Result<Scenario, AppError> {
        let result = scenario_repository::insert_scenario(&self.pool, scenario)
            .await
            .map_err(Into::into);
        match &result {
            Ok(s) => info!(id = ?s.id, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    #[instrument(skip(self, scenario), fields(
        scenario_id = %scenario.id.0,
        name = %scenario.name,
        status = %scenario.status.as_str(),
        rounds = scenario.rounds.len()
    ))]
    pub async fn update_scenario(&self, scenario: &Scenario) -> Result<Scenario, AppError> {
        let result = scenario_repository::update_scenario(&self.pool, scenario)
            .await
            .map_err(Into::into);
        match &result {
            Ok(s) => info!(id = ?s.id, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    #[instrument(skip(self, round), fields(round = %round.log_view()))]
    pub async fn insert_round(
        &self,
        scenario_id: ScenarioId,
        idx: i32,
        round: &Round,
        expected_updated_at: NaiveDateTime,
    ) -> Result<NaiveDateTime, AppError> {
        let result = scenario_repository::insert_round(
            &self.pool,
            scenario_id,
            idx,
            round,
            expected_updated_at,
        )
        .await
        .map_err(Into::into);
        match &result {
            Ok(updated_at) => info!(updated_at = %updated_at, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    #[instrument(skip(self))]
    pub async fn delete_scenario(&self, scenario_id: ScenarioId) -> Result<bool, AppError> {
        let result = scenario_repository::delete_scenario(&self.pool, scenario_id)
            .await
            .map_err(Into::into);
        match &result {
            Ok(deleted) => info!(deleted = deleted, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    #[instrument(skip(self))]
    pub async fn delete_scenarios_after(
        &self,
        chat_id: ChatId,
        created_at: NaiveDateTime,
    ) -> Result<usize, AppError> {
        let result = scenario_repository::delete_scenarios_after(&self.pool, chat_id, created_at)
            .await
            .map_err(Into::into);
        match &result {
            Ok(count) => info!(deleted = count, status = result.status()),
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
    use crate::domain::models::chat::{Chat, ChatId};
    use crate::domain::models::messages::{Message, MessagePart, Role, TextBlock};
    use crate::domain::models::scenario::ScenarioStatus;
    use crate::infra::repositories::chat_repository::{self, NewChatDb};

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

    fn make_service(pool: &Pool) -> ScenarioService {
        ScenarioService::new(pool.clone())
    }

    /// Create a bare chat row to hang scenarios off.
    async fn make_chat(pool: &Pool) -> Chat {
        let chat_db = chat_repository::insert(&pool, NewChatDb::new(Uuid::new_v4(), None))
            .await
            .expect("insert chat");
        chat_db.into()
    }

    fn make_scenario(chat_id: ChatId, question: &str) -> Scenario {
        Scenario::new(
            chat_id,
            "test-model".to_string(),
            ScenarioStatus::Pending,
            Message::user(chat_id, question.to_string()),
        )
    }

    fn assistant_round(chat_id: ChatId, text: &str) -> Round {
        let mut msg = Message::new(chat_id, Role::Assistant);
        msg.parts.push(MessagePart::Text(TextBlock {
            text: text.to_string(),
        }));
        Round {
            message: msg,
            sources: Vec::new(),
            hidden: false,
            usage: None,
        }
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn insert_scenario_roundtrip() {
        let pool = get_or_create_test_pool().await;
        let service = make_service(&pool);
        let chat = make_chat(&pool).await;

        let scenario = service
            .insert_scenario(&make_scenario(chat.id, "first question?"))
            .await
            .expect("insert_scenario");

        let got = service
            .get_scenario(chat.id, scenario.id)
            .await
            .expect("get_scenario")
            .expect("scenario should exist");
        assert_eq!(got.id.0, scenario.id.0);
        assert_eq!(got.chat_id.0, chat.id.0);
        assert_eq!(got.name, "test-model");
        assert_eq!(got.rounds.len(), 1);
        assert_eq!(got.rounds[0].message.text(), "first question?");

        let all = service
            .get_scenarios_for_chat(chat.id)
            .await
            .expect("get_scenarios_for_chat");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id.0, scenario.id.0);

        let deleted = service
            .delete_scenario(scenario.id)
            .await
            .expect("delete_scenario");
        assert!(deleted, "deleting an existing scenario should report true");
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn get_scenarios_for_chat_orders_by_created_at() {
        let pool = get_or_create_test_pool().await;
        let service = make_service(&pool);
        let chat = make_chat(&pool).await;

        let mut ids = Vec::new();
        for q in ["one", "two", "three"] {
            let s = service
                .insert_scenario(&make_scenario(chat.id, q))
                .await
                .expect("insert_scenario");
            ids.push(s.id.0);
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let all = service
            .get_scenarios_for_chat(chat.id)
            .await
            .expect("get_scenarios_for_chat");
        let got_ids: Vec<Uuid> = all.iter().map(|s| s.id.0).collect();
        assert_eq!(
            got_ids, ids,
            "scenarios must be ordered by created_at ascending"
        );

        for id in &ids {
            let id = ScenarioId(*id);
            service.delete_scenario(id).await.expect("delete_scenario");
        }
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn insert_round_and_update_scenario_persist() {
        let pool = get_or_create_test_pool().await;
        let service = make_service(&pool);
        let chat = make_chat(&pool).await;

        let mut scenario = service
            .insert_scenario(&make_scenario(chat.id, "question"))
            .await
            .expect("insert_scenario");

        let round = assistant_round(chat.id, "answer one");
        let updated_at = service
            .insert_round(scenario.id, 1, &round.clone(), scenario.updated_at)
            .await
            .expect("insert_round");
        scenario.rounds.push(round);

        let got = service
            .get_scenario(chat.id, scenario.id)
            .await
            .expect("get_scenario")
            .expect("scenario should exist");
        assert_eq!(got.rounds.len(), 2);
        assert_eq!(got.rounds[1].message.text(), "answer one");
        assert_eq!(got.updated_at, updated_at);

        let mut answer_round = assistant_round(chat.id, "final answer");
        answer_round.hidden = false;
        scenario.rounds.push(answer_round);
        scenario.status = ScenarioStatus::Completed;
        let updated = service
            .update_scenario(&scenario)
            .await
            .expect("update_scenario");
        assert_eq!(updated.rounds.len(), 3);
        assert!(updated.is_complete());

        let got = service
            .get_scenario(chat.id, scenario.id)
            .await
            .expect("get_scenario")
            .expect("scenario should exist");
        assert_eq!(got.rounds.len(), 3, "full rewrite must persist all rounds");
        assert_eq!(got.rounds[2].message.text(), "final answer");

        service
            .delete_scenario(scenario.id)
            .await
            .expect("delete_scenario");
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn delete_scenario_cascades_rounds_and_is_idempotent() {
        let pool = get_or_create_test_pool().await;
        let service = make_service(&pool);
        let chat = make_chat(&pool).await;

        let scenario = service
            .insert_scenario(&make_scenario(chat.id, "question"))
            .await
            .expect("insert_scenario");
        service
            .insert_round(
                scenario.id,
                1,
                &assistant_round(chat.id, "answer"),
                scenario.updated_at,
            )
            .await
            .expect("insert_round");

        assert!(
            service
                .delete_scenario(scenario.id)
                .await
                .expect("delete_scenario"),
            "first delete should report true"
        );
        let missing = service
            .get_scenario(chat.id, scenario.id)
            .await
            .expect("get_scenario");
        assert!(
            missing.is_none(),
            "deleted scenario must be gone with its rounds"
        );
        assert!(
            !service
                .delete_scenario(scenario.id)
                .await
                .expect("delete_scenario"),
            "second delete should report false"
        );
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn delete_scenarios_after_removes_only_later_scenarios() {
        let pool = get_or_create_test_pool().await;
        let service = make_service(&pool);
        let chat = make_chat(&pool).await;

        let a = service
            .insert_scenario(&make_scenario(chat.id, "a"))
            .await
            .expect("insert a");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let b = service
            .insert_scenario(&make_scenario(chat.id, "b"))
            .await
            .expect("insert b");
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        service
            .insert_scenario(&make_scenario(chat.id, "c"))
            .await
            .expect("insert c");

        let deleted = service
            .delete_scenarios_after(chat.id, b.created_at)
            .await
            .expect("delete_scenarios_after");
        assert_eq!(deleted, 1, "only the scenario after `b` should be deleted");

        let all = service
            .get_scenarios_for_chat(chat.id)
            .await
            .expect("get_scenarios_for_chat");
        let ids: Vec<Uuid> = all.iter().map(|s| s.id.0).collect();
        assert_eq!(
            ids,
            vec![a.id.0, b.id.0],
            "a and b must survive, c must be gone"
        );

        for id in [a.id, b.id] {
            service.delete_scenario(id).await.expect("delete_scenario");
        }
    }
}
