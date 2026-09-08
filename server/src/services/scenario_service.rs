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
