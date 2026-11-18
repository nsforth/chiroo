use chrono::NaiveDateTime;
use diesel::deserialize::Queryable;
use diesel::{
    ExpressionMethods, Insertable, JoinOnDsl, QueryDsl, RunQueryDsl, Selectable, SelectableHelper,
};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

use crate::domain::models::chat::ChatId;
use crate::domain::models::scenario::ScenarioId;
use crate::domain::models::user::{User, UserId};
use crate::domain::models::votes::{Vote, VoteType, VoteWithChat};
use crate::infra::db::schema::{chats, scenarios, users, votes};
use crate::infra::errors::{map_infra_error, InfraError};

#[derive(Serialize, Selectable, Queryable, Debug)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(table_name = votes)]
pub struct VoteDb {
    pub chat_id: Uuid,
    pub scenario_id: Uuid,
    pub is_upvoted: bool,
    pub created_at: NaiveDateTime,
}

#[derive(Deserialize, Insertable, Debug)]
#[diesel(table_name = votes)]
pub struct NewVoteDb {
    pub chat_id: Uuid,
    pub scenario_id: Uuid,
    pub is_upvoted: bool,
    pub created_at: NaiveDateTime,
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

#[derive(Queryable, Debug)]
pub struct VoteWithChatDb {
    pub chat_id: Uuid,
    pub scenario_id: Uuid,
    pub is_upvoted: bool,
    pub voted_at: NaiveDateTime,
    pub chat_title: Option<String>,
    pub chat_created_at: NaiveDateTime,
    pub user_id: Uuid,
    pub sso_id: String,
    pub sso_provider: String,
    pub user_name: Option<String>,
    pub username: String,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub admin: bool,
    pub user_created_at: NaiveDateTime,
    pub model: String,
}

#[instrument(level = "debug", skip(pool))]
pub async fn list_all(
    pool: &deadpool_diesel::postgres::Pool,
    limit: i64,
    offset: i64,
) -> Result<(Vec<VoteWithChatDb>, i64), InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(
            move |conn| -> Result<(Vec<VoteWithChatDb>, i64), diesel::result::Error> {
                let rows = votes::table
                    .inner_join(chats::table.on(votes::chat_id.eq(chats::id)))
                    .inner_join(users::table.on(chats::owner.eq(users::id)))
                    .inner_join(scenarios::table.on(votes::scenario_id.eq(scenarios::id)))
                    .select((
                        votes::chat_id,
                        votes::scenario_id,
                        votes::is_upvoted,
                        votes::created_at,
                        chats::title,
                        chats::created_at,
                        users::id,
                        users::sso_id,
                        users::sso_provider,
                        users::name,
                        users::username,
                        users::email,
                        users::avatar_url,
                        users::admin,
                        users::created_at,
                        scenarios::name,
                    ))
                    .order((
                        votes::created_at.desc(),
                        votes::chat_id.desc(),
                        votes::scenario_id.desc(),
                    ))
                    .limit(limit)
                    .offset(offset)
                    .load::<VoteWithChatDb>(conn)?;

                let total: i64 = votes::table.select(diesel::dsl::count_star()).first(conn)?;

                Ok((rows, total))
            },
        )
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res)
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

impl From<VoteWithChatDb> for VoteWithChat {
    fn from(db: VoteWithChatDb) -> Self {
        Self {
            user: User {
                id: UserId(db.user_id),
                sso_id: db.sso_id,
                sso_provider: db.sso_provider,
                name: db.user_name,
                username: db.username,
                email: db.email,
                avatar_url: db.avatar_url,
                admin: db.admin,
                created_at: db.user_created_at,
            },
            chat_id: ChatId(db.chat_id),
            scenario_id: ScenarioId(db.scenario_id),
            is_upvoted: db.is_upvoted,
            voted_at: db.voted_at,
            chat_title: db.chat_title,
            chat_created_at: db.chat_created_at,
            model: db.model,
        }
    }
}
