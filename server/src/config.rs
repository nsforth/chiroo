use std::{env, sync::Arc, time::Duration};

use dotenvy::dotenv;
use secrecy::{SecretBox, SecretString};
use tokio::sync::OnceCell;

use crate::utils::crypto::KEY_LEN;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    host: String,
    port: u16,
}

#[derive(Debug, Clone)]
pub struct InternalApiConfig {
    host: String,
    port: u16,
}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    url: String,
    encryption_key: Arc<SecretBox<[u8; KEY_LEN]>>,
}

#[derive(Debug, Clone)]
pub struct OpenAiConfig {
    base_url: String,
    api_key: SecretString,
    model: String,
}

#[derive(Debug, Clone)]
pub struct OidcConfig {
    issuer_url: String,
    client_id: SecretString,
    client_secret: SecretString,
    redirect_uri: String,
    app_base_url: String,
    secure_cookie: bool,
    state_secret: SecretString,
    cleanup_interval_secs: Duration,
    session_ttl_multiplier: u32,
    default_access_token_expires_secs: Duration,
}

impl OidcConfig {
    pub fn issuer_url(&self) -> &str {
        &self.issuer_url
    }

    pub fn client_id(&self) -> &SecretString {
        &self.client_id
    }

    pub fn client_secret(&self) -> &SecretString {
        &self.client_secret
    }

    pub fn redirect_uri(&self) -> &str {
        &self.redirect_uri
    }

    pub fn app_base_url(&self) -> &str {
        &self.app_base_url
    }

    pub fn secure_cookie(&self) -> bool {
        self.secure_cookie
    }

    pub fn state_secret(&self) -> &SecretBox<str> {
        &self.state_secret
    }

    pub fn cleanup_interval_secs(&self) -> Duration {
        self.cleanup_interval_secs
    }

    pub fn session_ttl_multiplier(&self) -> u32 {
        self.session_ttl_multiplier
    }

    pub fn default_access_token_expires_secs(&self) -> Duration {
        self.default_access_token_expires_secs
    }
}

impl InternalApiConfig {
    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl OpenAiConfig {
    pub fn base_url(&self) -> String {
        self.base_url.clone()
    }

    pub fn api_key(&self) -> &SecretBox<str> {
        &self.api_key
    }

    pub fn model(&self) -> String {
        self.model.clone()
    }
}

#[derive(Debug, Clone)]
pub struct QdrantConfig {
    url: String,
    api_key: Option<String>,
    collection_prefix: String,
}

impl QdrantConfig {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref()
    }

    pub fn collection_prefix(&self) -> &str {
        &self.collection_prefix
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub internal_api: InternalApiConfig,
    pub db: DatabaseConfig,
    pub openai: OpenAiConfig,
    pub oidc: OidcConfig,
    pub qdrant: QdrantConfig,
}

impl Config {
    pub fn db_url(&self) -> String {
        self.db.url.clone()
    }

    pub fn db_encryption_key(&self) -> Arc<SecretBox<[u8; KEY_LEN]>> {
        Arc::clone(&self.db.encryption_key)
    }

    pub fn server_host(&self) -> String {
        self.server.host.clone()
    }

    pub fn server_port(&self) -> u16 {
        self.server.port
    }

    pub fn internal_api_host(&self) -> &str {
        self.internal_api.host()
    }

    pub fn internal_api_port(&self) -> u16 {
        self.internal_api.port()
    }

    pub fn oidc_config(&self) -> &OidcConfig {
        &self.oidc
    }
}

pub static CONFIG: OnceCell<Config> = OnceCell::const_new();

async fn init_config() -> Config {
    dotenv().ok();
    let server_config = ServerConfig {
        host: env::var("HOST").unwrap_or_else(|_| String::from("127.0.0.1")),
        port: env::var("PORT")
            .unwrap_or_else(|_| String::from("3000"))
            .parse::<u16>()
            .expect("PORT is not and integer"),
    };

    let internal_api_config = InternalApiConfig {
        host: env::var("INTERNAL_API_HOST").unwrap_or_else(|_| String::from("127.0.0.1")),
        port: env::var("INTERNAL_API_PORT")
            .unwrap_or_else(|_| String::from("3001"))
            .parse::<u16>()
            .expect("INTERNAL_API_PORT is not an integer"),
    };

    let raw_key = env::var("DATABASE_ENCRYPTION_KEY").expect("DATABASE_ENCRYPTION_KEY must be set");
    let key_bytes: [u8; KEY_LEN] = raw_key
        .as_bytes()
        .try_into()
        .unwrap_or_else(|_| panic!("DATABASE_ENCRYPTION_KEY must be exactly {KEY_LEN} bytes"));
    let database_config = DatabaseConfig {
        url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
        encryption_key: Arc::new(SecretBox::new(Box::new(key_bytes))),
    };

    let openai_config = OpenAiConfig {
        base_url: env::var("OPENAI_BASE_URL").expect("OPENAI_BASE_URL must be set"),
        api_key: SecretString::new(
            env::var("OPENAI_API_KEY")
                .expect("OPENAI_API_KEY must be set")
                .into_boxed_str(),
        ),
        model: env::var("OPENAI_MODEL").expect("OPENAI_MODEL must be set"),
    };

    let app_base_url = env::var("APP_BASE_URL").expect("APP_BASE_URL must be set");
    let secure_cookie = !app_base_url.starts_with("http://");

    let qdrant_config = QdrantConfig {
        url: env::var("QDRANT_URL").unwrap_or_else(|_| String::from("http://localhost:6334")),
        api_key: env::var("QDRANT_API_KEY").ok(),
        collection_prefix: env::var("QDRANT_COLLECTION_PREFIX")
            .unwrap_or_else(|_| String::from("chiroo")),
    };

    let oidc_config = OidcConfig {
        redirect_uri: format!("{app_base_url}/api/auth/oidc/callback"),
        secure_cookie,
        app_base_url,
        issuer_url: env::var("OIDC_ISSUER_URL").expect("OIDC_ISSUER_URL must be set"),
        client_id: SecretString::new(
            env::var("OIDC_CLIENT_ID")
                .expect("OIDC_CLIENT_ID must be set")
                .into_boxed_str(),
        ),
        client_secret: SecretString::new(
            env::var("OIDC_CLIENT_SECRET")
                .expect("OIDC_CLIENT_SECRET must be set")
                .into_boxed_str(),
        ),
        state_secret: SecretString::new(
            env::var("STATE_SECRET")
                .expect("STATE_SECRET must be set")
                .into_boxed_str(),
        ),
        cleanup_interval_secs: env::var("SESSION_CLEANUP_INTERVAL_SECS")
            .unwrap_or_else(|_| "600".into())
            .parse()
            .map(Duration::from_secs)
            .expect("Session cleanup interval must be positive integer in seconds"),
        session_ttl_multiplier: env::var("SESSION_TTL_MULTIPLIER")
            .unwrap_or_else(|_| "10".into())
            .parse()
            .expect("Session ttl multiplier must be positive integer"),
        default_access_token_expires_secs: env::var("OIDC_ACCESS_TOKEN_EXPIRES_SECS")
            .unwrap_or_else(|_| "300".into())
            .parse()
            .map(Duration::from_secs)
            .expect("oidc access token expires defult time must be positive integer in seconds"),
    };

    Config {
        server: server_config,
        internal_api: internal_api_config,
        db: database_config,
        openai: openai_config,
        oidc: oidc_config,
        qdrant: qdrant_config,
    }
}

pub async fn config() -> &'static Config {
    CONFIG.get_or_init(init_config).await
}
