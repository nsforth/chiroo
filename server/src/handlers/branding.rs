use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use axum::extract::{Path as AxumPath, State};
use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::Json;
use mime_guess::from_path;
use serde::Serialize;
use tokio::fs;
use tracing::warn;

use crate::config::{BrandingConfig, BrandingPalette};
use crate::errors::AppError;
use crate::AppState;

const ASSET_CACHE_CONTROL: &str = "public, max-age=86400";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontResponse {
    pub family: String,
    pub url: String,
    pub format: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaletteResponse {
    pub primary: String,
    pub neutral: String,
    pub primary_dark: String,
    pub neutral_dark: String,
}

impl From<&BrandingPalette> for PaletteResponse {
    fn from(palette: &BrandingPalette) -> Self {
        Self {
            primary: palette.primary.clone(),
            neutral: palette.neutral.clone(),
            primary_dark: palette.primary_dark.clone(),
            neutral_dark: palette.neutral_dark.clone(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrandingResponse {
    pub name: String,
    pub tagline: Option<String>,
    pub assistant_name: String,
    pub colors: Option<PaletteResponse>,
    pub logo_url: String,
    pub favicon_url: String,
    pub assistant_url: String,
    pub font_sans: Option<FontResponse>,
    pub font_mono: Option<FontResponse>,
}

pub async fn get_branding(State(state): State<AppState>) -> impl IntoResponse {
    let branding = &state.config.branding;
    let response = BrandingResponse {
        name: branding.name.clone(),
        tagline: branding.tagline.clone(),
        assistant_name: branding.assistant_name.clone(),
        colors: branding.palette.as_ref().map(PaletteResponse::from),
        logo_url: asset_url("logo", &branding.logo_path).await,
        favicon_url: asset_url("favicon", &branding.favicon_path).await,
        assistant_url: asset_url("assistant", &branding.assistant_icon_path).await,
        font_sans: font_response(
            &branding.sans_font_family,
            branding.sans_font_path.as_deref(),
            "font-sans",
        )
        .await,
        font_mono: font_response(
            &branding.mono_font_family,
            branding.mono_font_path.as_deref(),
            "font-mono",
        )
        .await,
    };
    let headers = [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))];
    (headers, Json(response))
}

pub(crate) async fn file_version(path: &Path) -> Option<u64> {
    fs::metadata(path)
        .await
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
}

pub(crate) async fn asset_url(kind: &str, path: &Path) -> String {
    let base = format!("/api/branding/assets/{kind}");
    match file_version(path).await {
        Some(version) => format!("{base}?v={version}"),
        None => base,
    }
}

pub(crate) async fn serve_asset_file(kind: &str, path: &Path) -> Result<Response, AppError> {
    let bytes = match fs::read(path).await {
        Ok(bytes) => bytes,
        Err(e) => {
            warn!(path = ?path, error = e.to_string(), status = "failed");
            return Err(AppError::NotFound(format!("Asset {kind} is unavailable")));
        },
    };

    let content_type = from_path(path).first_or_octet_stream();

    let headers = [
        (
            header::CONTENT_TYPE,
            HeaderValue::from_str(content_type.as_ref())
                .map_err(|_| AppError::InternalServerError)?,
        ),
        (
            header::CACHE_CONTROL,
            HeaderValue::from_static(ASSET_CACHE_CONTROL),
        ),
    ];

    Ok((headers, bytes).into_response())
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

async fn font_response(
    family: &Option<String>,
    path: Option<&Path>,
    kind: &str,
) -> Option<FontResponse> {
    let family = family.as_ref()?;
    let path = path?;
    Some(FontResponse {
        family: family.clone(),
        url: asset_url(kind, path).await,
        format: font_format(path).map(String::from),
    })
}

#[derive(Clone, Copy)]
enum BrandingAsset {
    Logo,
    Favicon,
    Assistant,
    FontSans,
    FontMono,
}

impl BrandingAsset {
    fn from_kind(kind: &str) -> Option<Self> {
        match kind {
            "logo" => Some(Self::Logo),
            "favicon" => Some(Self::Favicon),
            "assistant" => Some(Self::Assistant),
            "font-sans" => Some(Self::FontSans),
            "font-mono" => Some(Self::FontMono),
            _ => None,
        }
    }

    fn path(self, branding: &BrandingConfig) -> Option<&PathBuf> {
        match self {
            Self::Logo => Some(&branding.logo_path),
            Self::Favicon => Some(&branding.favicon_path),
            Self::Assistant => Some(&branding.assistant_icon_path),
            Self::FontSans => branding.sans_font_path.as_ref(),
            Self::FontMono => branding.mono_font_path.as_ref(),
        }
    }
}

pub async fn get_branding_asset(
    State(state): State<AppState>,
    AxumPath(kind): AxumPath<String>,
) -> Result<Response, AppError> {
    let asset = BrandingAsset::from_kind(&kind)
        .ok_or_else(|| AppError::NotFound(format!("Unknown branding asset: {kind}")))?;

    let path = asset
        .path(&state.config.branding)
        .ok_or_else(|| AppError::NotFound(format!("Branding asset {kind} is not configured")))?;

    serve_asset_file(&kind, path).await
}
