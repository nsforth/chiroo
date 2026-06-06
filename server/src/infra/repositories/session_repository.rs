use std::sync::Arc;

use chrono::NaiveDateTime;
use diesel::deserialize::Queryable;
use diesel::upsert::excluded;
use diesel::{
    ExpressionMethods, Insertable, OptionalExtension, QueryDsl, RunQueryDsl, Selectable,
    SelectableHelper,
};
use secrecy::{ExposeSecret, SecretBox};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::models::session::SessionId;
use crate::domain::models::session::{ProviderSessionId, Session};
use crate::domain::models::user::UserId;
use crate::infra::db::schema::sessions;
use crate::infra::errors::InfraError;
use crate::utils::crypto::{self, KEY_LEN};

#[derive(Serialize, Queryable, Selectable)]
#[diesel(table_name = sessions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct SessionDb {
    id: Uuid,
    user_id: Uuid,
    provider_sid: String,
    access_token: Vec<u8>,
    refresh_token: Vec<u8>,
    created_at: NaiveDateTime,
    expires_at: NaiveDateTime,
}

#[derive(Deserialize, Insertable)]
#[diesel(table_name = sessions)]
struct UpsertSession {
    provider_sid: String,
    user_id: Uuid,
    access_token: Vec<u8>,
    refresh_token: Vec<u8>,
    expires_at: NaiveDateTime,
}

#[derive(Clone)]
pub struct SessionRepository {
    encryption_key: Arc<SecretBox<[u8; KEY_LEN]>>,
}

impl SessionRepository {
    pub fn new(encryption_key: Arc<SecretBox<[u8; KEY_LEN]>>) -> Self {
        Self { encryption_key }
    }

    fn encrypt_token(&self, token: &SecretBox<str>) -> Result<Vec<u8>, InfraError> {
        crypto::encrypt(token.expose_secret().as_bytes(), &self.encryption_key)
    }

    fn decrypt_token(&self, bytes: &[u8]) -> Result<SecretBox<str>, InfraError> {
        let decrypted = crypto::decrypt(bytes, &self.encryption_key)?;
        String::from_utf8(decrypted)
            .map(|decrypted| SecretBox::from(decrypted))
            .map_err(|_| InfraError::InternalServerError)
    }

    fn session_from_db(&self, db: SessionDb) -> Result<Session, InfraError> {
        Ok(Session {
            id: SessionId(db.id),
            user_id: UserId(db.user_id),
            provider_sid: ProviderSessionId(db.provider_sid),
            access_token: self.decrypt_token(&db.access_token)?,
            refresh_token: self.decrypt_token(&db.refresh_token)?,
            created_at: db.created_at,
            expires_at: db.expires_at,
        })
    }

    pub async fn upsert(
        &self,
        pool: &deadpool_diesel::postgres::Pool,
        provider_sid: &str,
        user_id: Uuid,
        access_token: &SecretBox<str>,
        refresh_token: &SecretBox<str>,
        expires_at: NaiveDateTime,
    ) -> Result<Session, InfraError> {
        let sid_owned = provider_sid.to_string();
        let at_bytes = self.encrypt_token(access_token)?;
        let rt_bytes = self.encrypt_token(refresh_token)?;

        let conn = pool.get().await.map_err(InfraError::from)?;
        let res = conn
            .interact(move |conn| {
                diesel::insert_into(sessions::table)
                    .values(UpsertSession {
                        provider_sid: sid_owned,
                        user_id,
                        access_token: at_bytes,
                        refresh_token: rt_bytes,
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

        self.session_from_db(res)
    }

    pub async fn find_by_provider_sid(
        &self,
        pool: &deadpool_diesel::postgres::Pool,
        provider_sid: &ProviderSessionId,
    ) -> Result<Option<Session>, InfraError> {
        let sid_owned = provider_sid.0.clone();
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

        match res {
            Some(db) => Ok(Some(self.session_from_db(db)?)),
            None => Ok(None),
        }
    }
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
