use chrono::NaiveDateTime;
use diesel::deserialize::Queryable;
use diesel::upsert::excluded;
use diesel::{
    ExpressionMethods, Insertable, OptionalExtension, QueryDsl, RunQueryDsl, Selectable,
    SelectableHelper,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::models::session::Session;
use crate::domain::models::session::SessionId;
use crate::domain::models::user::UserId;
use crate::infra::db::schema::sessions;
use crate::infra::errors::InfraError;

#[derive(Serialize, Queryable, Selectable)]
#[diesel(table_name = sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct SessionDb {
    pub id: Uuid,
    pub user_id: Uuid,
    pub provider_sid: String,
    pub access_token: String,
    pub refresh_token: String,
    pub created_at: NaiveDateTime,
    pub expires_at: NaiveDateTime,
}

#[derive(Deserialize, Insertable)]
#[diesel(table_name = sessions)]
struct UpsertSession {
    provider_sid: String,
    user_id: Uuid,
    access_token: String,
    refresh_token: String,
    expires_at: NaiveDateTime,
}

pub async fn upsert(
    pool: &deadpool_diesel::postgres::Pool,
    provider_sid: &str,
    user_id: Uuid,
    access_token: &str,
    refresh_token: &str,
    expires_at: NaiveDateTime,
) -> Result<SessionDb, InfraError> {
    let sid_owned = provider_sid.to_string();
    let at_owned = access_token.to_string();
    let rt_owned = refresh_token.to_string();

    let conn = pool.get().await.map_err(InfraError::from)?;
    let res = conn
        .interact(move |conn| {
            diesel::insert_into(sessions::table)
                .values(UpsertSession {
                    provider_sid: sid_owned,
                    user_id,
                    access_token: at_owned,
                    refresh_token: rt_owned,
                    expires_at,
                })
                .on_conflict(sessions::provider_sid)
                .do_update()
                .set((
                    sessions::user_id.eq(excluded(sessions::user_id)),
                    sessions::access_token.eq(excluded(sessions::access_token)),
                    sessions::refresh_token.eq(excluded(sessions::refresh_token)),
                    sessions::expires_at.eq(excluded(sessions::expires_at)),
                ))
                .returning(SessionDb::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(InfraError::from)?
        .map_err(InfraError::from)?;

    Ok(res)
}

pub async fn find_by_provider_sid(
    pool: &deadpool_diesel::postgres::Pool,
    provider_sid: &str,
) -> Result<Option<Session>, InfraError> {
    let sid_owned = provider_sid.to_string();
    let conn = pool.get().await.map_err(InfraError::from)?;
    let res = conn
        .interact(move |conn| {
            sessions::table
                .filter(sessions::provider_sid.eq(sid_owned))
                .select(SessionDb::as_select())
                .get_result::<SessionDb>(conn)
                .optional()
        })
        .await
        .map_err(InfraError::from)?
        .map_err(InfraError::from)?;

    Ok(res.map(From::from))
}

pub async fn find_by_access_token(
    pool: &deadpool_diesel::postgres::Pool,
    access_token: &str,
) -> Result<Option<SessionDb>, InfraError> {
    let at_owned = access_token.to_string();
    let conn = pool.get().await.map_err(InfraError::from)?;
    let res = conn
        .interact(move |conn| {
            sessions::table
                .filter(sessions::access_token.eq(at_owned))
                .select(SessionDb::as_select())
                .get_result::<SessionDb>(conn)
                .optional()
        })
        .await
        .map_err(InfraError::from)?
        .map_err(InfraError::from)?;

    Ok(res)
}

pub async fn delete_expired(pool: &deadpool_diesel::postgres::Pool) -> Result<usize, InfraError> {
    let conn = pool.get().await.map_err(InfraError::from)?;
    let res = conn
        .interact(|conn| {
            diesel::delete(sessions::table.filter(sessions::expires_at.lt(diesel::dsl::now)))
                .execute(conn)
        })
        .await
        .map_err(InfraError::from)?
        .map_err(InfraError::from)?;

    Ok(res)
}

impl From<SessionDb> for Session {
    fn from(db: SessionDb) -> Self {
        Session {
            id: SessionId(db.id),
            user_id: UserId(db.user_id),
            provider_sid: db.provider_sid,
            access_token: db.access_token,
            refresh_token: db.refresh_token,
            created_at: db.created_at,
            expires_at: db.expires_at,
        }
    }
}
