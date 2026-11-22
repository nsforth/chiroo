use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};

use crate::config::StaticAsset;

pub mod admin;
pub mod auth;
pub mod branding;
pub mod chats;
pub mod internal;
pub mod kb;
pub mod localization;
pub mod models;
pub mod openai;
pub mod session;
pub mod user;

const ASSET_CACHE_CONTROL: &str = "public, max-age=86400";

pub fn serve_asset(asset: &StaticAsset) -> Response {
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
