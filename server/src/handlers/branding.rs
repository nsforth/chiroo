use axum::extract::{Path as AxumPath, State};
use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use crate::config::{BrandingConfig, BrandingFont, BrandingPalette, StaticAsset};
use crate::errors::AppError;
use crate::AppState;

const ASSET_CACHE_CONTROL: &str = "public, max-age=86400";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontResponse {
    pub family: String,
    pub url: String,
    pub format: Option<&'static str>,
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
    pub label: Option<String>,
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
        label: branding.label.clone(),
        tagline: branding.tagline.clone(),
        assistant_name: branding.assistant_name.clone(),
        colors: branding.palette.as_ref().map(PaletteResponse::from),
        logo_url: asset_url("logo", branding.logo.as_ref().map(|a| a.version)),
        favicon_url: asset_url("favicon", branding.favicon.as_ref().map(|a| a.version)),
        assistant_url: asset_url(
            "assistant",
            branding.assistant_icon.as_ref().map(|a| a.version),
        ),
        font_sans: branding
            .sans_font
            .as_ref()
            .map(|f| font_response(f, "font-sans")),
        font_mono: branding
            .mono_font
            .as_ref()
            .map(|f| font_response(f, "font-mono")),
    };
    let headers = [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))];
    (headers, Json(response))
}

fn asset_url(kind: &str, version: Option<u64>) -> String {
    match version {
        Some(version) => format!("/api/branding/assets/{kind}?v={version}"),
        None => format!("/api/branding/assets/{kind}"),
    }
}

fn font_response(font: &BrandingFont, kind: &str) -> FontResponse {
    FontResponse {
        family: font.family.clone(),
        url: asset_url(kind, Some(font.asset.version)),
        format: font.format,
    }
}

pub(crate) fn serve_asset(asset: &StaticAsset) -> Response {
    let content_type = HeaderValue::from_str(&asset.content_type)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    let headers = [
        (header::CONTENT_TYPE, content_type),
        (
            header::CACHE_CONTROL,
            HeaderValue::from_static(ASSET_CACHE_CONTROL),
        ),
    ];
    (headers, asset.data.clone()).into_response()
}

#[derive(Clone, Copy)]
enum BrandingAssetKind {
    Logo,
    Favicon,
    Assistant,
    FontSans,
    FontMono,
}

impl BrandingAssetKind {
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

    fn asset(self, branding: &BrandingConfig) -> Option<&StaticAsset> {
        match self {
            Self::Logo => branding.logo.as_ref(),
            Self::Favicon => branding.favicon.as_ref(),
            Self::Assistant => branding.assistant_icon.as_ref(),
            Self::FontSans => branding.sans_font.as_ref().map(|f| &f.asset),
            Self::FontMono => branding.mono_font.as_ref().map(|f| &f.asset),
        }
    }
}

pub async fn get_branding_asset(
    State(state): State<AppState>,
    AxumPath(kind): AxumPath<String>,
) -> Result<Response, AppError> {
    let asset = BrandingAssetKind::from_kind(&kind)
        .ok_or_else(|| AppError::NotFound(format!("Unknown branding asset: {kind}")))?
        .asset(&state.config.branding)
        .ok_or_else(|| AppError::NotFound(format!("Branding asset {kind} is not configured")))?;

    Ok(serve_asset(asset))
}
