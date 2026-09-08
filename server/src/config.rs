use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, UNIX_EPOCH},
};

use dotenvy::dotenv;
use mime_guess::from_path;
use secrecy::{SecretBox, SecretString};
use tokio::sync::OnceCell;
use tracing::warn;

use crate::utils::crypto::KEY_LEN;

/// Branding asset files, shipped under `server/assets/branding/`. Paths are
/// fixed: rebranding means replacing the files in place. Only the CSS family
/// labels of the two fonts remain overridable via `BRAND_FONT_FAMILY` /
/// `BRAND_MONO_FONT_FAMILY`.
pub const SCENARIO_ICONS_DIR: &str = "assets/branding/scenarios";

const FONT_FAMILY: &str = "Noto Sans";
const FONT_PATH: &str = "assets/branding/font.ttf";
const FONT_ITALIC_PATH: &str = "assets/branding/font-italic.ttf";
const MONO_FONT_FAMILY: &str = "Source Code Pro";
const MONO_FONT_PATH: &str = "assets/branding/font-mono.ttf";
const LOGO_PATH: &str = "assets/branding/logo.png";
const FAVICON_PATH: &str = "assets/branding/favicon.png";
const ASSISTANT_ICON_PATH: &str = "assets/branding/assistant.png";

fn is_hex_color(value: &str) -> bool {
    let Some(hex) = value.strip_prefix('#') else {
        return false;
    };
    (hex.len() == 3 || hex.len() == 6) && hex.chars().all(|c| c.is_ascii_hexdigit())
}

fn validate_hex_color(value: &str, var: &str) {
    if !is_hex_color(value) {
        panic!("{var} must be a hex color like #RRGGBB, got {value:?}");
    }
}

fn font_format(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()? {
        "woff2" => Some("woff2"),
        "woff" => Some("woff"),
        "ttf" => Some("truetype"),
        "otf" => Some("opentype"),
        _ => None,
    }
}

pub(crate) fn load_static_asset(path: &Path) -> Option<StaticAsset> {
    let data = match std::fs::read(path) {
        Ok(data) => data,
        Err(e) => {
            warn!(path = ?path, error = e.to_string(), status = "failed");
            return None;
        },
    };
    let version = std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())?;
    let content_type = from_path(path).first_or_octet_stream().to_string();
    Some(StaticAsset {
        data,
        content_type,
        version,
    })
}

fn font_family(family_var: &str, default_family: &str) -> String {
    env::var(family_var)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default_family.to_string())
}

/// Loads a required brand font (main or mono) from its fixed bundled path. The
/// file always ships with the repo, so a missing/unreadable font aborts startup.
fn load_required_font(path: &str, family: &str) -> BrandingFont {
    let path = PathBuf::from(path);
    let format = font_format(&path);
    let asset = load_static_asset(&path)
        .unwrap_or_else(|| panic!("failed to load brand font from {}", path.display()));
    BrandingFont {
        family: family.to_string(),
        format,
        asset,
    }
}

/// Loads the optional italic face for the main font, purely by file presence:
/// if `assets/branding/font-italic.ttf` exists it is used, otherwise there is
/// no italic (the browser synthesizes it). It shares the main font's family.
fn load_optional_italic(main: &BrandingFont) -> Option<BrandingFont> {
    let path = Path::new(FONT_ITALIC_PATH);
    let format = font_format(path);
    load_static_asset(path).map(|asset| BrandingFont {
        family: main.family.clone(),
        format,
        asset,
    })
}

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
pub struct EmbeddingConfig {
    pub base_url: String,
    pub api_key: SecretString,
    pub model: String,
    pub dimensions: u32,
}

impl EmbeddingConfig {
    pub fn base_url(&self) -> String {
        self.base_url.clone()
    }

    pub fn api_key(&self) -> &SecretBox<str> {
        &self.api_key
    }

    pub fn model(&self) -> String {
        self.model.clone()
    }

    pub fn dimensions(&self) -> u32 {
        self.dimensions
    }
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
pub struct RerankConfig {
    pub base_url: String,
    pub api_key: SecretString,
    pub model: String,
}

impl RerankConfig {
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
    timeout: Duration,
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

    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

#[derive(Debug, Clone)]
pub struct ChunkingConfig {
    pub max_chunk_size: usize,
}

#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub limit: usize,
    pub score_threshold: f32,
    pub candidate_limit: u64,
}

#[derive(Debug, Clone)]
pub struct BrandingPalette {
    pub primary: String,
    pub neutral: String,
    pub primary_dark: String,
    pub neutral_dark: String,
}

#[derive(Clone)]
pub struct StaticAsset {
    pub data: Vec<u8>,
    pub content_type: String,
    pub version: u64,
}

impl std::fmt::Debug for StaticAsset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StaticAsset")
            .field("content_type", &self.content_type)
            .field("version", &self.version)
            .field("data_len", &self.data.len())
            .finish()
    }
}

#[derive(Debug, Clone)]
pub struct BrandingFont {
    pub family: String,
    pub format: Option<&'static str>,
    pub asset: StaticAsset,
}

#[derive(Debug, Clone)]
pub struct BrandingConfig {
    pub name: String,
    pub label: Option<String>,
    pub tagline: Option<String>,
    pub assistant_name: String,
    pub palette: Option<BrandingPalette>,
    pub logo: Option<StaticAsset>,
    pub favicon: Option<StaticAsset>,
    pub assistant_icon: Option<StaticAsset>,
    pub font: BrandingFont,
    pub font_italic: Option<BrandingFont>,
    pub font_mono: BrandingFont,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub internal_api: InternalApiConfig,
    pub db: DatabaseConfig,
    pub openai: OpenAiConfig,
    pub embedding: EmbeddingConfig,
    pub oidc: OidcConfig,
    pub qdrant: QdrantConfig,
    pub rerank: RerankConfig,
    pub chunking: ChunkingConfig,
    pub search: SearchConfig,
    pub branding: BrandingConfig,
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

    let openai_base_url = env::var("OPENAI_BASE_URL").expect("OPENAI_BASE_URL must be set");
    let openai_api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set");

    let openai_config = OpenAiConfig {
        base_url: openai_base_url.clone(),
        api_key: SecretString::new(openai_api_key.clone().into_boxed_str()),
        model: env::var("OPENAI_MODEL").expect("OPENAI_MODEL must be set"),
    };

    let embedding_config = EmbeddingConfig {
        base_url: env::var("EMBEDDING_BASE_URL").unwrap_or(openai_base_url.clone()),
        api_key: SecretString::new(
            env::var("EMBEDDING_API_KEY")
                .unwrap_or(openai_api_key.clone())
                .into_boxed_str(),
        ),
        model: env::var("EMBEDDING_MODEL").expect("EMBEDDING_MODEL must be set"),
        dimensions: env::var("EMBEDDING_DIMENSIONS")
            .expect("EMBEDDING_DIMENSIONS must be set")
            .parse()
            .expect("EMBEDDING_DIMENSIONS must be a positive integer"),
    };

    let rerank_config = RerankConfig {
        base_url: env::var("RERANK_BASE_URL").unwrap_or(openai_base_url),
        api_key: SecretString::new(
            env::var("RERANK_API_KEY")
                .unwrap_or(openai_api_key)
                .into_boxed_str(),
        ),
        model: env::var("RERANK_MODEL").expect("RERANK_MODEL must be set"),
    };

    let app_base_url = env::var("APP_BASE_URL").expect("APP_BASE_URL must be set");
    let secure_cookie = !app_base_url.starts_with("http://");

    let qdrant_config = QdrantConfig {
        url: env::var("QDRANT_URL").unwrap_or_else(|_| String::from("http://localhost:6334")),
        api_key: env::var("QDRANT_API_KEY").ok(),
        collection_prefix: env::var("QDRANT_COLLECTION_PREFIX")
            .unwrap_or_else(|_| String::from("chiroo")),
        timeout: env::var("QDRANT_TIMEOUT_SECS")
            .unwrap_or_else(|_| "30".into())
            .parse()
            .map(Duration::from_secs)
            .expect("QDRANT_TIMEOUT_SECS must be positive integer in seconds"),
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

    let chunking_config = ChunkingConfig {
        max_chunk_size: env::var("CHUNK_SIZE")
            .unwrap_or_else(|_| "512".into())
            .parse()
            .expect("CHUNK_SIZE must be a positive integer"),
    };

    let search_config = SearchConfig {
        limit: env::var("SEARCH_LIMIT")
            .unwrap_or_else(|_| "10".into())
            .parse()
            .expect("SEARCH_LIMIT must be a positive integer"),
        score_threshold: env::var("SEARCH_SCORE_THRESHOLD")
            .unwrap_or_else(|_| "0.5".into())
            .parse()
            .expect("SEARCH_SCORE_THRESHOLD must be a float"),
        candidate_limit: env::var("SEARCH_CANDIDATE_LIMIT")
            .unwrap_or_else(|_| "30".into())
            .parse()
            .expect("SEARCH_CANDIDATE_LIMIT must be a positive integer"),
    };

    let brand_name = env::var("BRAND_NAME").unwrap_or_else(|_| String::from("ChiRoo"));
    let brand_tagline = env::var("BRAND_TAGLINE").ok().filter(|t| !t.is_empty());

    let brand_primary = env::var("BRAND_PRIMARY_COLOR")
        .ok()
        .filter(|v| !v.is_empty());
    let brand_neutral = env::var("BRAND_NEUTRAL_COLOR")
        .ok()
        .filter(|v| !v.is_empty());
    let brand_palette = match (brand_primary, brand_neutral) {
        (None, None) => None,
        (Some(primary), Some(neutral)) => {
            validate_hex_color(&primary, "BRAND_PRIMARY_COLOR");
            validate_hex_color(&neutral, "BRAND_NEUTRAL_COLOR");
            let primary_dark = env::var("BRAND_PRIMARY_DARK_COLOR")
                .ok()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| primary.clone());
            validate_hex_color(&primary_dark, "BRAND_PRIMARY_DARK_COLOR");
            let neutral_dark = env::var("BRAND_NEUTRAL_DARK_COLOR")
                .ok()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| neutral.clone());
            validate_hex_color(&neutral_dark, "BRAND_NEUTRAL_DARK_COLOR");
            Some(BrandingPalette {
                primary,
                neutral,
                primary_dark,
                neutral_dark,
            })
        },
        _ => {
            panic!(
                "BRAND_PRIMARY_COLOR and BRAND_NEUTRAL_COLOR must be set together \
                 (both or neither)"
            );
        },
    };

    let font = load_required_font(FONT_PATH, &font_family("BRAND_FONT_FAMILY", FONT_FAMILY));
    let font_italic = load_optional_italic(&font);
    let font_mono = load_required_font(
        MONO_FONT_PATH,
        &font_family("BRAND_MONO_FONT_FAMILY", MONO_FONT_FAMILY),
    );

    let assistant_name = env::var("BRAND_ASSISTANT_NAME").unwrap_or_else(|_| brand_name.clone());
    let label = env::var("BRAND_LABEL").ok();

    let branding_config = BrandingConfig {
        name: brand_name,
        label,
        tagline: brand_tagline,
        assistant_name,
        palette: brand_palette,
        logo: load_static_asset(Path::new(LOGO_PATH)),
        favicon: load_static_asset(Path::new(FAVICON_PATH)),
        assistant_icon: load_static_asset(Path::new(ASSISTANT_ICON_PATH)),
        font,
        font_italic,
        font_mono,
    };

    Config {
        server: server_config,
        internal_api: internal_api_config,
        db: database_config,
        openai: openai_config,
        embedding: embedding_config,
        rerank: rerank_config,
        oidc: oidc_config,
        qdrant: qdrant_config,
        chunking: chunking_config,
        search: search_config,
        branding: branding_config,
    }
}

pub async fn config() -> &'static Config {
    CONFIG.get_or_init(init_config).await
}
