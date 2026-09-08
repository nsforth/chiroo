use axum::extract::{Path as AxumPath, State};
use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use super::serve_asset;
use crate::config::{BrandingConfig, BrandingFont, BrandingPalette, StaticAsset};
use crate::errors::AppError;
use crate::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontResponse {
    pub family: String,
    pub url: String,
    pub format: Option<&'static str>,
}

impl FontResponse {
    fn from_font(font: &BrandingFont, kind: BrandingAssetKind) -> Self {
        Self {
            family: font.family.clone(),
            url: kind.url(Some(&font.asset)),
            format: font.format,
        }
    }
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
    pub font_normal: FontResponse,
    pub font_italic: Option<FontResponse>,
    pub font_mono: FontResponse,
}

impl From<&BrandingConfig> for BrandingResponse {
    fn from(branding: &BrandingConfig) -> Self {
        Self {
            name: branding.name.clone(),
            label: branding.label.clone(),
            tagline: branding.tagline.clone(),
            assistant_name: branding.assistant_name.clone(),
            colors: branding.palette.as_ref().map(PaletteResponse::from),
            logo_url: BrandingAssetKind::Logo.url(branding.logo.as_ref()),
            favicon_url: BrandingAssetKind::Favicon.url(branding.favicon.as_ref()),
            assistant_url: BrandingAssetKind::Assistant.url(branding.assistant_icon.as_ref()),
            font_normal: FontResponse::from_font(&branding.font, BrandingAssetKind::Font),
            font_italic: branding
                .font_italic
                .as_ref()
                .map(|f| FontResponse::from_font(f, BrandingAssetKind::FontItalic)),
            font_mono: FontResponse::from_font(&branding.font_mono, BrandingAssetKind::FontMono),
        }
    }
}

pub async fn get_branding(State(state): State<AppState>) -> impl IntoResponse {
    let response = BrandingResponse::from(&state.config.branding);
    let headers = [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))];
    (headers, Json(response))
}

#[derive(Clone, Copy)]
enum BrandingAssetKind {
    Logo,
    Favicon,
    Assistant,
    Font,
    FontItalic,
    FontMono,
}

impl BrandingAssetKind {
    fn from_kind(kind: &str) -> Option<Self> {
        match kind {
            "logo" => Some(Self::Logo),
            "favicon" => Some(Self::Favicon),
            "assistant" => Some(Self::Assistant),
            "font" => Some(Self::Font),
            "font-italic" => Some(Self::FontItalic),
            "font-mono" => Some(Self::FontMono),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Logo => "logo",
            Self::Favicon => "favicon",
            Self::Assistant => "assistant",
            Self::Font => "font",
            Self::FontItalic => "font-italic",
            Self::FontMono => "font-mono",
        }
    }

    fn url(self, asset: Option<&StaticAsset>) -> String {
        let path = format!("/api/branding/assets/{}", self.as_str());
        match asset.map(|a| a.version) {
            Some(version) => format!("{path}?v={version}"),
            None => path,
        }
    }

    fn asset(self, branding: &BrandingConfig) -> Option<&StaticAsset> {
        match self {
            Self::Logo => branding.logo.as_ref(),
            Self::Favicon => branding.favicon.as_ref(),
            Self::Assistant => branding.assistant_icon.as_ref(),
            Self::Font => Some(&branding.font.asset),
            Self::FontItalic => branding.font_italic.as_ref().map(|f| &f.asset),
            Self::FontMono => Some(&branding.font_mono.asset),
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
