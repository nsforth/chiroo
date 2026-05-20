use diesel::{ExpressionMethods, Insertable, QueryDsl, RunQueryDsl, Selectable};
use serde::Serialize;
use uuid::Uuid;

use crate::infra::db::schema::votes;
use crate::infra::errors::InfraError;

#[derive(Serialize, Debug)]
pub struct VoteDb {
    pub chat_id: Uuid,
    pub message_id: Uuid,
    pub is_upvoted: bool,
}

#[derive(Insertable)]
#[diesel(table_name = votes)]
pub struct NewVoteDb {
    pub chat_id: Uuid,
    pub message_id: Uuid,
    pub is_upvoted: bool,
}

/// Get all votes for a specific chat.
pub async fn get_votes_for_chat(
    pool: &deadpool_diesel::postgres::Pool,
    chat_id: Uuid,
) -> Result<Vec<VoteDb>, InfraError> {
    let conn = pool.get().await.map_err(InfraError::from)?;
    let res = conn
        .interact(move |conn| {
            votes::table
                .filter(votes::chat_id.eq(chat_id))
                .load::<VoteDb>(conn)
        })
        .await
        .map_err(InfraError::from)?
        .map_err(InfraError::from)?;

    Ok(res)
}

/// Insert or update a vote for a message in a chat.
/// Uses ON CONFLICT to handle the composite primary key (chat_id, message_id).
pub async fn upsert_vote(
    pool: &deadpool_diesel::postgres::Pool,
    new_vote: NewVoteDb,
) -> Result<VoteDb, InfraError> {
    let conn = pool.get().await.map_err(InfraError::from)?;
    let res = conn
        .interact(|conn| {
            diesel::insert_into(votes::table)
                .values(new_vote)
                .on_conflict_do_update()
                .column(votes::chat_id)
                .column(votes::message_id)
                .set(votes::is_upvoted.eq(new_vote.is_upvoted))
                .returning(VoteDb::as_returning())
                .get_result(conn)
        })
        .await
        .map_err(InfraError::from)?
        .map_err(InfraError::from)?;

    Ok(res)
}

/// Delete a vote for a specific message in a chat.
pub async fn delete_vote(
    pool: &deadpool_diesel::postgres::Pool,
    chat_id: Uuid,
    message_id: Uuid,
) -> Result<usize, InfraError> {
    let conn = pool.get().await.map_err(InfraError::from)?;
    let res = conn
        .interact(move |conn| {
            diesel::delete(
                votes::table.filter(votes::chat_id.eq(chat_id)).filter(votes::message_id.eq(message_id)),
            )
            .execute(conn)
        })
        .await
        .map_err(InfraError::from)?
        .map_err(InfraError::from)?;

    Ok(res as usize)
}
