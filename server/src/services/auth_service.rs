use deadpool_diesel::postgres::Pool;
use secrecy::{ExposeSecret, SecretBox};
use std::time::Duration;
use tokio::sync::OnceCell;

use crate::config::OidcConfig;
use crate::domain::models::session::ProviderSessionId;
use crate::domain::models::user::{User, UserId};
use crate::errors::AppError;
use crate::infra::clients::oidc_client::{AccessTokenClaims, OidcClient, TokenStatus};
use crate::infra::repositories::{session_repository, user_repository};

pub enum AuthResult {
    Valid {
        claims: AccessTokenClaims,
        user: User,
    },
    Refreshed {
        access_token: SecretBox<str>,
        user: User,
        provider_sid: String,
    },
    Expired,
    Invalid,
}

#[derive(Clone)]
pub struct AuthService {
    oidc_config: OidcConfig,
    oidc_client: OnceCell<OidcClient>,
    pool: Pool,
}

impl AuthService {
    pub fn new(oidc_config: OidcConfig, pool: Pool) -> Self {
        Self {
            oidc_config,
            oidc_client: OnceCell::new(),
            pool,
        }
    }

    async fn get_client(&self) -> Result<&OidcClient, AppError> {
        self.oidc_client
            .get_or_try_init(|| async { OidcClient::new(&self.oidc_config).await })
            .await
    }

    pub fn start_background_jobs(self, cleanup_interval_secs: Duration) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval_secs);
            loop {
                interval.tick().await;
                if let Err(e) = self.cleanup_expired().await {
                    tracing::error!("Session cleanup failed: {:?}", e);
                }
            }
        });
    }

    pub async fn build_login_url(&self, redirect: String) -> Result<String, AppError> {
        let client = self.get_client().await?;
        let result = client.build_auth_url(&redirect)?;
        Ok(result.url.to_string())
    }

    async fn resolve_user_by_sso_id(&self, sso_id: &str) -> Result<User, AppError> {
        user_repository::find_by_sso_id(&self.pool, sso_id)
            .await?
            .map(Into::into)
            .ok_or_else(|| AppError::Unauthorized)
    }

    // TODO: Add logging on errors
    // TODO: Better error handling, need to match on variants of AppError in private fn
    pub async fn verify_or_refresh(&self, access_token: &SecretBox<str>) -> AuthResult {
        match self.verify_access_token(access_token).await {
            TokenStatus::Valid(claims) => match self.resolve_user_by_sso_id(&claims.sub).await {
                Ok(user) => AuthResult::Valid { claims, user },
                Err(_) => AuthResult::Invalid,
            },
            TokenStatus::Expired(claims) => match self
                .refresh_session(ProviderSessionId(claims.sid.clone()), access_token)
                .await
            {
                Ok((new_token, user, sid)) => AuthResult::Refreshed {
                    access_token: new_token,
                    user,
                    provider_sid: sid,
                },
                Err(e) => {
                    tracing::debug!("User session is expired: {}", e);
                    AuthResult::Expired
                },
            },
            TokenStatus::Invalid(e) => {
                tracing::info!("Invalid token: {}", e);
                AuthResult::Invalid
            },
        }
    }

    pub async fn handle_callback(
        &self,
        code: &str,
        state: &str,
    ) -> Result<(String, SecretBox<str>), AppError> {
        let client = self.get_client().await?;
        let result = client.exchange_code(code, state).await?;

        let (sub, sid, name, preferred_username, email, picture) =
            if let Some(ref claims) = result.id_claims {
                (
                    claims.sub.clone(),
                    claims.sid.clone(),
                    claims.name.clone(),
                    claims.preferred_username.clone(),
                    claims.email.clone(),
                    claims.picture.clone(),
                )
            } else {
                return Err(AppError::OidcError("No ID claims in token exchange".into()));
            };

        let user = user_repository::upsert(
            &self.pool,
            &sub,
            "oidc",
            name.as_deref(),
            preferred_username.as_deref(),
            email.as_deref(),
            picture.as_deref(),
        )
        .await?;

        session_repository::upsert(
            &self.pool,
            &sid,
            user.id,
            &result.access_token,
            &result.refresh_token,
            result.session_expires_at,
        )
        .await?;

        Ok((result.redirect_url, result.access_token))
    }

    async fn refresh_session(
        &self,
        sid: ProviderSessionId,
        access_token: &SecretBox<str>,
    ) -> Result<(SecretBox<str>, User, String), AppError> {
        let session = session_repository::find_by_provider_sid(&self.pool, &sid)
            .await?
            .ok_or_else(|| AppError::Unauthorized)?;

        if session.access_token.expose_secret() != access_token.expose_secret() {
            return Err(AppError::Unauthorized);
        }

        let user: User = user_repository::find_by_id(&self.pool, session.user_id.0)
            .await?
            .map(Into::into)
            .ok_or_else(|| AppError::Unauthorized)?;

        let client = self.get_client().await?;
        let result = client.refresh_token(&session.refresh_token).await?;

        let sid = match self.verify_access_token(&result.access_token).await {
            TokenStatus::Valid(claims) => claims.sid,
            _ => return Err(AppError::Unauthorized),
        };

        session_repository::upsert(
            &self.pool,
            &sid,
            user.id.0,
            &result.access_token,
            &result.refresh_token,
            result.session_expires_at,
        )
        .await?;

        Ok((result.access_token, user, sid))
    }

    async fn verify_access_token(&self, token: &SecretBox<str>) -> TokenStatus<AccessTokenClaims> {
        match self.get_client().await {
            Ok(client) => client.verify_access_token(token).await,
            Err(e) => TokenStatus::Invalid(e.to_string()),
        }
    }

    pub async fn cleanup_expired(&self) -> Result<usize, AppError> {
        session_repository::delete_expired(&self.pool)
            .await
            .map_err(|_| AppError::InternalServerError)
    }

    pub async fn get_user(&self, user_id: UserId) -> Result<User, AppError> {
        user_repository::find_by_id(&self.pool, user_id.0)
            .await?
            .map(Into::into)
            .ok_or_else(|| AppError::NotFound("User not found".into()))
    }
}
