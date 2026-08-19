use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel::{
    BoolExpressionMethods, ExpressionMethods, Insertable, OptionalExtension, QueryDsl, RunQueryDsl,
    Selectable, SelectableHelper,
};
use serde::{Deserialize, Serialize};
use tracing::instrument;
use uuid::Uuid;

use crate::domain::models::messages::Role;
use crate::infra::db::schema::messages;
use crate::infra::errors::{map_infra_error, InfraError};

pub fn message_role_to_db_role(role: &Role) -> &'static str {
    match role {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::System => "system",
    }
}

#[derive(Serialize, Deserialize, Debug, Selectable, Queryable)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[diesel(table_name = messages)]
pub struct MessageDb {
    pub id: Uuid,
    pub chat_id: Uuid,
    pub role: String,
    #[serde(default)]
    pub parts: serde_json::Value,
    pub created_at: NaiveDateTime,
}

#[derive(Deserialize, Insertable)]
#[diesel(table_name = messages)]
pub struct NewMessageDb {
    pub id: Option<Uuid>,
    pub chat_id: Uuid,
    pub role: String,
    #[serde(default)]
    pub parts: serde_json::Value,
}

/// Get all messages for a specific chat, ordered by created_at ascending.
#[instrument(level = "debug", skip(pool))]
pub async fn get_messages_for_chat(
    pool: &deadpool_diesel::postgres::Pool,
    chat_id: Uuid,
) -> Result<Vec<MessageDb>, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(move |conn| {
            messages::table
                .filter(messages::chat_id.eq(chat_id))
                .order(messages::created_at.asc())
                .select(MessageDb::as_select())
                .load::<MessageDb>(conn)
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res)
}

/// Insert a new message and return it.
#[instrument(level = "debug", skip(pool, new_message))]
pub async fn insert(
    pool: &deadpool_diesel::postgres::Pool,
    new_message: NewMessageDb,
) -> Result<MessageDb, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let res = conn
        .interact(|conn| {
            diesel::insert_into(messages::table)
                .values(new_message)
                .returning(MessageDb::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res)
}

/// Delete all messages created after the given message_id for a specific chat.
/// Used for truncating history when user edits or regenerates a message.
#[instrument(level = "debug", skip(pool))]
pub async fn delete_after_message(
    pool: &deadpool_diesel::postgres::Pool,
    chat_id: Uuid,
    target_message_id: Uuid,
) -> Result<usize, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    // First, get the created_at of the target message
    let target_created_at = conn
        .interact(move |conn| {
            messages::table
                .filter(messages::id.eq(target_message_id))
                .select(messages::created_at)
                .first::<NaiveDateTime>(conn)
                .optional()
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    let Some(target_created_at) = target_created_at else {
        return Ok(0);
    };

    // Delete all messages in the chat created after the target message
    let res = conn
        .interact(move |conn| {
            diesel::delete(
                messages::table.filter(
                    messages::chat_id
                        .eq(chat_id)
                        .and(messages::created_at.gt(target_created_at)),
                ),
            )
            .execute(conn)
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;

    Ok(res as usize)
}
