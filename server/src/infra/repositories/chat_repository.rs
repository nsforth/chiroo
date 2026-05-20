use chrono::NaiveDateTime;
use diesel::{
    ExpressionMethods, Insertable, PgTextExpressionMethods, QueryDsl, RunQueryDsl, Selectable,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::models::chat::{Chat, ChatId};
use crate::domain::models::user::UserId;
use crate::infra::db::schema::chats;
use crate::infra::errors::InfraError;

#[derive(Serialize, Queryable, Selectable)]
#[diesel(table_name = chats)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ChatDb {
    pub id: Uuid,
    pub title: Option<String>,
    pub owner: Uuid,
    pub public: bool,
    pub created_at: NaiveDateTime,
}

#[derive(Deserialize, Insertable)]
#[diesel(table_name = chats)]
pub struct NewChatDb {
    pub title: Option<String>,
    pub owner: Uuid,
}

pub async fn insert(
    pool: &deadpool_diesel::postgres::Pool,
    new_chat: NewChatDb,
) -> Result<Chat, InfraError> {
    let conn = pool.get().await.map_err(InfraError::from)?;
    let res = conn
        .interact(|conn| {
            diesel::insert_into(chats::table)
                .values(new_chat)
                .returning(ChatDb::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(InfraError::from)?
        .map_err(InfraError::from)?;

    Ok(res.into())
}

pub async fn get_all(pool: &deadpool_diesel::postgres::Pool) -> Result<Vec<Chat>, InfraError> {
    let conn = pool.get().await.map_err(InfraError::from)?;
    let res = conn
        .interact(move |conn| {
            let query = chats::table.into_boxed::<diesel::pg::Pg>();
            query.select(ChatDb::as_select()).load::<ChatDb>(conn)
        })
        .await
        .map_err(InfraError::from)?
        .map_err(InfraError::from)?;

    let chats: Vec<Chat> = res.into_iter().map(Into::into).collect();

    Ok(chats)
}

/// Get a single chat by ID. Returns None if not found (not an error).
pub async fn get_by_id(
    pool: &deadpool_diesel::postgres::Pool,
    id: &ChatId,
) -> Result<Option<Chat>, InfraError> {
    let conn = pool.get().await.map_err(InfraError::from)?;
    let res = conn
        .interact(move |conn| {
            chats::table
                .filter(chats::id.eq(id.0))
                .select(ChatDb::as_select())
                .load::<ChatDb>(conn)
                .optional()
        })
        .await
        .map_err(InfraError::from)?
        .map_err(InfraError::from)?;

    Ok(res.map(Into::into))
}

/// Delete a chat. Only the owner can delete.
pub async fn delete(
    pool: &deadpool_diesel::postgres::Pool,
    id: &ChatId,
    user_id: &UserId,
) -> Result<(), InfraError> {
    let conn = pool.get().await.map_err(InfraError::from)?;
    let _res = conn
        .interact(move |conn| {
            diesel::delete(chats::table.filter(
                chats::id.eq(id.0).and(chats::owner.eq(user_id.0)),
            ))
            .execute(conn)
        })
        .await
        .map_err(InfraError::from)?
        .map_err(InfraError::from)?;

    // CASCADE handles messages/votes deletion via DB foreign keys
    Ok(())
}

/// Update chat title. Only the owner can update.
pub async fn update_title(
    pool: &deadpool_diesel::postgres::Pool,
    id: &ChatId,
    user_id: &UserId,
    title: &str,
) -> Result<Chat, InfraError> {
    let conn = pool.get().await.map_err(InfraError::from)?;
    let res = conn
        .interact(move |conn| {
            diesel::update(chats::table.filter(
                chats::id.eq(id.0).and(chats::owner.eq(user_id.0)),
            ))
            .set(chats::title.eq(Some(title)))
            .returning(ChatDb::as_returning())
            .get_result(conn)
        })
        .await
        .map_err(InfraError::from)?
        .map_err(InfraError::from)?;

    Ok(res.into())
}

/// Update chat visibility. Only the owner can update.
pub async fn update_visibility(
    pool: &deadpool_diesel::postgres::Pool,
    id: &ChatId,
    user_id: &UserId,
    is_public: bool,
) -> Result<Chat, InfraError> {
    let conn = pool.get().await.map_err(InfraError::from)?;
    let res = conn
        .interact(move |conn| {
            diesel::update(chats::table.filter(
                chats::id.eq(id.0).and(chats::owner.eq(user_id.0)),
            ))
            .set(chats::public.eq(is_public))
            .returning(ChatDb::as_returning())
            .get_result(conn)
        })
        .await
        .map_err(InfraError::from)?
        .map_err(InfraError::from)?;

    Ok(res.into())
}

impl Into<Chat> for ChatDb {
    fn into(self) -> Chat {
        Chat {
            id: ChatId(self.id),
            title: self.title,
            owner: UserId(self.owner),
            public: self.public,
            created_at: self.created_at,
        }
    }
}
