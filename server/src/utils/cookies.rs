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

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    fn headers(cookie: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_str(cookie).unwrap());
        headers
    }

    #[test]
    fn extracts_single_cookie() {
        assert_eq!(extract_cookie(&headers("a=1"), "a"), Some("1".into()));
    }

    #[test]
    fn finds_cookie_in_any_position() {
        assert_eq!(
            extract_cookie(&headers("a=1; b=2; c=3"), "b"),
            Some("2".into())
        );
    }

    #[test]
    fn missing_cookie_header_is_none() {
        let headers = HeaderMap::new();
        assert_eq!(extract_cookie(&headers, "a"), None);
    }

    #[test]
    fn missing_cookie_name_is_none() {
        assert_eq!(extract_cookie(&headers("a=1; b=2"), "c"), None);
    }

    #[test]
    fn empty_value_is_returned() {
        assert_eq!(extract_cookie(&headers("a="), "a"), Some("".into()));
    }

    #[test]
    fn value_whitespace_is_trimmed() {
        assert_eq!(
            extract_cookie(&headers("a=  hello "), "a"),
            Some("hello".into())
        );
    }
}
