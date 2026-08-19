use chrono::NaiveDateTime;
use diesel::deserialize::Queryable;
use diesel::{
    ExpressionMethods, Insertable, OptionalExtension, QueryDsl, RunQueryDsl, Selectable,
    SelectableHelper,
};
use serde_json::Value;
use tracing::instrument;
use uuid::Uuid;

use crate::domain::models::knowledge_base::{CreateKnowledgeBase, KnowledgeBase, KnowledgeBaseId};
use crate::infra::db::schema::knowledge_bases;
use crate::infra::errors::{map_infra_error, InfraError};

#[derive(Queryable, Selectable)]
#[diesel(table_name = knowledge_bases)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct KnowledgeBaseDb {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub public: bool,
    pub metadata: Value,
    pub language: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = knowledge_bases)]
struct NewKnowledgeBaseDb {
    name: String,
    description: String,
    public: bool,
    metadata: Value,
    language: Option<String>,
}

impl Into<KnowledgeBase> for KnowledgeBaseDb {
    fn into(self) -> KnowledgeBase {
        KnowledgeBase {
            id: KnowledgeBaseId(self.id),
            name: self.name,
            description: self.description,
            public: self.public,
            metadata: self.metadata,
            language: self.language,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[instrument(level = "debug", skip(pool))]
pub async fn insert(
    pool: &deadpool_diesel::postgres::Pool,
    cmd: &CreateKnowledgeBase,
) -> Result<KnowledgeBase, InfraError> {
    let new_kb = NewKnowledgeBaseDb {
        name: cmd.name.clone(),
        description: cmd.description.clone(),
        public: cmd.public,
        metadata: cmd.metadata.clone(),
        language: cmd.language.clone(),
    };
    let conn = pool.get().await.map_err(map_infra_error)?;
    conn.interact(|conn| {
        diesel::insert_into(knowledge_bases::table)
            .values(new_kb)
            .returning(KnowledgeBaseDb::as_returning())
            .get_result(conn)
    })
    .await
    .map_err(map_infra_error)?
    .map_err(map_infra_error)
    .map(Into::into)
}

#[instrument(level = "debug", skip(pool))]
pub async fn list_all(
    pool: &deadpool_diesel::postgres::Pool,
) -> Result<Vec<KnowledgeBase>, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    conn.interact(|conn| {
        knowledge_bases::table
            .select(KnowledgeBaseDb::as_select())
            .order_by(knowledge_bases::created_at.desc())
            .load::<KnowledgeBaseDb>(conn)
    })
    .await
    .map_err(map_infra_error)?
    .map_err(map_infra_error)
    .map(|v| v.into_iter().map(Into::into).collect())
}

#[instrument(level = "debug", skip(pool))]
pub async fn get_by_id(
    pool: &deadpool_diesel::postgres::Pool,
    id: KnowledgeBaseId,
) -> Result<Option<KnowledgeBase>, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    conn.interact(move |conn| {
        knowledge_bases::table
            .find(id.0)
            .select(KnowledgeBaseDb::as_select())
            .get_result::<KnowledgeBaseDb>(conn)
            .optional()
    })
    .await
    .map_err(map_infra_error)?
    .map_err(map_infra_error)
    .map(|opt| opt.map(Into::into))
}

#[instrument(level = "debug", skip(pool))]
pub async fn update(
    pool: &deadpool_diesel::postgres::Pool,
    kb: &KnowledgeBase,
) -> Result<KnowledgeBase, InfraError> {
    let id = kb.id.0;
    let name = kb.name.clone();
    let description = kb.description.clone();
    let public = kb.public;
    let metadata = kb.metadata.clone();
    let conn = pool.get().await.map_err(map_infra_error)?;
    conn.interact(move |conn| {
        diesel::update(knowledge_bases::table.find(id))
            .set((
                knowledge_bases::name.eq(name),
                knowledge_bases::description.eq(description),
                knowledge_bases::public.eq(public),
                knowledge_bases::metadata.eq(metadata),
                knowledge_bases::updated_at.eq(diesel::dsl::now),
            ))
            .returning(KnowledgeBaseDb::as_returning())
            .get_result(conn)
    })
    .await
    .map_err(map_infra_error)?
    .map_err(map_infra_error)
    .map(Into::into)
}

#[instrument(level = "debug", skip(pool))]
pub async fn delete(
    pool: &deadpool_diesel::postgres::Pool,
    id: KnowledgeBaseId,
) -> Result<bool, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    conn.interact(move |conn| diesel::delete(knowledge_bases::table.find(id.0)).execute(conn))
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;
    Ok(true)
}
