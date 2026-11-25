use crate::domain::models::chat::ChatId;
use crate::domain::models::scenario::ScenarioId;
use crate::domain::models::votes::{Vote, VoteWithChat};
use crate::errors::AppError;
use crate::infra::repositories::vote_repository::{self, NewVoteDb};
use crate::operation_result::OperationResult;
use deadpool_diesel::postgres::Pool;
use tracing::{info, instrument};

#[derive(Clone)]
pub struct VoteService {
    pool: Pool,
}

impl VoteService {
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    #[instrument(skip(self))]
    pub async fn get_votes_for_chat(&self, chat_id: ChatId) -> Result<Vec<Vote>, AppError> {
        let result: Result<Vec<Vote>, AppError> =
            vote_repository::get_votes_for_chat(&self.pool, chat_id)
                .await
                .map(|votes| votes.iter().map(Into::into).collect())
                .map_err(|_| AppError::InternalServerError);
        match &result {
            Ok(votes) => info!(count = votes.len(), status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    #[instrument(skip(self))]
    pub async fn like(&self, chat_id: ChatId, scenario_id: ScenarioId) -> Result<Vote, AppError> {
        let result: Result<Vote, AppError> = vote_repository::upsert_vote(
            &self.pool,
            NewVoteDb {
                chat_id: chat_id.0,
                scenario_id: scenario_id.0,
                is_upvoted: true,
                created_at: chrono::Utc::now().naive_utc(),
            },
        )
        .await
        .map(Into::into)
        .map_err(|_| AppError::InternalServerError);
        match &result {
            Ok(vote) => info!(vote_type = ?vote.vote_type, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    #[instrument(skip(self))]
    pub async fn dislike(
        &self,
        chat_id: ChatId,
        scenario_id: ScenarioId,
    ) -> Result<Vote, AppError> {
        let result: Result<Vote, AppError> = vote_repository::upsert_vote(
            &self.pool,
            NewVoteDb {
                chat_id: chat_id.0,
                scenario_id: scenario_id.0,
                is_upvoted: false,
                created_at: chrono::Utc::now().naive_utc(),
            },
        )
        .await
        .map(Into::into)
        .map_err(|_| AppError::InternalServerError);
        match &result {
            Ok(vote) => info!(vote_type = ?vote.vote_type, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    #[instrument(skip(self))]
    pub async fn unvote(&self, chat_id: ChatId, scenario_id: ScenarioId) -> Result<bool, AppError> {
        let result = vote_repository::delete_vote(&self.pool, chat_id.0, scenario_id.0)
            .await
            .map_err(|_| AppError::InternalServerError);
        match &result {
            Ok(deleted) => info!(deleted = deleted, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }

    #[instrument(skip(self))]
    pub async fn list_all(
        &self,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<VoteWithChat>, i64), AppError> {
        let page_size = page_size.clamp(1, 100) as i64;
        let offset = (page.saturating_sub(1) as i64) * page_size;
        let result = vote_repository::list_all(&self.pool, page_size, offset)
            .await
            .map(|(rows, total)| {
                let votes = rows
                    .into_iter()
                    .map(VoteWithChat::from)
                    .collect::<Vec<VoteWithChat>>();
                (votes, total)
            })
            .map_err(|_| AppError::InternalServerError);
        match &result {
            Ok((votes, total)) => {
                info!(count = votes.len(), total = total, status = result.status())
            },
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
    use crate::domain::models::messages::Message;
    use crate::domain::models::scenario::{Scenario, ScenarioStatus};
    use crate::domain::models::votes::VoteType;
    use crate::infra::repositories::chat_repository::{self, NewChatDb};
    use crate::infra::repositories::user_repository;
    use crate::services::scenario_service::ScenarioService;

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

    fn make_service(pool: &Pool) -> VoteService {
        VoteService::new(pool.clone())
    }

    /// Create a chat with a scenario to vote on.
    async fn make_chat_with_scenario(pool: &Pool) -> (ChatId, ScenarioId) {
        let chat: crate::domain::models::chat::Chat =
            chat_repository::insert(pool, NewChatDb::new(Uuid::new_v4(), None, None))
                .await
                .expect("insert chat")
                .into();
        let scenario = Scenario::new(
            chat.id,
            "test-model".to_string(),
            ScenarioStatus::Completed,
            Message::user(chat.id, "question".to_string()),
        );
        let scenario = ScenarioService::new(pool.clone())
            .insert_scenario(&scenario)
            .await
            .expect("insert scenario");
        (chat.id, scenario.id)
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn like_then_dislike_upserts_same_vote() {
        let pool = get_or_create_test_pool().await;
        let service = make_service(&pool);
        let (chat_id, scenario_id) = make_chat_with_scenario(&pool).await;

        let like = service.like(chat_id, scenario_id).await.expect("like");
        assert!(matches!(like.vote_type, VoteType::Like));

        let dislike = service
            .dislike(chat_id, scenario_id)
            .await
            .expect("dislike");
        assert!(matches!(dislike.vote_type, VoteType::Dislike));

        let votes = service
            .get_votes_for_chat(chat_id)
            .await
            .expect("get_votes_for_chat");
        assert_eq!(votes.len(), 1, "vote must be upserted, not duplicated");
        assert!(matches!(votes[0].vote_type, VoteType::Dislike));
        assert_eq!(votes[0].chat_id.0, chat_id.0);
        assert_eq!(votes[0].scenario_id.0, scenario_id.0);
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn unvote_removes_vote() {
        let pool = get_or_create_test_pool().await;
        let service = make_service(&pool);
        let (chat_id, scenario_id) = make_chat_with_scenario(&pool).await;

        service.like(chat_id, scenario_id).await.expect("like");

        let deleted = service.unvote(chat_id, scenario_id).await.expect("unvote");
        assert!(deleted, "first unvote should remove the vote");
        assert!(
            service
                .get_votes_for_chat(chat_id)
                .await
                .expect("get_votes_for_chat")
                .is_empty(),
            "votes must be empty after unvote"
        );
        assert!(
            !service
                .unvote(chat_id, scenario_id)
                .await
                .expect("unvote again"),
            "unvote without a vote should report false"
        );
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn votes_are_keyed_by_scenario() {
        let pool = get_or_create_test_pool().await;
        let service = make_service(&pool);
        let chat: crate::domain::models::chat::Chat =
            chat_repository::insert(&pool, NewChatDb::new(Uuid::new_v4(), None, None))
                .await
                .expect("insert chat")
                .into();
        let scenarios_svc = ScenarioService::new(pool.clone());
        let scenario_a = scenarios_svc
            .insert_scenario(&Scenario::new(
                chat.id,
                "test-model".to_string(),
                ScenarioStatus::Completed,
                Message::user(chat.id, "a".to_string()),
            ))
            .await
            .expect("insert scenario a");
        let scenario_b = scenarios_svc
            .insert_scenario(&Scenario::new(
                chat.id,
                "test-model".to_string(),
                ScenarioStatus::Completed,
                Message::user(chat.id, "b".to_string()),
            ))
            .await
            .expect("insert scenario b");

        service
            .dislike(chat.id, scenario_a.id)
            .await
            .expect("dislike a");
        service.like(chat.id, scenario_b.id).await.expect("like b");

        let votes = service
            .get_votes_for_chat(chat.id)
            .await
            .expect("get_votes_for_chat");
        assert_eq!(votes.len(), 2, "votes must be keyed per scenario");
        let vote_a = votes
            .iter()
            .find(|v| v.scenario_id.0 == scenario_a.id.0)
            .expect("vote for scenario A");
        let vote_b = votes
            .iter()
            .find(|v| v.scenario_id.0 == scenario_b.id.0)
            .expect("vote for scenario B");
        assert!(matches!(vote_a.vote_type, VoteType::Dislike));
        assert!(matches!(vote_b.vote_type, VoteType::Like));
    }

    #[tokio::test]
    #[ignore = "requires Postgres"]
    async fn list_all_returns_votes_with_user_and_chat_context() {
        let pool = get_or_create_test_pool().await;
        let service = make_service(&pool);

        let username = format!("alice-admin-{}", Uuid::new_v4());
        let user = user_repository::upsert(
            &pool,
            &format!("admin-test-{}", Uuid::new_v4()),
            "oidc",
            Some("Alice Admin"),
            &username,
            Some("alice@example.com"),
            None,
        )
        .await
        .expect("upsert user");

        let chat: crate::domain::models::chat::Chat =
            chat_repository::insert(&pool, NewChatDb::new(user.id, None, None))
                .await
                .expect("insert chat")
                .into();
        let scenario = ScenarioService::new(pool.clone())
            .insert_scenario(&Scenario::new(
                chat.id,
                "test-model".to_string(),
                ScenarioStatus::Completed,
                Message::user(chat.id, "question".to_string()),
            ))
            .await
            .expect("insert scenario");

        service.like(chat.id, scenario.id).await.expect("like");

        let (votes, total) = service.list_all(1, 10).await.expect("list_all");
        assert!(total >= 1, "total must count at least the new vote");
        let vote = votes
            .iter()
            .find(|v| v.scenario_id.0 == scenario.id.0)
            .expect("the new vote must appear in list_all");
        assert_eq!(vote.chat_id.0, chat.id.0);
        assert_eq!(vote.user.id.0, user.id);
        assert_eq!(vote.user.username, username);
        assert_eq!(vote.user.name.as_deref(), Some("Alice Admin"));
        assert_eq!(vote.user.email.as_deref(), Some("alice@example.com"));
        assert!(vote.is_upvoted);
    }
}
