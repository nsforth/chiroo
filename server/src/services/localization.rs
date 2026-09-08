use std::collections::HashMap;
use std::fs;

use anyhow::{Context, Result};
use serde_json::Value;

/// Loads all localization bundles from `dir`. Each `*.json` file becomes an
/// entry keyed by its language code (the file stem), holding the raw parsed
/// content. The loader knows nothing about the inner structure — it serves the
/// files as-is.
pub fn load(dir: &str) -> Result<HashMap<String, Value>> {
    let entries = fs::read_dir(dir).with_context(|| format!("failed to read {dir}"))?;

    let mut by_lang = HashMap::new();
    for entry in entries {
        let path = entry
            .with_context(|| format!("failed to read entry in {dir}"))?
            .path();

        let lang = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .with_context(|| format!("invalid localization file name: {}", path.display()))?
            .to_string();

        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let value: Value = serde_json::from_str(&content)
            .with_context(|| format!("invalid JSON in {}", path.display()))?;

        by_lang.insert(lang, value);
    }

    Ok(by_lang)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    #[test]
    fn loads_all_language_files() {
        let dir = tempfile_dir();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("en.json"),
            r#"{ "scenarios": { "simple": { "label": "Fast" } } }"#,
        )
        .unwrap();
        std::fs::write(dir.join("ru.json"), r#"{ "scenarios": {} }"#).unwrap();

        let localizations = load(dir.to_str().unwrap()).unwrap();

        assert_eq!(localizations.len(), 2);
        assert!(localizations.contains_key("en"));
        assert!(localizations.contains_key("ru"));
    }

    #[test]
    fn rejects_invalid_json() {
        let dir = tempfile_dir();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("en.json"), "{ not json").unwrap();

        assert!(load(dir.to_str().unwrap()).is_err());
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("chiroo-localizations-{}-{id}", std::process::id()))
    }
}
