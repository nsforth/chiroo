use chrono::NaiveDateTime;
use diesel::deserialize::Queryable;
use diesel::dsl::exists;
use diesel::{
    BoolExpressionMethods, ExpressionMethods, Insertable, OptionalExtension, QueryDsl, RunQueryDsl,
    Selectable, SelectableHelper,
};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

use crate::domain::models::chat::{Chat, ChatId};
use crate::domain::models::user::UserId;
use crate::infra::db::schema::chats::{self, owner};
use crate::infra::errors::{map_infra_error, InfraError};

#[derive(Serialize, Queryable, Selectable)]
#[diesel(table_name = chats)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ChatDb {
    pub id: Uuid,
    pub title: Option<String>,
    pub owner: Uuid,
    pub public: bool,
    pub search_config: Option<serde_json::Value>,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, Deserialize, Insertable)]
#[diesel(table_name = chats)]
pub struct NewChatDb {
    pub title: Option<String>,
    pub owner: Uuid,
    pub search_config: Option<serde_json::Value>,
}

impl NewChatDb {
    pub fn new(
        owner_id: Uuid,
        title: Option<String>,
        search_config: Option<serde_json::Value>,
    ) -> Self {
        NewChatDb {
            owner: owner_id,
            title,
            search_config,
        }
    }
}

#[instrument(level = "debug", skip(pool))]
pub async fn insert(
    pool: &deadpool_diesel::postgres::Pool,
    new_chat: NewChatDb,
) -> Result<ChatDb, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(|conn| {
            diesel::insert_into(chats::table)
                .values(new_chat)
                .returning(ChatDb::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res)
}

#[instrument(level = "debug", skip(pool))]
pub async fn get_all(
    pool: &deadpool_diesel::postgres::Pool,
    user_id: UserId,
) -> Result<Vec<ChatDb>, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(move |conn| {
            let query = chats::table.into_boxed::<diesel::pg::Pg>();
            query
                .select(ChatDb::as_select())
                .filter(owner.eq(user_id.0))
                .order_by(chats::created_at.desc())
                .load::<ChatDb>(conn)
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res)
}

#[instrument(level = "debug", skip(pool))]
pub async fn get_by_id(
    pool: &deadpool_diesel::postgres::Pool,
    id: ChatId,
    user_id: UserId,
) -> Result<Option<ChatDb>, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(move |conn| {
            chats::table
                .find(id.0)
                .select(ChatDb::as_select())
                .filter(owner.eq(user_id.0).or(chats::public.eq(true)))
                .get_result::<ChatDb>(conn)
                .optional()
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res)
}

#[instrument(level = "debug", skip(pool))]
pub async fn delete(
    pool: &deadpool_diesel::postgres::Pool,
    id: ChatId,
    user_id: UserId,
) -> Result<bool, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(move |conn| {
            diesel::delete(chats::table.filter(chats::id.eq(id.0).and(chats::owner.eq(user_id.0))))
                .execute(conn)
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res > 0)
}

#[instrument(level = "debug", skip(pool))]
pub async fn update_title(
    pool: &deadpool_diesel::postgres::Pool,
    id: ChatId,
    user_id: UserId,
    title: String,
) -> Result<ChatDb, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(move |conn| {
            diesel::update(chats::table.filter(chats::id.eq(id.0).and(chats::owner.eq(user_id.0))))
                .set(chats::title.eq(Some(title)))
                .returning(ChatDb::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res)
}

#[instrument(level = "debug", skip(pool))]
pub async fn update_visibility(
    pool: &deadpool_diesel::postgres::Pool,
    id: ChatId,
    user_id: UserId,
    is_public: bool,
) -> Result<ChatDb, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(move |conn| {
            diesel::update(chats::table.filter(chats::id.eq(id.0).and(chats::owner.eq(user_id.0))))
                .set(chats::public.eq(is_public))
                .returning(ChatDb::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res)
}

#[instrument(level = "debug", skip(pool))]
pub async fn is_owner(
    pool: &deadpool_diesel::postgres::Pool,
    chat_id: ChatId,
    user_id: UserId,
) -> Result<bool, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(move |conn| {
            diesel::select(exists(
                chats::table.filter(chats::id.eq(chat_id.0).and(chats::owner.eq(user_id.0))),
            ))
            .get_result::<bool>(conn)
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;
    Ok(res)
}

impl Into<Chat> for ChatDb {
    fn into(self) -> Chat {
        Chat {
            id: ChatId(self.id),
            title: self.title,
            owner: UserId(self.owner),
            public: self.public,
            search_config: self
                .search_config
                .and_then(|v| serde_json::from_value(v).ok()),
            created_at: self.created_at,
        }
    }
}
