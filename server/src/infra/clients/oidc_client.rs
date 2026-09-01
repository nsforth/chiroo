use std::time::Duration;

use chrono::NaiveDateTime;
use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use openidconnect::core::{
    CoreAuthenticationFlow, CoreClient, CoreIdTokenClaims, CoreProviderMetadata, CoreTokenResponse,
};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, OAuth2TokenResponse,
    RedirectUrl, RefreshToken, Scope, TokenResponse,
};
use secrecy::{ExposeSecret, SecretBox};
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};
use url::Url;

use crate::config::OidcConfig;
use crate::errors::AppError;

#[derive(Serialize, Deserialize)]
struct StateTokenPayload {
    #[serde(rename = "r")]
    redirect: String,
    #[serde(rename = "n")]
    nonce: String,
    #[serde(rename = "e")]
    exp: i64,
}

fn encode_state_token(
    redirect: &str,
    nonce: &str,
    secret: &SecretBox<str>,
) -> Result<String, AppError> {
    let claims = StateTokenPayload {
        redirect: redirect.to_string(),
        nonce: nonce.to_string(),
        exp: chrono::Utc::now().timestamp() + 300,
    };
    let header = Header::new(Algorithm::HS256);
    Ok(jsonwebtoken::encode(
        &header,
        &claims,
        &EncodingKey::from_secret(secret.expose_secret().as_bytes()),
    )?)
}

fn decode_state_token(state: &str, secret: &SecretBox<str>) -> Result<StateTokenPayload, AppError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = true;
    validation.required_spec_claims.clear();
    let token_data = jsonwebtoken::decode::<StateTokenPayload>(
        state,
        &DecodingKey::from_secret(secret.expose_secret().as_bytes()),
        &validation,
    )?;
    Ok(token_data.claims)
}

#[cached::cached(
    ttl = 3600,
    result = true,
    key = "String",
    convert = r#"{ jwks_uri.clone() }"#
)]
async fn fetch_jwk_set(jwks_uri: String) -> Result<JwkSet, AppError> {
    let client = reqwest::Client::new();
    let jwks: JwkSet = client
        .get(&jwks_uri)
        .send()
        .await
        .map_err(|e| {
            let msg = format!("Failed to fetch JWK: {e}");
            debug!(error = msg, status = "failed");
            AppError::OidcError(msg)
        })?
        .json()
        .await
        .map_err(|e| AppError::OidcError(format!("Failed to parse JWK set: {e}")))?;
    Ok(jwks)
}

pub struct AuthUrlResult {
    pub url: Url,
}

#[derive(Debug)]
pub struct TokenExchangeResult {
    pub access_token: SecretBox<str>,
    pub refresh_token: SecretBox<str>,
    pub id_claims: Option<IdTokenClaims>,
    pub session_expires_at: chrono::NaiveDateTime,
    pub redirect_url: String,
}

impl TokenExchangeResult {
    fn new(
        access_token: SecretBox<str>,
        refresh_token: SecretBox<str>,
        id_claims: Option<IdTokenClaims>,
        session_expires_at: chrono::NaiveDateTime,
        redirect_url: String,
    ) -> Self {
        Self {
            access_token,
            refresh_token,
            id_claims,
            session_expires_at,
            redirect_url,
        }
    }
}

#[derive(Debug)]
pub struct IdTokenClaims {
    pub sub: String,
    pub sid: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub preferred_username: Option<String>,
    pub picture: Option<String>,
}

impl IdTokenClaims {
    fn new(sub: String, sid: String, claims: &CoreIdTokenClaims) -> Self {
        IdTokenClaims {
            sub,
            sid,
            name: claims
                .name()
                .and_then(|n| n.get(None).map(|v| v.to_string())),
            email: claims.email().map(|e| e.to_string()),
            preferred_username: claims.preferred_username().map(|u| u.to_string()),
            picture: claims
                .picture()
                .and_then(|p| p.get(None).map(|v| v.to_string())),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    pub sub: String,
    pub sid: String,
}

#[derive(Deserialize, Debug)]
struct RefreshTokenClaims {
    exp: Option<i64>,
}

#[derive(Debug)]
pub enum TokenStatus<T> {
    Valid(T),
    Expired(T),
    Invalid(String),
}

#[derive(Clone)]
pub struct OidcClient {
    metadata: CoreProviderMetadata,
    client_id: ClientId,
    client_secret: ClientSecret,
    redirect_uri: RedirectUrl,
    http_client: reqwest::Client,
    issuer_url: String,
    jwks_uri: String,
    state_secret: SecretBox<str>,
    session_ttl_multiplier: u32,
    default_expires_in: Duration,
}

impl OidcClient {
    pub async fn new(config: &OidcConfig) -> Result<Self, AppError> {
        let http_client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| AppError::OidcError(format!("Failed to create HTTP client: {e}")))?;

        let metadata = CoreProviderMetadata::discover_async(
            IssuerUrl::new(config.issuer_url().to_string())?,
            &http_client,
        )
        .await
        .map_err(|e| {
            let msg = format!("Provider discovery: {e:?}");
            debug!(error = msg, status = "failed");
            AppError::OidcError(msg)
        })?;

        let jwks_uri = metadata.jwks_uri().to_string();

        Ok(Self {
            metadata,
            client_id: ClientId::new(config.client_id().expose_secret().to_string()),
            client_secret: ClientSecret::new(config.client_secret().expose_secret().to_string()),
            redirect_uri: RedirectUrl::new(config.redirect_uri().to_string())?,
            http_client,
            issuer_url: config.issuer_url().to_string(),
            jwks_uri,
            state_secret: config.state_secret().clone(),
            session_ttl_multiplier: config.session_ttl_multiplier(),
            default_expires_in: config.default_access_token_expires_secs(),
        })
    }

    #[instrument(level = "debug", skip(self, redirect))]
    pub fn build_auth_url(&self, redirect: &str) -> Result<AuthUrlResult, AppError> {
        let nonce_value = Nonce::new_random();
        let signed_state = encode_state_token(redirect, nonce_value.secret(), &self.state_secret)?;
        let csrf_token = CsrfToken::new(signed_state);

        let client = CoreClient::from_provider_metadata(
            self.metadata.clone(),
            self.client_id.clone(),
            Some(self.client_secret.clone()),
        )
        .set_redirect_uri(self.redirect_uri.clone());

        let (auth_url, _, _) = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                || csrf_token,
                || nonce_value,
            )
            .add_scope(Scope::new("openid".to_string()))
            .url();

        Ok(AuthUrlResult { url: auth_url })
    }

    #[instrument(level = "debug", skip(self, code, state))]
    pub async fn exchange_code(
        &self,
        code: &str,
        state: &str,
    ) -> Result<TokenExchangeResult, AppError> {
        let state_payload = decode_state_token(state, &self.state_secret)?;

        let jwks = fetch_jwk_set(self.jwks_uri.clone())
            .await
            .map_err(|_| AppError::OidcError("Failed to fetch JWKS".into()))?;

        let client = CoreClient::from_provider_metadata(
            self.metadata.clone(),
            self.client_id.clone(),
            Some(self.client_secret.clone()),
        )
        .set_redirect_uri(self.redirect_uri.clone());

        let token_response: CoreTokenResponse = client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .map_err(|e| AppError::OidcError(format!("Token exchange setup: {e:?}")))?
            .request_async(&self.http_client)
            .await
            .map_err(|e| {
                let msg = format!("Token exchange: {e:?}");
                debug!(error = msg, status = "failed");
                AppError::OidcError(msg)
            })?;

        let id_token = token_response
            .id_token()
            .ok_or_else(|| AppError::OidcError("No ID token in response".into()))?;

        let id_token_verifier = client.id_token_verifier();
        let nonce = Nonce::new(state_payload.nonce);
        let claims: &CoreIdTokenClaims = id_token
            .claims(&id_token_verifier, &nonce)
            .map_err(|e| AppError::OidcError(format!("ID token verification: {e:?}")))?;

        let sub = claims.subject().to_string();

        let access_token = From::from(token_response.access_token().secret().as_str());
        let sid = match verify_token::<AccessTokenClaims>(&access_token, &jwks, &self.issuer_url) {
            TokenStatus::Valid(claims) => claims.sid,
            _ => return Err(AppError::OidcError("Failed to verify access token".into())),
        };

        let id_claims = IdTokenClaims::new(sub, sid, claims);

        let refresh_token = From::from(
            token_response
                .refresh_token()
                .ok_or_else(|| AppError::OidcError("No refresh token".into()))?
                .secret()
                .as_str(),
        );
        let session_expires_at = get_expired_at_from_refresh_token(
            &refresh_token,
            self.default_expires_in * self.session_ttl_multiplier,
        )?;

        Ok(TokenExchangeResult::new(
            SecretBox::from(access_token),
            SecretBox::from(refresh_token),
            Some(id_claims),
            session_expires_at,
            state_payload.redirect,
        ))
    }

    #[instrument(level = "debug", skip(self, refresh_token))]
    pub async fn refresh_token(
        &self,
        refresh_token: &SecretBox<str>,
    ) -> Result<TokenExchangeResult, AppError> {
        let jwks = fetch_jwk_set(self.jwks_uri.clone())
            .await
            .map_err(|_| AppError::OidcError("Failed to fetch JWKS".into()))?;

        let client = CoreClient::from_provider_metadata(
            self.metadata.clone(),
            self.client_id.clone(),
            Some(self.client_secret.clone()),
        )
        .set_redirect_uri(self.redirect_uri.clone());

        let token_response: CoreTokenResponse = client
            .exchange_refresh_token(&RefreshToken::new(
                refresh_token.expose_secret().to_string(),
            ))
            .map_err(|e| AppError::OidcError(format!("Token refresh setup: {e:?}")))?
            .request_async(&self.http_client)
            .await
            .map_err(|e| {
                let msg = format!("Token refresh: {e:?}");
                debug!(error = msg, status = "failed");
                AppError::OidcError(msg)
            })?;

        let access_token = From::from(token_response.access_token().secret().as_str());
        match verify_token::<AccessTokenClaims>(&access_token, &jwks, &self.issuer_url) {
            TokenStatus::Valid(_) => {},
            _ => {
                return Err(AppError::OidcError(
                    "Failed to verify new access token".into(),
                ))
            },
        };

        let new_refresh_token = From::from(
            token_response
                .refresh_token()
                .ok_or_else(|| AppError::OidcError("No refresh token".into()))?
                .secret()
                .as_str(),
        );

        let session_expires_at = get_expired_at_from_refresh_token(
            &new_refresh_token,
            self.default_expires_in * self.session_ttl_multiplier,
        )?;

        Ok(TokenExchangeResult::new(
            SecretBox::from(access_token),
            new_refresh_token,
            None,
            session_expires_at,
            String::new(),
        ))
    }

    #[instrument(level = "debug", skip(self, token))]
    pub async fn verify_access_token(
        &self,
        token: &SecretBox<str>,
    ) -> TokenStatus<AccessTokenClaims> {
        let jwks = match fetch_jwk_set(self.jwks_uri.clone()).await {
            Ok(keys) => keys,
            Err(e) => return TokenStatus::Invalid(e.to_string()),
        };
        verify_token(token, &jwks, &self.issuer_url)
    }
}

fn get_expired_at_from_refresh_token(
    token: &SecretBox<str>,
    fallback_expiry: Duration,
) -> Result<NaiveDateTime, AppError> {
    match jsonwebtoken::dangerous::insecure_decode::<RefreshTokenClaims>(token.expose_secret()) {
        Ok(token_data) => Ok(token_data
            .claims
            .exp
            .and_then(|exp| chrono::DateTime::from_timestamp(exp, 0))
            .unwrap_or_else(|| chrono::Utc::now() + fallback_expiry)
            .naive_utc()),
        Err(e) => Err(AppError::OidcError(format!(
            "Refresh token parsing error: {}",
            e
        ))),
    }
}

fn verify_token<T: serde::de::DeserializeOwned>(
    token: &SecretBox<str>,
    jwks: &JwkSet,
    issuer_url: &str,
) -> TokenStatus<T> {
    let header = match jsonwebtoken::decode_header(token.expose_secret()) {
        Ok(h) => h,
        Err(e) => return TokenStatus::Invalid(e.to_string()),
    };

    let kid = match header.kid {
        Some(ref k) => k,
        None => return TokenStatus::Invalid(String::from("No key id in jwt header")),
    };

    let jwk = match jwks.find(kid) {
        Some(j) => j,
        None => return TokenStatus::Invalid(format!("Not found jwk key {} in key set", kid)),
    };

    let decoding_key = match DecodingKey::try_from(jwk) {
        Ok(k) => k,
        Err(e) => return TokenStatus::Invalid(e.to_string()),
    };

    let mut validation = Validation::new(header.alg);
    validation.set_issuer(&[issuer_url]);
    validation.validate_exp = true;
    validation.validate_aud = false;
    validation.required_spec_claims.clear();

    let result = match jsonwebtoken::decode::<T>(token.expose_secret(), &decoding_key, &validation)
    {
        Ok(data) => TokenStatus::Valid(data.claims),
        Err(e) => match e.kind() {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                match jsonwebtoken::dangerous::insecure_decode::<T>(token.expose_secret()) {
                    Ok(data) => TokenStatus::Expired(data.claims),
                    Err(e) => TokenStatus::Invalid(e.to_string()),
                }
            },
            _ => TokenStatus::Invalid(e.to_string()),
        },
    };

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYCLOAK_JWKS: &str = r#"{
  "keys": [
    {
      "kid": "KlugESBFgOxZPCC-qWGYsyPIkWo3txoaQ3CDyciTpDM",
      "kty": "RSA",
      "alg": "RS256",
      "use": "sig",
      "n": "xG48ztAY8JEy7Y3weRwpZ5UpZawMupSWm00PEuz3zqX66f_4WzHgto2Y3U6Bww0FtpE1dlALekog5x05UM18O_02DWsug7k5YXeGVo6KSD0tTEnIM2uOC6XczeH11v0sFDU_xHywxJToleCmUN1q5J37iJlH43lWcJcAn8j7UcZEqD8vvg4lriUkqyRIyyVQdTPEoSlWfplEMopH8OGiZwBQkjgZx9tvyifErYJ8XJMLNbTB0clMBDRDA5_D9FN6X7GYXZUgeFPBeQmUkLkNsZrAw-qBPOtlTpPRCQ3mh7S08LqIHXMCzJvDqsSQNsqkFI3IKupzSa3gf5JHJHqafQ",
      "e": "AQAB"
    }
  ]
}"#;

    const VALID_ACCESS_TOKEN: &str = "eyJhbGciOiJSUzI1NiIsInR5cCIgOiAiSldUIiwia2lkIiA6ICJLbHVnRVNCRmdPeFpQQ0MtcVdHWXN5UElrV28zdHhvYVEzQ0R5Y2lUcERNIn0.eyJleHAiOjE3ODAzMjIwNzEsImlhdCI6MTc4MDMyMTc3MSwiYXV0aF90aW1lIjoxNzgwMzIxNzExLCJqdGkiOiJvbnJ0YWM6ZDFjZGFjY2MtNjA2MS0xMGE3LWJiMGMtYTg2MTdmZWI2NzAwIiwiaXNzIjoiaHR0cDovL2xvY2FsaG9zdDo4MDgwL3JlYWxtcy9teXJlYWxtIiwiYXVkIjoiYWNjb3VudCIsInN1YiI6ImMxZjU3MjhlLTFjNmQtNGFhMS1iNDdjLTU1Mzk4YWNlOTUwYiIsInR5cCI6IkJlYXJlciIsImF6cCI6Im15Y2xpZW50LWZlIiwic2lkIjoiZVMxTmhGbGp5S2pkVERUdDY4Uk5RbUVwIiwiYWNyIjoiMCIsImFsbG93ZWQtb3JpZ2lucyI6WyJodHRwczovL3d3dy5rZXljbG9hay5vcmciXSwicmVhbG1fYWNjZXNzIjp7InJvbGVzIjpbImRlZmF1bHQtcm9sZXMtbXlyZWFsbSIsIm9mZmxpbmVfYWNjZXNzIiwidW1hX2F1dGhvcml6YXRpb24iXX0sInJlc291cmNlX2FjY2VzcyI6eyJhY2NvdW50Ijp7InJvbGVzIjpbIm1hbmFnZS1hY2NvdW50IiwibWFuYWdlLWFjY291bnQtbGlua3MiLCJ2aWV3LXByb2ZpbGUiXX19LCJzY29wZSI6Im9wZW5pZCBlbWFpbCBwcm9maWxlIiwiZW1haWxfdmVyaWZpZWQiOnRydWUsIm5hbWUiOiJ0ZXN0IHRlc3RlciIsInByZWZlcnJlZF91c2VybmFtZSI6InRlc3QiLCJnaXZlbl9uYW1lIjoidGVzdCIsImZhbWlseV9uYW1lIjoidGVzdGVyIiwiZW1haWwiOiJ0ZXN0QGxvY2FsLmxvY2FsIn0.CEAAH0FJev4jRlno2VHOlSzhJndwI9DXy3T2rd-ivJ1PzX0mQV1Y7pqXSpvqOALw7jY6BQhWTahZNoOokULwMsX6EYjyKL-Dc9d-w9xLH7uPPYTYcyrw4htqZXwzgJfYU-c_GerPnJkj95rmUX9Fwl9wgV24ZUIHH_9g4pFbBORrHf9GtLIXDkRNh9wWCrofQ_ILacj6nRzAjjZAHvi7hwjvNOVWmYd43Pg7N46Dy-XEyltScswUB_4TELynVbYkQ-UK_-wREWiAFgPMSEuNwpeVIrA4AeVW-ePMnPwwGNHMlqvZR6GhssbJX74HWsdaVk8FPqGTuGWJI4coy0lLEQ";

    #[test]
    fn find_missing_kid_returns_none() {
        let jwks: JwkSet = serde_json::from_str(KEYCLOAK_JWKS).unwrap();
        assert!(jwks.find("nonexistent").is_none());
    }

    #[test]
    fn verify_access_token_keycloak() {
        let jwks: JwkSet = serde_json::from_str(KEYCLOAK_JWKS).unwrap();
        let issuer = "http://localhost:8080/realms/myrealm";

        let status =
            verify_token::<AccessTokenClaims>(&SecretBox::from(VALID_ACCESS_TOKEN), &jwks, issuer);
        assert!(
            matches!(status, TokenStatus::Valid(_) | TokenStatus::Expired(_)),
            "Expected Valid or Expired, got {status:?}"
        );

        let wrong_issuer = verify_token::<AccessTokenClaims>(
            &SecretBox::from(VALID_ACCESS_TOKEN),
            &jwks,
            "http://wrong.issuer",
        );
        assert!(
            matches!(
                wrong_issuer,
                TokenStatus::Invalid(_) | TokenStatus::Expired(_)
            ),
            "Expected Invalid or Expired for wrong issuer, got {wrong_issuer:?}"
        );

        let wrong_kid = jwks.find("does-not-exist");
        assert!(wrong_kid.is_none());
    }
}
