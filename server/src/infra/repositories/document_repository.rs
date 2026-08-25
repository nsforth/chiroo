use chrono::NaiveDateTime;
use diesel::deserialize::Queryable;
use diesel::expression_methods::{BoolExpressionMethods, PgTextExpressionMethods};
use diesel::{
    ExpressionMethods, Insertable, OptionalExtension, QueryDsl, RunQueryDsl, Selectable,
    SelectableHelper,
};
use serde_json::Value;
use tracing::instrument;
use uuid::Uuid;

use crate::domain::models::document::{
    CreateDocument, Document, DocumentData, DocumentId, MimeType,
};
use crate::domain::models::knowledge_base::KnowledgeBaseId;
use crate::infra::db::schema::{document_data, documents};
use crate::infra::errors::{map_infra_error, InfraError};

#[derive(Queryable, Selectable)]
#[diesel(table_name = documents)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct DocumentDb {
    pub id: String,
    pub kb_id: Uuid,
    pub title: String,
    pub url: String,
    pub version: String,
    pub mime_type: String,
    pub metadata: Value,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Insertable)]
#[diesel(table_name = documents)]
struct NewDocumentDb {
    id: String,
    kb_id: Uuid,
    title: String,
    url: String,
    version: String,
    mime_type: String,
    metadata: Value,
}

impl Into<Document> for DocumentDb {
    fn into(self) -> Document {
        Document {
            id: DocumentId(self.id),
            kb_id: KnowledgeBaseId(self.kb_id),
            title: self.title,
            url: self.url,
            version: self.version,
            mime_type: MimeType::from_str(&self.mime_type).unwrap_or(MimeType::TextPlain),
            metadata: self.metadata,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = document_data)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct DocumentDataDb {
    pub document_id: String,
    pub data: Vec<u8>,
}

#[derive(Insertable)]
#[diesel(table_name = document_data)]
struct NewDocumentDataDb {
    document_id: String,
    data: Vec<u8>,
}

impl Into<DocumentData> for DocumentDataDb {
    fn into(self) -> DocumentData {
        DocumentData {
            document_id: DocumentId(self.document_id),
            data: self.data,
        }
    }
}

#[instrument(level = "debug", skip(pool, data))]
pub async fn insert(
    pool: &deadpool_diesel::postgres::Pool,
    cmd: &CreateDocument,
    data: &[u8],
) -> Result<Document, InfraError> {
    let new_doc = NewDocumentDb {
        id: cmd.id.clone(),
        kb_id: cmd.kb_id.0,
        title: cmd.title.clone(),
        url: cmd.url.clone(),
        version: cmd.version.clone(),
        mime_type: cmd.mime_type.as_str().to_string(),
        metadata: cmd.metadata.clone(),
    };
    let doc_data = NewDocumentDataDb {
        document_id: cmd.id.clone(),
        data: data.to_vec(),
    };
    let conn = pool.get().await.map_err(map_infra_error)?;
    conn.interact(move |conn| {
        conn.build_transaction()
            .run::<DocumentDb, diesel::result::Error, _>(|conn| {
                let doc_db: DocumentDb = diesel::insert_into(documents::table)
                    .values(&new_doc)
                    .returning(DocumentDb::as_returning())
                    .get_result(conn)?;
                diesel::insert_into(document_data::table)
                    .values(&doc_data)
                    .execute(conn)?;
                Ok(doc_db)
            })
    })
    .await
    .map_err(map_infra_error)?
    .map_err(map_infra_error)
    .map(Into::into)
}

#[instrument(level = "debug", skip(pool))]
pub async fn list_by_kb_page(
    pool: &deadpool_diesel::postgres::Pool,
    kb_id: KnowledgeBaseId,
    filter: Option<&str>,
    cursor: Option<&str>,
    limit: i64,
) -> Result<(Vec<Document>, Option<String>), InfraError> {
    let pattern = filter.map(|f| format!("%{f}%"));
    let cursor_owned = cursor.map(str::to_owned);
    let conn = pool.get().await.map_err(map_infra_error)?;
    let result: Vec<DocumentDb> = conn
        .interact(move |conn| {
            let mut query = documents::table
                .filter(documents::kb_id.eq(kb_id.0))
                .into_boxed();
            if let Some(cursor) = cursor_owned.as_deref() {
                query = query.filter(documents::id.gt(cursor));
            }
            if let Some(pattern) = pattern.as_deref() {
                query = query.filter(
                    documents::title
                        .ilike(pattern)
                        .or(documents::url.ilike(pattern)),
                );
            }
            query
                .select(DocumentDb::as_select())
                .order_by(documents::id.asc())
                .limit(limit + 1)
                .load::<DocumentDb>(conn)
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;
    let next_cursor = if result.len() > limit as usize {
        Some(result[limit as usize - 1].id.clone())
    } else {
        None
    };
    let docs = result
        .into_iter()
        .take(limit as usize)
        .map(Into::into)
        .collect();
    Ok((docs, next_cursor))
}

#[instrument(level = "debug", skip(pool))]
pub async fn list_versions_page_by_kb(
    pool: &deadpool_diesel::postgres::Pool,
    kb_id: KnowledgeBaseId,
    cursor: Option<&str>,
    limit: i64,
) -> Result<(Vec<(String, String)>, Option<String>), InfraError> {
    let cursor_owned = cursor.map(str::to_owned);
    let conn = pool.get().await.map_err(map_infra_error)?;
    let result: Vec<(String, String)> = conn
        .interact(move |conn| {
            let mut query = documents::table
                .filter(documents::kb_id.eq(kb_id.0))
                .into_boxed();
            if let Some(cursor) = cursor_owned.as_deref() {
                query = query.filter(documents::id.gt(cursor));
            }
            query
                .select((documents::id, documents::version))
                .order_by(documents::id.asc())
                .limit(limit + 1)
                .load::<(String, String)>(conn)
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;
    let next_cursor = if result.len() > limit as usize {
        Some(result[limit as usize - 1].0.clone())
    } else {
        None
    };
    let items = result.into_iter().take(limit as usize).collect();
    Ok((items, next_cursor))
}

#[instrument(level = "debug", skip(pool))]
pub async fn get_by_id(
    pool: &deadpool_diesel::postgres::Pool,
    id: &DocumentId,
) -> Result<Option<Document>, InfraError> {
    let id_clone = id.0.clone();
    let conn = pool.get().await.map_err(map_infra_error)?;
    conn.interact(move |conn| {
        documents::table
            .find(&id_clone)
            .select(DocumentDb::as_select())
            .get_result::<DocumentDb>(conn)
            .optional()
    })
    .await
    .map_err(map_infra_error)?
    .map_err(map_infra_error)
    .map(|opt| opt.map(Into::into))
}

#[instrument(level = "debug", skip(pool))]
pub async fn get_data_by_id(
    pool: &deadpool_diesel::postgres::Pool,
    id: &DocumentId,
) -> Result<Option<DocumentData>, InfraError> {
    let id_clone = id.0.clone();
    let conn = pool.get().await.map_err(map_infra_error)?;
    conn.interact(move |conn| {
        document_data::table
            .find(&id_clone)
            .select(DocumentDataDb::as_select())
            .get_result::<DocumentDataDb>(conn)
            .optional()
    })
    .await
    .map_err(map_infra_error)?
    .map_err(map_infra_error)
    .map(|opt| opt.map(Into::into))
}

#[instrument(level = "debug", skip(pool))]
pub async fn get_with_data(
    pool: &deadpool_diesel::postgres::Pool,
    id: &DocumentId,
) -> Result<Option<(Document, Vec<u8>)>, InfraError> {
    let id_clone = id.0.clone();
    let conn = pool.get().await.map_err(map_infra_error)?;
    conn.interact(
        move |conn| -> Result<Option<(DocumentDb, Vec<u8>)>, diesel::result::Error> {
            let doc: Option<DocumentDb> = documents::table
                .find(&id_clone)
                .select(DocumentDb::as_select())
                .get_result::<DocumentDb>(conn)
                .optional()?;

            let Some(doc) = doc else {
                return Ok(None);
            };

            let data: Option<Vec<u8>> = document_data::table
                .find(&id_clone)
                .select(document_data::data)
                .get_result::<Vec<u8>>(conn)
                .optional()?;

            Ok(data.map(|d| (doc, d)))
        },
    )
    .await
    .map_err(map_infra_error)?
    .map_err(map_infra_error)
    .map(|opt| opt.map(|(doc, data)| (doc.into(), data)))
}

#[instrument(level = "debug", skip(pool, data))]
pub async fn update(
    pool: &deadpool_diesel::postgres::Pool,
    doc: &Document,
    data: &[u8],
) -> Result<Document, InfraError> {
    let id_clone = doc.id.0.clone();
    let title = doc.title.clone();
    let url = doc.url.clone();
    let version = doc.version.clone();
    let mime_type = doc.mime_type.as_str().to_string();
    let metadata = doc.metadata.clone();
    let data_vec = data.to_vec();
    let conn = pool.get().await.map_err(map_infra_error)?;
    conn.interact(move |conn| {
        conn.build_transaction()
            .run::<DocumentDb, diesel::result::Error, _>(|conn| {
                let doc_db: DocumentDb = diesel::update(documents::table.find(&id_clone))
                    .set((
                        documents::title.eq(title),
                        documents::url.eq(url),
                        documents::version.eq(version),
                        documents::mime_type.eq(mime_type),
                        documents::metadata.eq(metadata),
                        documents::updated_at.eq(diesel::dsl::now),
                    ))
                    .returning(DocumentDb::as_returning())
                    .get_result(conn)?;

                diesel::insert_into(document_data::table)
                    .values((
                        document_data::document_id.eq(&id_clone),
                        document_data::data.eq(&data_vec),
                    ))
                    .on_conflict(document_data::document_id)
                    .do_update()
                    .set(document_data::data.eq(&data_vec))
                    .execute(conn)?;

                Ok(doc_db)
            })
    })
    .await
    .map_err(map_infra_error)?
    .map_err(map_infra_error)
    .map(Into::into)
}

#[instrument(level = "debug", skip(pool, metadata))]
pub async fn update_metadata(
    pool: &deadpool_diesel::postgres::Pool,
    id: &DocumentId,
    title: &str,
    url: &str,
    metadata: &Value,
) -> Result<Document, InfraError> {
    let id_clone = id.0.clone();
    let title = title.to_string();
    let url = url.to_string();
    let metadata = metadata.clone();
    let conn = pool.get().await.map_err(map_infra_error)?;
    conn.interact(move |conn| {
        diesel::update(documents::table.find(&id_clone))
            .set((
                documents::title.eq(title),
                documents::url.eq(url),
                documents::metadata.eq(metadata),
                documents::updated_at.eq(diesel::dsl::now),
            ))
            .returning(DocumentDb::as_returning())
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
    id: &DocumentId,
) -> Result<bool, InfraError> {
    let id_clone = id.0.clone();
    let conn = pool.get().await.map_err(map_infra_error)?;
    conn.interact(move |conn| diesel::delete(documents::table.find(&id_clone)).execute(conn))
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;
    Ok(true)
}

#[instrument(level = "debug", skip(pool))]
pub async fn delete_by_kb(
    pool: &deadpool_diesel::postgres::Pool,
    kb_id: KnowledgeBaseId,
) -> Result<u64, InfraError> {
    let conn = pool.get().await.map_err(map_infra_error)?;
    let count = conn
        .interact(move |conn| {
            diesel::delete(documents::table.filter(documents::kb_id.eq(kb_id.0))).execute(conn)
        })
        .await
        .map_err(map_infra_error)?
        .map_err(map_infra_error)?;
    Ok(count as u64)
}
