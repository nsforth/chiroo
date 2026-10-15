use std::time::Duration;
use tokio::sync::OnceCell;
use tracing::{debug, info, instrument};

use deadpool_diesel::postgres::Pool;
use secrecy::{ExposeSecret, SecretBox};

use crate::config::OidcConfig;
use crate::domain::models::auth_user::AuthUser;
use crate::domain::models::session::ProviderSessionId;
use crate::errors::AppError;
use crate::infra::clients::oidc_client::{AccessTokenClaims, OidcClient, TokenStatus};
use crate::infra::repositories::session_repository::{self, SessionRepository};
use crate::infra::repositories::user_repository;
use crate::operation_result::OperationResult;

pub enum AuthResult {
    Valid(AuthUser),
    Refreshed(AuthUser, SecretBox<str>),
    Expired,
    Invalid,
}

impl OperationResult for AuthResult {
    fn is_success(&self) -> bool {
        matches!(self, AuthResult::Valid(_) | AuthResult::Refreshed(_, _))
    }
}

impl std::fmt::Display for AuthResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthResult::Valid(_) => write!(f, "valid"),
            AuthResult::Refreshed(_, _) => write!(f, "refreshed"),
            AuthResult::Expired => write!(f, "expired"),
            AuthResult::Invalid => write!(f, "invalid"),
        }
    }
}

#[derive(Clone)]
pub struct AuthService {
    oidc_config: OidcConfig,
    oidc_client: OnceCell<OidcClient>,
    pool: Pool,
    session_repo: SessionRepository,
}

impl AuthService {
    pub fn new(session_repo: SessionRepository, oidc_config: OidcConfig, pool: Pool) -> Self {
        Self {
            oidc_config,
            oidc_client: OnceCell::new(),
            pool,
            session_repo,
        }
    }

    async fn get_client(&self) -> Result<&OidcClient, AppError> {
        self.oidc_client
            .get_or_try_init(|| async { OidcClient::new(&self.oidc_config).await })
            .await
    }

    #[instrument(skip(self))]
    pub fn start_background_jobs(self, cleanup_interval: Duration) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            loop {
                interval.tick().await;
                if let Err(e) = self.cleanup_expired().await {
                    tracing::error!("Session cleanup failed: {:?}", e);
                }
            }
        });
    }

    #[instrument(skip(self))]
    pub async fn build_login_url(&self, redirect: String) -> Result<String, AppError> {
        let client = self.get_client().await?;
        let result = client.build_auth_url(&redirect)?;
        let url = result.url.to_string();
        info!(status = "success");
        Ok(url)
    }

    #[instrument(skip(self))]
    pub async fn verify_or_refresh(&self, access_token: &SecretBox<str>) -> AuthResult {
        let result = match self.verify_access_token(access_token).await {
            TokenStatus::Valid(claims) => {
                match self
                    .session_repo
                    .find_by_provider_sid(&self.pool, &ProviderSessionId(claims.sid.clone()))
                    .await
                {
                    Ok(Some(session)) => AuthResult::Valid(AuthUser {
                        user_id: session.user_id,
                        session_id: session.id.0.to_string(),
                    }),
                    _ => AuthResult::Invalid,
                }
            },
            TokenStatus::Expired(claims) => match self
                .refresh_session(ProviderSessionId(claims.sid.clone()), access_token)
                .await
            {
                Ok((new_token, auth_user)) => AuthResult::Refreshed(auth_user, new_token),
                Err(_e) => AuthResult::Expired,
            },
            TokenStatus::Invalid(_e) => AuthResult::Invalid,
        };
        info!(auth_result = %result, status = result.status());
        result
    }

    #[instrument(skip(self, code, state))]
    pub async fn handle_callback(
        &self,
        code: &str,
        state: &str,
    ) -> Result<(String, SecretBox<str>, AuthUser), AppError> {
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

        let username = match preferred_username.as_ref().or(email.as_ref()) {
            Some(username) => username.clone(),
            None => format!("user_{sub}"),
        };

        let user = user_repository::upsert(
            &self.pool,
            &sub,
            "oidc",
            name.as_deref(),
            &username,
            email.as_deref(),
            picture.as_deref(),
        )
        .await?;

        let session = self
            .session_repo
            .upsert(
                &self.pool,
                &sid,
                user.id,
                &result.access_token,
                &result.refresh_token,
                result.session_expires_at,
            )
            .await?;

        let auth_user = AuthUser {
            user_id: session.user_id,
            session_id: session.id.0.to_string(),
        };

        info!(status = "success");
        Ok((result.redirect_url, result.access_token, auth_user))
    }

    async fn refresh_session(
        &self,
        sid: ProviderSessionId,
        access_token: &SecretBox<str>,
    ) -> Result<(SecretBox<str>, AuthUser), AppError> {
        let session = self
            .session_repo
            .find_by_provider_sid(&self.pool, &sid)
            .await?
            .ok_or_else(|| AppError::Unauthorized)?;

        if session.access_token.expose_secret() != access_token.expose_secret() {
            return Err(AppError::Unauthorized);
        }

        let client = self.get_client().await?;
        let result = client.refresh_token(&session.refresh_token).await?;

        let new_sid = match self.verify_access_token(&result.access_token).await {
            TokenStatus::Valid(claims) => claims.sid,
            _ => return Err(AppError::Unauthorized),
        };

        self.session_repo
            .upsert(
                &self.pool,
                &new_sid,
                session.user_id.0,
                &result.access_token,
                &result.refresh_token,
                result.session_expires_at,
            )
            .await?;

        let auth_user = AuthUser {
            user_id: session.user_id,
            session_id: session.id.0.to_string(),
        };

        Ok((result.access_token, auth_user))
    }

    async fn verify_access_token(&self, token: &SecretBox<str>) -> TokenStatus<AccessTokenClaims> {
        match self.get_client().await {
            Ok(client) => client.verify_access_token(token).await,
            Err(e) => TokenStatus::Invalid(e.to_string()),
        }
    }

    #[instrument(skip(self))]
    pub async fn cleanup_expired(&self) -> Result<usize, AppError> {
        let result = session_repository::delete_expired(&self.pool)
            .await
            .map_err(|_| AppError::InternalServerError);
        match &result {
            Ok(count) if *count > 0usize => info!(deleted = count, status = result.status()),
            Ok(count) => debug!(deleted = count, status = result.status()),
            Err(e) => info!(error = e.to_string(), status = result.status()),
        }
        result
    }
}
