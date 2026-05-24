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

    Config {
        server: server_config,
        db: database_config,
        openai: openai_config,
    }
}

pub async fn config() -> &'static Config {
    CONFIG.get_or_init(init_config).await
}
