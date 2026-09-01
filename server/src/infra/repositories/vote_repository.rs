use diesel::deserialize::Queryable;
use diesel::{ExpressionMethods, Insertable, QueryDsl, RunQueryDsl, Selectable, SelectableHelper};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

use crate::domain::models::chat::ChatId;
use crate::domain::models::scenario::ScenarioId;
use crate::domain::models::votes::{Vote, VoteType};
use crate::infra::db::schema::votes;
use crate::infra::errors::{map_infra_error, InfraError};

#[derive(Serialize, Selectable, Queryable, Debug)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(table_name = votes)]
pub struct VoteDb {
    pub chat_id: Uuid,
    pub scenario_id: Uuid,
    pub is_upvoted: bool,
}

#[derive(Deserialize, Insertable, Debug)]
#[diesel(table_name = votes)]
pub struct NewVoteDb {
    pub chat_id: Uuid,
    pub scenario_id: Uuid,
    pub is_upvoted: bool,
}

#[instrument(level = "debug", skip(pool))]
pub async fn get_votes_for_chat(
    pool: &deadpool_diesel::postgres::Pool,
    chat_id: ChatId,
) -> Result<Vec<VoteDb>, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(move |conn| {
            votes::table
                .filter(votes::chat_id.eq(chat_id.0))
                .load::<VoteDb>(conn)
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res)
}

#[instrument(level = "debug", skip(pool))]
pub async fn upsert_vote(
    pool: &deadpool_diesel::postgres::Pool,
    new_vote: NewVoteDb,
) -> Result<VoteDb, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(move |conn| {
            diesel::insert_into(votes::table)
                .values(&new_vote)
                .on_conflict((votes::chat_id, votes::scenario_id))
                .do_update()
                .set(votes::is_upvoted.eq(new_vote.is_upvoted))
                .returning(VoteDb::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res)
}

#[instrument(level = "debug", skip(pool))]
pub async fn delete_vote(
    pool: &deadpool_diesel::postgres::Pool,
    chat_id: Uuid,
    scenario_id: Uuid,
) -> Result<bool, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(move |conn| {
            diesel::delete(
                votes::table
                    .filter(votes::chat_id.eq(chat_id))
                    .filter(votes::scenario_id.eq(scenario_id)),
            )
            .execute(conn)
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res > 0)
}

impl Into<Vote> for &VoteDb {
    fn into(self) -> Vote {
        Vote {
            chat_id: ChatId(self.chat_id),
            scenario_id: ScenarioId(self.scenario_id),
            vote_type: if self.is_upvoted {
                VoteType::Like
            } else {
                VoteType::Dislike
            },
        }
    }
}

impl Into<Vote> for VoteDb {
    fn into(self) -> Vote {
        Into::into(&self)
    }
}
