use std::fs;
use std::future::Future;
use std::io::{BufReader, Read};
use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use futures::{stream, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub(crate) fn parse_metadata(raw: &Option<String>) -> Result<Value> {
    match raw {
        None => Ok(Value::Null),
        Some(text) => serde_json::from_str(text).context("invalid --metadata JSON"),
    }
}

pub(crate) fn parse_reindex_mode(mode: &str) -> Result<&str> {
    match mode {
        "compare" | "force" | "full" => Ok(mode),
        _ => Err(anyhow!(
            "invalid --mode '{mode}', expected compare|force|full"
        )),
    }
}

pub(crate) fn make_bar(mp: &MultiProgress, total: usize, prefix: &str) -> ProgressBar {
    let bar = mp.add(ProgressBar::new(total as u64));
    bar.set_style(
        ProgressStyle::with_template(
            "{prefix:>10} {percent:>3}%|{bar:40.cyan/blue}| {pos}/{len} \
             [{elapsed_precise}<{eta_precise}, {per_sec}] {msg}",
        )
        .expect("valid progress template"),
    );
    bar.set_prefix(prefix.to_string());
    bar.enable_steady_tick(Duration::from_millis(100));
    bar
}

pub(crate) fn sha256_hex(data: &[u8]) -> String {
    hex_digest(Sha256::digest(data).into())
}

pub(crate) fn file_sha256_hex(path: &Path) -> Result<String> {
    let file =
        fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let mut reader = BufReader::with_capacity(128 * 1024, file);
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 128 * 1024];
    loop {
        let n = reader
            .read(&mut buf)
            .with_context(|| format!("failed to read {}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_digest(hasher.finalize().into()))
}

fn hex_digest(digest: [u8; 32]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn file_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document")
        .to_string()
}

pub(crate) async fn map_concurrent<T, R, F, Fut>(
    items: impl IntoIterator<Item = T>,
    concurrency: usize,
    f: F,
) -> Vec<R>
where
    F: Fn(T) -> Fut,
    Fut: Future<Output = R>,
{
    stream::iter(items)
        .map(f)
        .buffer_unordered(concurrency)
        .collect()
        .await
}
