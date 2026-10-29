use std::collections::HashMap;

use chrono::NaiveDateTime;
use diesel::deserialize::Queryable;
use diesel::{
    BoolExpressionMethods, Connection, ExpressionMethods, Insertable, OptionalExtension,
    PgConnection, QueryDsl, RunQueryDsl, Selectable, SelectableHelper,
};
use serde::de::DeserializeOwned;
use tracing::{debug, instrument};
use uuid::Uuid;

use crate::domain::models::chat::ChatId;
use crate::domain::models::messages::Message;
use crate::domain::models::scenario::{Round, Scenario, ScenarioId, ScenarioStatus};
use crate::domain::models::source::Source;
use crate::domain::models::token_usage::TokenUsage;
use crate::infra::db::schema::{rounds, scenarios};
use crate::infra::errors::{map_infra_error, InfraError};

#[derive(Queryable, Selectable)]
#[diesel(table_name = scenarios)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ScenarioDb {
    pub id: Uuid,
    pub chat_id: Uuid,
    pub name: String,
    pub status: String,
    pub token_usage: Option<serde_json::Value>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = rounds)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct RoundDb {
    pub id: Uuid,
    pub scenario_id: Uuid,
    pub idx: i32,
    pub message: serde_json::Value,
    pub sources: serde_json::Value,
    pub hidden: bool,
    pub token_usage: Option<serde_json::Value>,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = scenarios)]
pub struct NewScenarioDb {
    pub id: Uuid,
    pub chat_id: Uuid,
    pub name: String,
    pub status: String,
    pub token_usage: Option<serde_json::Value>,
}

#[derive(Insertable)]
#[diesel(table_name = rounds)]
pub struct NewRoundDb {
    pub id: Uuid,
    pub scenario_id: Uuid,
    pub idx: i32,
    pub message: serde_json::Value,
    pub sources: serde_json::Value,
    pub hidden: bool,
    pub token_usage: Option<serde_json::Value>,
}

fn map_json_error(e: serde_json::Error) -> InfraError {
    debug!(error = e.to_string(), status = "failed");
    InfraError::InternalServerError
}

fn parse_json<T>(value: serde_json::Value) -> Result<T, InfraError>
where
    T: DeserializeOwned,
{
    serde_json::from_value(value).map_err(map_json_error)
}

fn option_usage_json(usage: &Option<TokenUsage>) -> Result<Option<serde_json::Value>, InfraError> {
    usage
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(map_json_error)
}

fn round_to_db(scenario_id: Uuid, idx: i32, round: &Round) -> Result<NewRoundDb, InfraError> {
    Ok(NewRoundDb {
        id: round.message.id.0,
        scenario_id,
        idx,
        message: serde_json::to_value(&round.message).map_err(map_json_error)?,
        sources: serde_json::to_value(&round.sources).map_err(map_json_error)?,
        hidden: round.hidden,
        token_usage: option_usage_json(&round.usage)?,
    })
}

fn rounds_from_domain(scenario_id: Uuid, rounds: &[Round]) -> Result<Vec<NewRoundDb>, InfraError> {
    rounds
        .iter()
        .enumerate()
        .map(|(idx, round)| round_to_db(scenario_id, idx as i32, round))
        .collect()
}

fn parse_usage(value: &Option<serde_json::Value>) -> Result<Option<TokenUsage>, InfraError> {
    value
        .as_ref()
        .map(|v| parse_json::<TokenUsage>(v.clone()))
        .transpose()
}

fn scenario_from_db(scenario: ScenarioDb, rounds: Vec<RoundDb>) -> Result<Scenario, InfraError> {
    let rounds = rounds
        .into_iter()
        .map(|round| {
            Ok(Round {
                message: parse_json::<Message>(round.message)?,
                sources: parse_json::<Vec<Source>>(round.sources)?,
                hidden: round.hidden,
                usage: parse_usage(&round.token_usage)?,
            })
        })
        .collect::<Result<Vec<Round>, InfraError>>()?;

    Ok(Scenario {
        id: ScenarioId(scenario.id),
        chat_id: ChatId(scenario.chat_id),
        name: scenario.name,
        status: ScenarioStatus::from_str(&scenario.status),
        usage: parse_usage(&scenario.token_usage)?,
        rounds,
        created_at: scenario.created_at,
        updated_at: scenario.updated_at,
    })
}

fn load_scenario(
    conn: &mut PgConnection,
    chat_id: ChatId,
    scenario_id: Uuid,
) -> Result<Option<Scenario>, InfraError> {
    let scenario: Option<ScenarioDb> = scenarios::table
        .find(scenario_id)
        .filter(scenarios::chat_id.eq(chat_id.0))
        .select(ScenarioDb::as_select())
        .get_result(conn)
        .optional()
        .map_err(map_infra_error)?;

    let Some(scenario) = scenario else {
        return Ok(None);
    };

    let rounds = rounds::table
        .filter(rounds::scenario_id.eq(scenario_id))
        .order_by(rounds::idx.asc())
        .select(RoundDb::as_select())
        .load::<RoundDb>(conn)
        .map_err(map_infra_error)?;

    Ok(Some(scenario_from_db(scenario, rounds)?))
}

#[instrument(level = "debug", skip(pool))]
pub async fn get_scenarios_for_chat(
    pool: &deadpool_diesel::postgres::Pool,
    chat_id: ChatId,
) -> Result<Vec<Scenario>, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let (scenarios_db, rounds_db) = conn
        .interact(
            move |conn| -> diesel::QueryResult<(Vec<ScenarioDb>, Vec<RoundDb>)> {
                let scenarios_list = scenarios::table
                    .filter(scenarios::chat_id.eq(chat_id.0))
                    .order_by(scenarios::created_at.asc())
                    .select(ScenarioDb::as_select())
                    .load::<ScenarioDb>(conn)?;

                if scenarios_list.is_empty() {
                    return Ok((scenarios_list, Vec::new()));
                }

                let ids: Vec<Uuid> = scenarios_list.iter().map(|s| s.id).collect();
                let rounds_list = rounds::table
                    .filter(rounds::scenario_id.eq_any(ids))
                    .order_by((rounds::scenario_id.asc(), rounds::idx.asc()))
                    .select(RoundDb::as_select())
                    .load::<RoundDb>(conn)?;

                Ok((scenarios_list, rounds_list))
            },
        )
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    let mut rounds_by_scenario: HashMap<Uuid, Vec<RoundDb>> = HashMap::new();
    for round in rounds_db {
        rounds_by_scenario
            .entry(round.scenario_id)
            .or_default()
            .push(round);
    }

    scenarios_db
        .into_iter()
        .map(|scenario| {
            let rounds = rounds_by_scenario.remove(&scenario.id).unwrap_or_default();
            scenario_from_db(scenario, rounds)
        })
        .collect()
}

#[instrument(level = "debug", skip(pool))]
pub async fn get_scenario(
    pool: &deadpool_diesel::postgres::Pool,
    chat_id: ChatId,
    scenario_id: ScenarioId,
) -> Result<Option<Scenario>, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    conn.interact(move |conn| load_scenario(conn, chat_id, scenario_id.0))
        .await
        .map_err(map_infra_error)?
}

#[instrument(level = "debug", skip(pool, scenario))]
pub async fn insert_scenario(
    pool: &deadpool_diesel::postgres::Pool,
    scenario: &Scenario,
) -> Result<Scenario, InfraError> {
    let new_scenario = NewScenarioDb {
        id: scenario.id.0,
        chat_id: scenario.chat_id.0,
        name: scenario.name.clone(),
        status: scenario.status.as_str().to_string(),
        token_usage: option_usage_json(&scenario.usage)?,
    };
    let new_rounds = rounds_from_domain(scenario.id.0, &scenario.rounds)?;
    let chat_id = scenario.chat_id;
    let scenario_id = scenario.id.0;

    let conn = pool.get().await.map_err(map_infra_error)?;
    let result = conn
        .interact(move |conn| {
            conn.transaction(|conn| {
                diesel::insert_into(scenarios::table)
                    .values(&new_scenario)
                    .execute(conn)
                    .map_err(map_infra_error)?;

                if !new_rounds.is_empty() {
                    diesel::insert_into(rounds::table)
                        .values(&new_rounds)
                        .execute(conn)
                        .map_err(map_infra_error)?;
                }

                load_scenario(conn, chat_id, scenario_id)
            })
        })
        .await
        .map_err(map_infra_error)??;

    match result {
        Some(scenario) => Ok(scenario),
        None => Err(InfraError::InternalServerError),
    }
}

#[instrument(level = "debug", skip(pool, scenario))]
pub async fn update_scenario(
    pool: &deadpool_diesel::postgres::Pool,
    scenario: &Scenario,
) -> Result<Scenario, InfraError> {
    let id = scenario.id.0;
    let name = scenario.name.clone();
    let status = scenario.status.as_str().to_string();
    let token_usage = option_usage_json(&scenario.usage)?;
    let chat_id = scenario.chat_id;
    let new_rounds = rounds_from_domain(id, &scenario.rounds)?;

    let conn = pool.get().await.map_err(map_infra_error)?;
    let result = conn
        .interact(move |conn| {
            conn.transaction(|conn| {
                diesel::update(scenarios::table.find(id))
                    .set((
                        scenarios::name.eq(name),
                        scenarios::status.eq(status),
                        scenarios::token_usage.eq(token_usage),
                        scenarios::updated_at.eq(diesel::dsl::now),
                    ))
                    .execute(conn)
                    .map_err(map_infra_error)?;

                diesel::delete(rounds::table.filter(rounds::scenario_id.eq(id)))
                    .execute(conn)
                    .map_err(map_infra_error)?;

                if !new_rounds.is_empty() {
                    diesel::insert_into(rounds::table)
                        .values(&new_rounds)
                        .execute(conn)
                        .map_err(map_infra_error)?;
                }

                load_scenario(conn, chat_id, id)
            })
        })
        .await
        .map_err(map_infra_error)??;

    match result {
        Some(scenario) => Ok(scenario),
        None => Err(InfraError::InternalServerError),
    }
}

#[instrument(level = "debug", skip(pool, round))]
pub async fn insert_round(
    pool: &deadpool_diesel::postgres::Pool,
    scenario_id: ScenarioId,
    idx: i32,
    round: &Round,
    expected_updated_at: NaiveDateTime,
) -> Result<NaiveDateTime, InfraError> {
    let new_round = round_to_db(scenario_id.0, idx, round)?;
    let conn = pool.get().await.map_err(map_infra_error)?;
    let result = conn
        .interact(move |conn| {
            conn.transaction(|conn| {
                // Optimistic lock: only proceed if the scenario hasn't been modified
                // since we loaded it. A concurrent tab / regenerate would have bumped
                // updated_at (or deleted the row), so the UPDATE affects 0 rows.
                let new_updated_at: Option<NaiveDateTime> = diesel::update(
                    scenarios::table.filter(
                        scenarios::id
                            .eq(scenario_id.0)
                            .and(scenarios::updated_at.eq(expected_updated_at)),
                    ),
                )
                .set(scenarios::updated_at.eq(diesel::dsl::now))
                .returning(scenarios::updated_at)
                .get_result(conn)
                .optional()
                .map_err(map_infra_error)?;

                let Some(new_updated_at) = new_updated_at else {
                    return Err(InfraError::Conflict);
                };

                diesel::insert_into(rounds::table)
                    .values(new_round)
                    .execute(conn)
                    .map_err(map_infra_error)?;

                Ok(new_updated_at)
            })
        })
        .await
        .map_err(map_infra_error)??;

    Ok(result)
}

#[instrument(level = "debug", skip(pool))]
pub async fn delete_scenario(
    pool: &deadpool_diesel::postgres::Pool,
    scenario_id: ScenarioId,
) -> Result<bool, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(move |conn| {
            diesel::delete(scenarios::table.filter(scenarios::id.eq(scenario_id.0))).execute(conn)
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res > 0)
}

#[instrument(level = "debug", skip(pool))]
pub async fn delete_scenarios_after(
    pool: &deadpool_diesel::postgres::Pool,
    chat_id: ChatId,
    created_at: NaiveDateTime,
) -> Result<usize, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(move |conn| {
            diesel::delete(
                scenarios::table.filter(
                    scenarios::chat_id
                        .eq(chat_id.0)
                        .and(scenarios::created_at.gt(created_at)),
                ),
            )
            .execute(conn)
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res)
}
