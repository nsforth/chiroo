use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};
use indicatif::MultiProgress;
use rayon::prelude::*;
use serde::Deserialize;
use serde_json::Value;

use crate::api::{build_doc_form, Api, DEFAULT_PAGE_LIMIT};
use crate::models::{Doc, Paged, VersionListItem};
use crate::util::{
    file_sha256_hex, file_stem, make_bar, run_phase, PhaseContext, PhaseItem, StopFlag,
};

#[derive(Args)]
pub(crate) struct SyncOptions {
    /// Disable progress bars; print actions to the console instead
    #[arg(long)]
    no_progress: bool,
    /// With --no-progress, print each action line
    #[arg(long)]
    verbose: bool,
    /// Do not delete server documents missing locally
    #[arg(long)]
    no_delete: bool,
    /// Show the plan without making any changes
    #[arg(long)]
    dry_run: bool,
    /// Maximum number of concurrent create/update operations
    #[arg(long, default_value_t = 5)]
    concurrency: usize,
}

#[derive(Subcommand)]
pub(crate) enum SyncKbCommand {
    /// Synchronize a knowledge base from a markdown export with sidecar JSON metadata
    Markdown {
        /// Root directory of the markdown export
        dir: PathBuf,
        /// Target knowledge base id
        #[arg(long)]
        kb: String,
        #[clap(flatten)]
        options: SyncOptions,
    },
}

#[derive(Debug, Deserialize)]
struct LocalDocMeta {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    metadata: Option<Value>,
}

#[derive(Debug)]
struct LocalDoc {
    id: String,
    path: PathBuf,
    title: String,
    url: String,
    version: String,
    metadata: Value,
}

struct ScanOutcome {
    docs: Vec<LocalDoc>,
    errors: Vec<(String, String)>,
}

struct Plan<'a> {
    to_create: Vec<&'a LocalDoc>,
    to_update: Vec<&'a LocalDoc>,
    to_delete: Vec<String>,
    up_to_date: usize,
}

pub(crate) async fn run(api: &Api, command: SyncKbCommand) -> Result<()> {
    match command {
        SyncKbCommand::Markdown { dir, kb, options } => {
            cmd_sync_kb_markdown(api, &dir, &kb, options).await
        },
    }
}

fn collect_md_files_recursive(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in
        fs::read_dir(dir).with_context(|| format!("failed to read dir {}", dir.display()))?
    {
        let entry = entry.with_context(|| format!("failed to read entry in {}", dir.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_md_files_recursive(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
    Ok(())
}

fn doc_id_from_path(rel: &Path) -> String {
    rel.with_extension("")
        .to_string_lossy()
        .replace('\\', "/")
        .replace('/', "---")
}

fn parse_sidecar(md_path: &Path) -> Result<LocalDocMeta> {
    let json_path = md_path.with_extension("json");
    if !json_path.is_file() {
        return Ok(LocalDocMeta {
            title: None,
            url: None,
            version: None,
            metadata: None,
        });
    }
    let text = fs::read_to_string(&json_path)
        .with_context(|| format!("failed to read {}", json_path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("invalid sidecar JSON {}", json_path.display()))
}

fn build_local_doc(file: &Path, id: &str) -> Result<LocalDoc> {
    let meta = parse_sidecar(file)?;
    let version = match meta.version {
        Some(v) => v,
        None => file_sha256_hex(file)?,
    };
    let title = meta.title.unwrap_or_else(|| file_stem(file));
    let url = meta.url.unwrap_or_default();
    let metadata = meta.metadata.unwrap_or_else(|| serde_json::json!({}));
    Ok(LocalDoc {
        id: id.to_string(),
        path: file.to_path_buf(),
        title,
        url,
        version,
        metadata,
    })
}

fn scan_dir(dir: &Path, mp: &Option<MultiProgress>) -> ScanOutcome {
    let mut files = Vec::new();
    if let Err(e) = collect_md_files_recursive(dir, &mut files) {
        return ScanOutcome {
            docs: Vec::new(),
            errors: vec![(dir.display().to_string(), format!("{e:#}"))],
        };
    }

    let bar = mp.as_ref().map(|m| make_bar(m, files.len(), "scanning"));

    let results: Vec<(String, Result<LocalDoc>)> = files
        .par_iter()
        .map(|file| {
            let rel = file.strip_prefix(dir).unwrap_or(file);
            let id = doc_id_from_path(rel);
            let result = build_local_doc(file, &id);
            if let Some(bar) = bar.as_ref() {
                bar.inc(1);
            }
            (id, result)
        })
        .collect();

    if let Some(bar) = &bar {
        bar.finish();
    }

    let mut docs = Vec::with_capacity(results.len());
    let mut errors = Vec::new();
    for (id, result) in results {
        match result {
            Ok(doc) => docs.push(doc),
            Err(e) => errors.push((id, format!("{e:#}"))),
        }
    }
    ScanOutcome { docs, errors }
}

async fn fetch_versions_map(api: &Api, kb: &str) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    let mut cursor: Option<String> = None;
    loop {
        let page: Paged<VersionListItem> = api
            .get_json_page(
                "/internal/api/documents/versions",
                &[("kb_id", kb.to_string())],
                cursor.as_deref(),
                DEFAULT_PAGE_LIMIT,
            )
            .await?;
        for item in page.items {
            map.insert(item.id, item.version);
        }
        match page.next_cursor {
            Some(next_cursor) => cursor = Some(next_cursor),
            None => break,
        }
    }
    Ok(map)
}

fn reconcile<'a>(
    local: &'a [LocalDoc],
    server: &HashMap<String, String>,
    no_delete: bool,
) -> Plan<'a> {
    let mut seen: HashSet<&str> = HashSet::with_capacity(local.len());
    let mut to_update = Vec::new();
    let mut up_to_date = 0usize;

    for doc in local {
        if let Some(version) = server.get(&doc.id) {
            seen.insert(doc.id.as_str());
            if *version == doc.version {
                up_to_date += 1;
            } else {
                to_update.push(doc);
            }
        }
    }

    let mut to_create: Vec<&LocalDoc> = local
        .iter()
        .filter(|d| !server.contains_key(&d.id))
        .collect();
    to_create.sort_by(|a, b| a.id.cmp(&b.id));
    to_update.sort_by(|a, b| a.id.cmp(&b.id));

    let mut to_delete: Vec<String> = if no_delete {
        Vec::new()
    } else {
        server
            .keys()
            .filter(|id| !seen.contains(id.as_str()))
            .cloned()
            .collect()
    };
    to_delete.sort();

    Plan {
        to_create,
        to_update,
        to_delete,
        up_to_date,
    }
}

async fn publish_document(api: &Api, doc: &LocalDoc, kb_id: Option<&str>) -> Result<Doc> {
    let data =
        fs::read(&doc.path).with_context(|| format!("failed to read {}", doc.path.display()))?;
    let form = build_doc_form(
        Some(&doc.id),
        kb_id,
        &doc.title,
        &doc.url,
        &doc.version,
        &doc.metadata,
        data,
    );
    match kb_id {
        Some(_) => {
            api.send_multipart(reqwest::Method::POST, "/internal/api/documents", form)
                .await
        },
        None => {
            api.send_multipart(
                reqwest::Method::PUT,
                &format!("/internal/api/documents/{}", doc.id),
                form,
            )
            .await
        },
    }
}

impl PhaseItem for &LocalDoc {
    fn id(&self) -> &str {
        self.id.as_str()
    }
}

async fn cmd_sync_kb_markdown(api: &Api, dir: &Path, kb: &str, opts: SyncOptions) -> Result<()> {
    if opts.concurrency == 0 {
        bail!("--concurrency must be >= 1");
    }
    let mp = if opts.no_progress {
        None
    } else {
        Some(MultiProgress::new())
    };

    let scan = scan_dir(dir, &mp);
    let mut failed = scan.errors.len();
    println!(
        "Found {} markdown files in {}",
        scan.docs.len(),
        dir.display()
    );
    for (id, message) in &scan.errors {
        eprintln!("error {id}: {message}");
    }

    let server = fetch_versions_map(api, kb).await?;
    let plan = reconcile(&scan.docs, &server, opts.no_delete);

    println!(
        "Plan: {} to create, {} to update, {} to delete, {} up to date",
        plan.to_create.len(),
        plan.to_update.len(),
        plan.to_delete.len(),
        plan.up_to_date
    );

    if opts.dry_run {
        for doc in &plan.to_create {
            println!("create {} (version {})", doc.id, doc.version);
        }
        for doc in &plan.to_update {
            println!("update {} (version {})", doc.id, doc.version);
        }
        for id in &plan.to_delete {
            println!("delete {id}");
        }
        return Ok(());
    }

    let stop = StopFlag::arm();

    let ctx = PhaseContext {
        mp: &mp,
        concurrency: opts.concurrency,
        show_actions: opts.no_progress && opts.verbose,
        stop: &stop,
    };

    let create_outcome = run_phase(
        plan.to_create,
        "creating",
        "created",
        &ctx,
        |doc| async move {
            publish_document(api, doc, Some(kb)).await?;
            Ok(())
        },
    )
    .await;
    let update_outcome = run_phase(
        plan.to_update,
        "updating",
        "updated",
        &ctx,
        |doc| async move {
            publish_document(api, doc, None).await?;
            Ok(())
        },
    )
    .await;
    let delete_outcome = run_phase(
        plan.to_delete,
        "deleting",
        "deleted",
        &ctx,
        |id| async move { api.delete(&format!("/internal/api/documents/{id}")).await },
    )
    .await;

    failed +=
        create_outcome.failed.len() + update_outcome.failed.len() + delete_outcome.failed.len();
    let skipped = create_outcome.skipped + update_outcome.skipped + delete_outcome.skipped;

    println!(
        "Synced: {} created, {} updated, {} deleted, {} up to date, {} failed, {} skipped",
        create_outcome.done.len(),
        update_outcome.done.len(),
        delete_outcome.done.len(),
        plan.up_to_date,
        failed,
        skipped
    );

    if skipped > 0 {
        bail!("sync interrupted: {skipped} operations skipped");
    }

    if failed > 0 {
        bail!("sync finished with {failed} failures");
    }

    Ok(())
}
