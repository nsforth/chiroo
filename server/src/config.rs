use std::env;

use dotenvy::dotenv;
use secrecy::{SecretBox, SecretString};
use tokio::sync::OnceCell;

#[derive(Debug)]
pub struct ServerConfig {
    host: String,
    port: u16,
}

#[derive(Debug)]
pub struct DatabaseConfig {
    url: String,
}

#[derive(Debug)]
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
    state_secret: SecretString,
    cleanup_interval_secs: u64,
    session_ttl_multiplier: u32,
    default_access_token_expires_secs: u64,
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

    pub fn state_secret(&self) -> &SecretBox<str> {
        &self.state_secret
    }

    pub fn cleanup_interval_secs(&self) -> u64 {
        self.cleanup_interval_secs
    }

    pub fn session_ttl_multiplier(&self) -> u32 {
        self.session_ttl_multiplier
    }

    pub fn default_access_token_expires_secs(&self) -> u64 {
        self.default_access_token_expires_secs
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

#[derive(Debug)]
pub struct Config {
    pub server: ServerConfig,
    pub db: DatabaseConfig,
    pub openai: OpenAiConfig,
    pub oidc: OidcConfig,
}

impl Config {
    pub fn db_url(&self) -> String {
        self.db.url.clone()
    }

    pub fn server_host(&self) -> String {
        self.server.host.clone()
    }

    pub fn server_port(&self) -> u16 {
        self.server.port
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
            .unwrap(),
    };

    let database_config = DatabaseConfig {
        url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
    };

    let openai_config = OpenAiConfig {
        base_url: env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".into()),
        api_key: SecretString::new(
            env::var("OPENAI_API_KEY")
                .unwrap_or_else(|_| "sk-no-key".into())
                .into_boxed_str(),
        ),
        model: env::var("OPENAI_MODEL").unwrap_or_else(|_| "LLM".into()),
    };

    let app_base_url = env::var("APP_BASE_URL").expect("APP_BASE_URL must be set");

    let oidc_config = OidcConfig {
        redirect_uri: format!("{app_base_url}/api/auth/oidc/callback"),
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
            .unwrap_or(600),
        session_ttl_multiplier: env::var("SESSION_TTL_MULTIPLIER")
            .unwrap_or_else(|_| "10".into())
            .parse()
            .unwrap_or(10),
        default_access_token_expires_secs: env::var("OIDC_ACCESS_TOKEN_EXPIRES_SECS")
            .unwrap_or_else(|_| "300".into())
            .parse()
            .unwrap_or(300),
    };

    Config {
        server: server_config,
        db: database_config,
        openai: openai_config,
        oidc: oidc_config,
    }
}

pub async fn config() -> &'static Config {
    CONFIG.get_or_init(init_config).await
}
