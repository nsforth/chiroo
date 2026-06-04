use axum::http::{header, HeaderMap};
use cookie::Cookie;

pub fn extract_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in cookie_header.split(';').map(|s| s.trim()) {
        if let Ok(cookie) = Cookie::parse(part) {
            if cookie.name() == name {
                return Some(cookie.value_trimmed().to_string());
            }
        }
    }
    None
}
