use chrono::NaiveDateTime;
use diesel::deserialize::Queryable;
use diesel::upsert::excluded;
use diesel::{
    ExpressionMethods, Insertable, OptionalExtension, QueryDsl, RunQueryDsl, Selectable,
    SelectableHelper,
};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

use crate::domain::models::user::User;
use crate::domain::models::user::UserId;
use crate::infra::db::schema::users;
use crate::infra::errors::{map_infra_error, InfraError};

#[derive(Serialize, Queryable, Selectable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UserDb {
    pub id: Uuid,
    pub sso_id: String,
    pub sso_provider: String,
    pub name: Option<String>,
    pub username: String,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub admin: bool,
    pub created_at: NaiveDateTime,
}

#[derive(Deserialize, Insertable)]
#[diesel(table_name = users)]
struct UpsertUser {
    sso_id: String,
    sso_provider: String,
    name: Option<String>,
    username: String,
    email: Option<String>,
    avatar_url: Option<String>,
}

// TODO: Change to save and User model from caller
#[instrument(level = "debug", skip(pool, avatar_url))]
pub async fn upsert(
    pool: &deadpool_diesel::postgres::Pool,
    sso_id: &str,
    sso_provider: &str,
    name: Option<&str>,
    username: &str,
    email: Option<&str>,
    avatar_url: Option<&str>,
) -> Result<UserDb, InfraError> {
    let sso_id_owned = sso_id.to_string();
    let sso_provider_owned = sso_provider.to_string();
    let name_owned = name.map(|s| s.to_string());
    let username_owned = username.to_string();
    let email_owned = email.map(|s| s.to_string());
    let avatar_url_owned = avatar_url.map(|s| s.to_string());

    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(move |conn| {
            diesel::insert_into(users::table)
                .values(UpsertUser {
                    sso_id: sso_id_owned,
                    sso_provider: sso_provider_owned,
                    name: name_owned,
                    username: username_owned,
                    email: email_owned,
                    avatar_url: avatar_url_owned,
                })
                .on_conflict(users::sso_id)
                .do_update()
                .set((
                    users::name.eq(excluded(users::name)),
                    users::username.eq(excluded(users::username)),
                    users::email.eq(excluded(users::email)),
                    users::avatar_url.eq(excluded(users::avatar_url)),
                ))
                .returning(UserDb::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res)
}

#[instrument(level = "debug", skip(pool))]
pub async fn find_by_id(
    pool: &deadpool_diesel::postgres::Pool,
    id: Uuid,
) -> Result<Option<UserDb>, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(move |conn| {
            users::table
                .filter(users::id.eq(id))
                .select(UserDb::as_select())
                .get_result::<UserDb>(conn)
                .optional()
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res)
}

#[instrument(level = "debug", skip(pool))]
pub async fn find_by_sso_id(
    pool: &deadpool_diesel::postgres::Pool,
    sso_id: &str,
) -> Result<Option<UserDb>, InfraError> {
    let sso_id_owned = sso_id.to_string();
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(move |conn| {
            users::table
                .filter(users::sso_id.eq(sso_id_owned))
                .select(UserDb::as_select())
                .get_result::<UserDb>(conn)
                .optional()
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res)
}

#[instrument(level = "debug", skip(pool))]
pub async fn find_by_username(
    pool: &deadpool_diesel::postgres::Pool,
    username: &str,
) -> Result<Option<UserDb>, InfraError> {
    let username_owned = username.to_string();
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(move |conn| {
            users::table
                .filter(users::username.eq(username_owned))
                .select(UserDb::as_select())
                .first(conn)
                .optional()
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res)
}

#[instrument(level = "debug", skip(pool))]
pub async fn set_admin(
    pool: &deadpool_diesel::postgres::Pool,
    id: Uuid,
    admin: bool,
) -> Result<Option<UserDb>, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(move |conn| {
            diesel::update(users::table)
                .filter(users::id.eq(id))
                .set(users::admin.eq(admin))
                .returning(UserDb::as_returning())
                .get_result::<UserDb>(conn)
                .optional()
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res)
}

impl From<UserDb> for User {
    fn from(db: UserDb) -> Self {
        User {
            id: UserId(db.id),
            sso_id: db.sso_id,
            sso_provider: db.sso_provider,
            name: db.name,
            username: db.username,
            email: db.email,
            avatar_url: db.avatar_url,
            admin: db.admin,
            created_at: db.created_at,
        }
    }
}
