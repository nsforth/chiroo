use std::fs;
use std::future::Future;
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use futures::{stream, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::signal::unix::{signal as unix_signal, SignalKind};

pub(crate) fn parse_metadata(raw: &Option<String>) -> Result<Value> {
    match raw {
        None => Ok(Value::Null),
        Some(text) => serde_json::from_str(text).context("invalid --metadata JSON"),
    }
}

pub(crate) fn parse_indexes(raw: &Option<String>) -> Result<Option<Value>> {
    match raw {
        None => Ok(None),
        Some(text) => {
            let value: Value = serde_json::from_str(text).context("invalid --indexes JSON")?;
            Ok(if value.is_null() { None } else { Some(value) })
        },
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

async fn wait_for_termination_signal() -> std::io::Result<()> {
    let mut sigint = unix_signal(SignalKind::interrupt())?;
    let mut sigterm = unix_signal(SignalKind::terminate())?;
    tokio::select! {
        _ = sigint.recv() => {},
        _ = sigterm.recv() => {},
    };
    Ok(())
}

/// Shared stop flag: armed once, set on the first SIGINT/SIGTERM.
/// In-flight operations finish; no new ones start.
pub(crate) struct StopFlag {
    inner: Arc<AtomicBool>,
}

impl StopFlag {
    pub(crate) fn arm() -> Self {
        let flag = Self {
            inner: Arc::new(AtomicBool::new(false)),
        };
        let inner = flag.inner.clone();
        tokio::spawn(async move {
            match wait_for_termination_signal().await {
                Ok(()) => {
                    inner.store(true, Ordering::SeqCst);
                    eprintln!(
                        "Interrupt received: finishing in-flight operations, no new ones will start"
                    );
                },
                Err(e) => eprintln!("failed to install signal handlers: {e}"),
            }
        });
        flag
    }

    pub(crate) fn is_set(&self) -> bool {
        self.inner.load(Ordering::SeqCst)
    }
}

pub(crate) trait PhaseItem {
    fn id(&self) -> &str;
}

impl PhaseItem for String {
    fn id(&self) -> &str {
        self.as_str()
    }
}

pub(crate) struct PhaseContext<'a> {
    pub(crate) mp: &'a Option<MultiProgress>,
    pub(crate) concurrency: usize,
    pub(crate) show_actions: bool,
    pub(crate) stop: &'a StopFlag,
}

pub(crate) struct PhaseOutcome<M> {
    pub(crate) done: Vec<(String, M)>,
    pub(crate) failed: Vec<(String, String)>,
    pub(crate) skipped: usize,
}

pub(crate) async fn run_phase<I, A, Fut, M>(
    items: Vec<I>,
    label: &str,
    verb: &str,
    ctx: &PhaseContext<'_>,
    action: A,
) -> PhaseOutcome<M>
where
    I: PhaseItem,
    A: Fn(I) -> Fut + Clone,
    Fut: Future<Output = Result<M>>,
{
    let mut outcome = PhaseOutcome {
        done: Vec::new(),
        failed: Vec::new(),
        skipped: 0,
    };
    if ctx.stop.is_set() {
        outcome.skipped = items.len();
        return outcome;
    }

    let bar = ctx.mp.as_ref().map(|m| make_bar(m, items.len(), label));
    let outcomes = map_concurrent(items, ctx.concurrency, |item| {
        let id = item.id().to_string();
        let action = action.clone();
        let bar = bar.clone();
        let stop = ctx.stop;
        async move {
            if stop.is_set() {
                return (id, None);
            }
            let result = action(item).await;
            if let Some(bar) = &bar {
                bar.inc(1);
            }
            (id, Some(result))
        }
    })
    .await;
    if let Some(bar) = &bar {
        bar.finish();
    }

    for (id, result) in outcomes {
        match result {
            Some(Ok(value)) => {
                if ctx.show_actions {
                    println!("{verb} {id}");
                }
                outcome.done.push((id, value));
            },
            Some(Err(e)) => {
                eprintln!("error {id}: {e:#}");
                outcome.failed.push((id, format!("{e:#}")));
            },
            None => {
                outcome.skipped += 1;
            },
        }
    }

    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_metadata_none_is_null() {
        assert_eq!(parse_metadata(&None).unwrap(), Value::Null);
    }

    #[test]
    fn parse_metadata_valid_json() {
        let value = parse_metadata(&Some(r#"{"team":"ml"}"#.into())).unwrap();
        assert_eq!(value["team"], "ml");
    }

    #[test]
    fn parse_metadata_invalid_json_is_error() {
        assert!(parse_metadata(&Some("not json".into())).is_err());
    }

    #[test]
    fn parse_indexes_none_is_none() {
        assert_eq!(parse_indexes(&None).unwrap(), None);
    }

    #[test]
    fn parse_indexes_null_is_none() {
        assert_eq!(parse_indexes(&Some("null".into())).unwrap(), None);
    }

    #[test]
    fn parse_indexes_valid_json() {
        let value = parse_indexes(&Some(r#"{"a":{"type":"keyword"}}"#.into())).unwrap();
        let value = value.expect("non-null indexes");
        assert_eq!(value["a"]["type"], "keyword");
    }

    #[test]
    fn parse_indexes_invalid_json_is_error() {
        assert!(parse_indexes(&Some("[broken".into())).is_err());
    }

    #[test]
    fn parse_reindex_mode_valid() {
        for mode in ["compare", "force", "full"] {
            assert_eq!(parse_reindex_mode(mode).unwrap(), mode);
        }
    }

    #[test]
    fn parse_reindex_mode_invalid_is_error() {
        assert!(parse_reindex_mode("sometimes").is_err());
    }

    #[test]
    fn sha256_hex_known_vector() {
        assert_eq!(
            sha256_hex(b"hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn file_stem_strips_extension() {
        assert_eq!(file_stem(Path::new("report.txt")), "report");
        assert_eq!(file_stem(Path::new("dir/doc.md")), "doc");
    }
}
