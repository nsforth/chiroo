use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
    time::{Duration, UNIX_EPOCH},
};

use appconfig::{Case, Environment, File, FileFormat};
use dotenvy::dotenv;
use mime_guess::from_path;
use secrecy::{SecretBox, SecretString};
use serde::Deserialize;
use tracing::warn;

use crate::utils::crypto::KEY_LEN;

/// Branding asset files, shipped under `server/assets/branding/`. Paths are
/// fixed: rebranding means replacing the files in place. Only the CSS family
/// labels of the two fonts remain overridable via `CHIROO_BRANDING__FONT_FAMILY` /
/// `CHIROO_BRANDING__MONO_FONT_FAMILY`.
pub const SCENARIO_ICONS_DIR: &str = "assets/branding/scenarios";

const FONT_PATH: &str = "assets/branding/font.ttf";
const FONT_ITALIC_PATH: &str = "assets/branding/font-italic.ttf";
const MONO_FONT_PATH: &str = "assets/branding/font-mono.ttf";
const LOGO_PATH: &str = "assets/branding/logo.png";
const FAVICON_PATH: &str = "assets/branding/favicon.png";
const ASSISTANT_ICON_PATH: &str = "assets/branding/assistant.png";

/// Built-in default configuration, compiled into the binary. This is the
/// lowest-precedence layer; see `server/config/default.yaml` for the source.
const DEFAULT_CONFIG_YAML: &str = include_str!("../config/default.yaml");

fn is_hex_color(value: &str) -> bool {
    let Some(hex) = value.strip_prefix('#') else {
        return false;
    };
    (hex.len() == 3 || hex.len() == 6) && hex.chars().all(|c| c.is_ascii_hexdigit())
}

fn validate_hex_color(value: &str, var: &str) -> anyhow::Result<()> {
    if is_hex_color(value) {
        Ok(())
    } else {
        anyhow::bail!("{var} must be a hex color like #RRGGBB, got {value:?}")
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
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(e) => {
            warn!(path = ?path, error = e.to_string(), status = "failed");
            return None;
        },
    };
    let version = fs::metadata(path)
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

/// Loads a required brand font (main or mono) from its fixed bundled path. The
/// file always ships with the repo, so a missing/unreadable font aborts startup.
fn load_required_font(path: &str, family: &str) -> anyhow::Result<BrandingFont> {
    let path = PathBuf::from(path);
    let format = font_format(&path);
    let asset = load_static_asset(&path)
        .ok_or_else(|| anyhow::anyhow!("failed to load brand font from {}", path.display()))?;
    Ok(BrandingFont {
        family: family.to_string(),
        format,
        asset,
    })
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

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    host: String,
    port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InternalApiConfig {
    host: String,
    port: u16,
}

#[derive(Clone, Deserialize)]
pub struct DatabaseConfig {
    url: String,
    #[serde(deserialize_with = "de_db_key")]
    encryption_key: Arc<SecretBox<[u8; KEY_LEN]>>,
}

impl std::fmt::Debug for DatabaseConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatabaseConfig")
            .field("url", &"[REDACTED]")
            .field("encryption_key", &self.encryption_key)
            .finish()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenAiConfig {
    base_url: String,
    api_key: SecretString,
    model: String,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct OidcConfig {
    issuer_url: String,
    client_id: SecretString,
    client_secret: SecretString,
    app_base_url: String,
    state_secret: SecretString,
    #[serde(with = "humantime_serde")]
    cleanup_interval: Duration,
    session_ttl_multiplier: u32,
    #[serde(with = "humantime_serde")]
    access_token_expires: Duration,
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

    /// Derived from `app_base_url`; not a stored field.
    pub fn redirect_uri(&self) -> String {
        format!("{}/api/auth/oidc/callback", self.app_base_url)
    }

    pub fn app_base_url(&self) -> &str {
        &self.app_base_url
    }

    /// Derived from `app_base_url`: secure for anything that is not plain HTTP.
    pub fn secure_cookie(&self) -> bool {
        !self.app_base_url.starts_with("http://")
    }

    pub fn state_secret(&self) -> &SecretBox<str> {
        &self.state_secret
    }

    pub fn cleanup_interval(&self) -> Duration {
        self.cleanup_interval
    }

    pub fn session_ttl_multiplier(&self) -> u32 {
        self.session_ttl_multiplier
    }

    /// Fallback TTL used when the OIDC response carries no access-token expiry.
    pub fn access_token_expires(&self) -> Duration {
        self.access_token_expires
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

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct QdrantConfig {
    url: String,
    #[serde(default)]
    api_key: Option<String>,
    collection_prefix: String,
    #[serde(with = "humantime_serde")]
    timeout: Duration,
}

impl QdrantConfig {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn api_key(&self) -> Option<&str> {
        self.api_key.as_deref().filter(|s| !s.is_empty())
    }

    pub fn collection_prefix(&self) -> &str {
        &self.collection_prefix
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChunkingConfig {
    pub max_chunk_size: usize,
}

#[derive(Debug, Clone, Deserialize)]
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

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
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
    #[serde(deserialize_with = "de_branding")]
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

/// Raw branding values as they appear in the merged config tree. `name`,
/// `font_family` and `mono_font_family` are concrete because the embedded
/// default YAML always provides them; the rest are genuinely optional.
#[derive(Debug, Clone, Deserialize)]
struct RawBranding {
    name: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    tagline: Option<String>,
    #[serde(default)]
    assistant_name: Option<String>,
    font_family: String,
    mono_font_family: String,
    #[serde(default)]
    palette: Option<RawPalette>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct RawPalette {
    primary: Option<String>,
    neutral: Option<String>,
    primary_dark: Option<String>,
    neutral_dark: Option<String>,
}

/// Deserializes the database encryption key and rejects any length other than
/// exactly `KEY_LEN` bytes.
fn de_db_key<'de, D>(deserializer: D) -> Result<Arc<SecretBox<[u8; KEY_LEN]>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    let key = String::deserialize(deserializer)?;
    let bytes: [u8; KEY_LEN] = key.as_bytes().try_into().map_err(|_| {
        D::Error::custom(format!("db.encryption_key must be exactly {KEY_LEN} bytes"))
    })?;
    Ok(Arc::new(SecretBox::new(Box::new(bytes))))
}

/// Deserializes branding values and runs the branding-specific resolution
/// (asset loading, palette validation) that plain serde cannot express.
fn de_branding<'de, D>(deserializer: D) -> Result<BrandingConfig, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    let raw = RawBranding::deserialize(deserializer)?;
    BrandingConfig::try_from(raw).map_err(|e| D::Error::custom(e.to_string()))
}

/// Builds the merged configuration from the three layers (default YAML,
/// optional file, environment) and deserializes it directly into the runtime
/// [`Config`].
///
/// `env_override` is `Some(map)` only in tests; in production it is `None` and
/// the real process environment is used.
fn build_with_env(
    config_path: Option<&Path>,
    env_override: Option<HashMap<String, String>>,
) -> Config {
    let mut builder = appconfig::Config::builder()
        .add_source(File::from_str(DEFAULT_CONFIG_YAML, FileFormat::Yaml));

    if let Some(path) = config_path {
        builder = builder.add_source(File::with_name(&path.to_string_lossy()));
    }

    let mut env = Environment::with_prefix("CHIROO")
        .prefix_separator("_")
        .separator("__")
        .convert_case(Case::Snake);
    if let Some(map) = env_override {
        env = env.source(Some(map));
    }
    builder = builder.add_source(env);

    builder
        .build()
        .expect("failed to build server configuration")
        .try_deserialize()
        .expect("failed to deserialize server configuration")
}

impl TryFrom<RawPalette> for Option<BrandingPalette> {
    type Error = anyhow::Error;

    fn try_from(raw: RawPalette) -> Result<Self, Self::Error> {
        let primary = raw.primary.as_deref().filter(|s| !s.is_empty());
        let neutral = raw.neutral.as_deref().filter(|s| !s.is_empty());
        match (primary, neutral) {
            (None, None) => Ok(None),
            (Some(primary), Some(neutral)) => {
                validate_hex_color(primary, "branding.palette.primary")?;
                validate_hex_color(neutral, "branding.palette.neutral")?;
                let primary_dark = raw
                    .primary_dark
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(primary);
                validate_hex_color(primary_dark, "branding.palette.primary_dark")?;
                let neutral_dark = raw
                    .neutral_dark
                    .as_deref()
                    .filter(|s| !s.is_empty())
                    .unwrap_or(neutral);
                validate_hex_color(neutral_dark, "branding.palette.neutral_dark")?;
                Ok(Some(BrandingPalette {
                    primary: primary.to_string(),
                    neutral: neutral.to_string(),
                    primary_dark: primary_dark.to_string(),
                    neutral_dark: neutral_dark.to_string(),
                }))
            },
            _ => anyhow::bail!(
                "branding.palette.primary and branding.palette.neutral must be set \
                 together (both or neither)"
            ),
        }
    }
}

impl TryFrom<RawBranding> for BrandingConfig {
    type Error = anyhow::Error;

    fn try_from(raw: RawBranding) -> Result<Self, Self::Error> {
        let palette = raw
            .palette
            .map(Option::<BrandingPalette>::try_from)
            .transpose()?
            .flatten();
        let font = load_required_font(FONT_PATH, &raw.font_family)?;
        let font_italic = load_optional_italic(&font);
        let font_mono = load_required_font(MONO_FONT_PATH, &raw.mono_font_family)?;
        let assistant_name = raw
            .assistant_name
            .filter(|s| !s.is_empty())
            .unwrap_or(raw.name.clone());
        let label = raw.label;
        let tagline = raw.tagline.filter(|s| !s.is_empty());
        Ok(Self {
            name: raw.name,
            label,
            tagline,
            assistant_name,
            palette,
            logo: load_static_asset(Path::new(LOGO_PATH)),
            favicon: load_static_asset(Path::new(FAVICON_PATH)),
            assistant_icon: load_static_asset(Path::new(ASSISTANT_ICON_PATH)),
            font,
            font_italic,
            font_mono,
        })
    }
}

static CONFIG: OnceLock<Config> = OnceLock::new();

/// Loads and caches the server configuration.
///
/// Layering: embedded YAML defaults, then the optional `config_path` file
/// (`-c/--config`), then the `CHIROO_*` environment (populated from `.env`).
pub fn load_config(config_path: Option<&Path>) -> &'static Config {
    CONFIG.get_or_init(|| {
        dotenv().ok();
        build_with_env(config_path, None)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::ExposeSecret;

    fn required_env() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert(
            "CHIROO_DB__URL".into(),
            "postgres://user:pass@localhost/chiroo".into(),
        );
        m.insert("CHIROO_DB__ENCRYPTION_KEY".into(), "k".repeat(KEY_LEN));
        m.insert("CHIROO_OPENAI__BASE_URL".into(), "http://openai/v1".into());
        m.insert("CHIROO_OPENAI__API_KEY".into(), "openai-key".into());
        m.insert("CHIROO_OPENAI__MODEL".into(), "gpt-4o".into());
        m.insert(
            "CHIROO_EMBEDDING__BASE_URL".into(),
            "http://embed/v1".into(),
        );
        m.insert("CHIROO_EMBEDDING__API_KEY".into(), "embed-key".into());
        m.insert("CHIROO_EMBEDDING__MODEL".into(), "emb".into());
        m.insert("CHIROO_EMBEDDING__DIMENSIONS".into(), "8".into());
        m.insert("CHIROO_RERANK__BASE_URL".into(), "http://rerank/v1".into());
        m.insert("CHIROO_RERANK__API_KEY".into(), "rerank-key".into());
        m.insert("CHIROO_RERANK__MODEL".into(), "rerank".into());
        m.insert(
            "CHIROO_OIDC__APP_BASE_URL".into(),
            "https://app.example".into(),
        );
        m.insert("CHIROO_OIDC__ISSUER_URL".into(), "https://issuer".into());
        m.insert("CHIROO_OIDC__CLIENT_ID".into(), "cid".into());
        m.insert("CHIROO_OIDC__CLIENT_SECRET".into(), "csec".into());
        m.insert("CHIROO_OIDC__STATE_SECRET".into(), "ss".into());
        m
    }

    fn load(env: HashMap<String, String>) -> Config {
        build_with_env(None, Some(env))
    }

    #[test]
    fn defaults_are_used_when_unset() {
        let cfg = load(required_env());
        assert_eq!(cfg.server_port(), 3000);
        assert_eq!(cfg.server_host(), "127.0.0.1");
        assert_eq!(cfg.internal_api_port(), 3001);
        assert_eq!(cfg.qdrant.url(), "http://localhost:6334");
        assert_eq!(cfg.qdrant.collection_prefix(), "chiroo");
        assert_eq!(cfg.qdrant.timeout(), Duration::from_secs(30));
        assert_eq!(
            cfg.oidc_config().cleanup_interval(),
            Duration::from_secs(600)
        );
        assert_eq!(cfg.oidc_config().session_ttl_multiplier(), 10);
        assert_eq!(
            cfg.oidc_config().access_token_expires(),
            Duration::from_secs(300)
        );
        assert_eq!(cfg.chunking.max_chunk_size, 512);
        assert_eq!(cfg.search.limit, 10);
        assert_eq!(cfg.search.score_threshold, 0.5);
        assert_eq!(cfg.search.candidate_limit, 30);
        assert_eq!(cfg.branding.name, "ChiRoo");
        assert_eq!(cfg.branding.assistant_name, "ChiRoo");
    }

    #[test]
    fn env_overrides_defaults() {
        let mut env = required_env();
        env.insert("CHIROO_SERVER__PORT".into(), "4000".into());
        env.insert("CHIROO_SEARCH__LIMIT".into(), "25".into());
        env.insert("CHIROO_BRANDING__NAME".into(), "Acme".into());
        let cfg = load(env);
        assert_eq!(cfg.server_port(), 4000);
        assert_eq!(cfg.search.limit, 25);
        assert_eq!(cfg.branding.name, "Acme");
        assert_eq!(cfg.branding.assistant_name, "Acme");
    }

    #[test]
    fn duration_env_uses_serde_format() {
        let mut env = required_env();
        env.insert("CHIROO_QDRANT__TIMEOUT".into(), "1m".into());
        env.insert("CHIROO_OIDC__CLEANUP_INTERVAL".into(), "30s".into());
        env.insert("CHIROO_OIDC__ACCESS_TOKEN_EXPIRES".into(), "2min".into());
        let cfg = load(env);
        assert_eq!(cfg.qdrant.timeout(), Duration::from_secs(60));
        assert_eq!(
            cfg.oidc_config().cleanup_interval(),
            Duration::from_secs(30)
        );
        assert_eq!(
            cfg.oidc_config().access_token_expires(),
            Duration::from_secs(120)
        );
    }

    #[test]
    fn numeric_string_field_stays_a_string() {
        let mut env = required_env();
        env.insert("CHIROO_OPENAI__MODEL".into(), "7".into());
        env.insert("CHIROO_EMBEDDING__DIMENSIONS".into(), "512".into());
        let cfg = load(env);
        // `model` is a `String` and must not be coerced to an integer.
        assert_eq!(cfg.openai.model, "7");
        // `dimensions` is numeric and is still parsed from the string.
        assert_eq!(cfg.embedding.dimensions, 512);
    }

    #[test]
    fn embedding_and_rerank_use_their_own_values() {
        let cfg = load(required_env());
        assert_eq!(cfg.embedding.base_url, "http://embed/v1");
        assert_eq!(cfg.embedding.api_key.expose_secret(), "embed-key");
        assert_eq!(cfg.rerank.base_url, "http://rerank/v1");
        assert_eq!(cfg.rerank.api_key.expose_secret(), "rerank-key");
    }

    #[test]
    #[should_panic(expected = "missing configuration field \"embedding.base_url\"")]
    fn missing_embedding_base_url_panics() {
        let mut env = required_env();
        env.remove("CHIROO_EMBEDDING__BASE_URL");
        let _ = load(env);
    }

    #[test]
    #[should_panic(expected = "missing configuration field \"rerank.api_key\"")]
    fn missing_rerank_api_key_panics() {
        let mut env = required_env();
        env.remove("CHIROO_RERANK__API_KEY");
        let _ = load(env);
    }

    #[test]
    #[should_panic(expected = "unknown field `foo`")]
    fn unknown_top_level_section_rejected() {
        let mut env = required_env();
        // `foo` is not a known section (typo / stray CHIROO_* var).
        env.insert("CHIROO_FOO__BAR".into(), "1".into());
        let _ = load(env);
    }

    #[test]
    fn derived_oidc_fields() {
        let cfg = load(required_env());
        assert!(cfg.oidc_config().secure_cookie());
        assert_eq!(
            cfg.oidc_config().redirect_uri(),
            "https://app.example/api/auth/oidc/callback"
        );

        let mut env = required_env();
        env.insert(
            "CHIROO_OIDC__APP_BASE_URL".into(),
            "http://insecure.local".into(),
        );
        let cfg = load(env);
        assert!(!cfg.oidc_config().secure_cookie());
    }

    #[test]
    fn palette_defaults_and_validation() {
        let mut env = required_env();
        env.insert("CHIROO_BRANDING__PALETTE__PRIMARY".into(), "#112233".into());
        env.insert("CHIROO_BRANDING__PALETTE__NEUTRAL".into(), "#aabbcc".into());
        let cfg = load(env);
        let palette = cfg.branding.palette.expect("palette present");
        assert_eq!(palette.primary, "#112233");
        assert_eq!(palette.neutral, "#aabbcc");
        assert_eq!(palette.primary_dark, "#112233");
        assert_eq!(palette.neutral_dark, "#aabbcc");
    }

    #[test]
    fn label_empty_is_meaningful() {
        let cfg = load(required_env());
        assert!(cfg.branding.label.is_none());

        let mut env = required_env();
        env.insert("CHIROO_BRANDING__LABEL".into(), "Custom".into());
        let cfg = load(env);
        assert_eq!(cfg.branding.label.as_deref(), Some("Custom"));

        // An explicit empty string is meaningful (rendered, not hidden).
        let mut env = required_env();
        env.insert("CHIROO_BRANDING__LABEL".into(), "".into());
        let cfg = load(env);
        assert_eq!(cfg.branding.label.as_deref(), Some(""));
    }

    /// Removes a temp file on drop so a failing assert doesn't leave it behind.
    struct TempFile(PathBuf);
    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[test]
    fn file_overrides_default_and_is_overridden_by_env() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("chiroo_cfg_test_{}.yaml", std::process::id()));
        let temp = TempFile(path.clone());
        fs::write(&path, "server:\n  port: 3500\n").expect("write test config file");

        // File overrides the built-in default.
        let cfg = build_with_env(Some(&path), Some(required_env()));
        assert_eq!(cfg.server_port(), 3500);

        // Environment beats the file.
        let mut env = required_env();
        env.insert("CHIROO_SERVER__PORT".into(), "4000".into());
        let cfg = build_with_env(Some(&path), Some(env));
        assert_eq!(cfg.server_port(), 4000);

        drop(temp);
    }

    #[test]
    #[should_panic(expected = "missing configuration field \"db.url\"")]
    fn missing_required_field_panics() {
        let mut env = required_env();
        env.remove("CHIROO_DB__URL");
        let _ = load(env);
    }

    #[test]
    #[should_panic(expected = "must be exactly")]
    fn wrong_key_length_panics() {
        let mut env = required_env();
        env.insert("CHIROO_DB__ENCRYPTION_KEY".into(), "short".into());
        let _ = load(env);
    }

    #[test]
    #[should_panic(expected = "must be set together")]
    fn palette_requires_both_colors() {
        let mut env = required_env();
        env.insert("CHIROO_BRANDING__PALETTE__PRIMARY".into(), "#112233".into());
        let _ = load(env);
    }
}
