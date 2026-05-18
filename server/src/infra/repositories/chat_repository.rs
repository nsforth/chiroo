use chrono::NaiveDateTime;
use diesel::{
    ExpressionMethods, Insertable, PgTextExpressionMethods, QueryDsl, Queryable, RunQueryDsl,
    Selectable, SelectableHelper,
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
