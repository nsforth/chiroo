use std::path::Path;

use anyhow::{Context, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub(crate) fn parse_metadata(raw: &Option<String>) -> Result<Value> {
    match raw {
        None => Ok(Value::Null),
        Some(text) => serde_json::from_str(text).context("invalid --metadata JSON"),
    }
}

pub(crate) fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub(crate) fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document")
        .to_string()
}
