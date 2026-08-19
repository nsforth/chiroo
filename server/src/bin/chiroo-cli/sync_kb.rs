use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use indicatif::MultiProgress;
use serde::Deserialize;
use serde_json::Value;

use crate::api::{build_doc_form, Api};
use crate::models::Doc;
use crate::util::{file_stem, make_bar, map_concurrent, sha256_hex};

#[derive(Subcommand)]
pub(crate) enum SyncKbCommand {
    /// Synchronize a knowledge base from a markdown export with sidecar JSON metadata
    Markdown {
        /// Root directory of the markdown export
        dir: PathBuf,
        /// Target knowledge base id
        #[arg(long)]
        kb: String,
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
        SyncKbCommand::Markdown {
            dir,
            kb,
            no_progress,
            verbose,
            no_delete,
            dry_run,
            concurrency,
        } => {
            cmd_sync_kb_markdown(
                api,
                &dir,
                &kb,
                no_progress,
                verbose,
                no_delete,
                dry_run,
                concurrency,
            )
            .await
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
        } else if path.extension().map_or(false, |ext| ext == "md") {
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
    let data = fs::read(file).with_context(|| format!("failed to read {}", file.display()))?;
    let version = meta.version.unwrap_or_else(|| sha256_hex(&data));
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

fn scan_dir(dir: &Path) -> ScanOutcome {
    let mut files = Vec::new();
    if let Err(e) = collect_md_files_recursive(dir, &mut files) {
        return ScanOutcome {
            docs: Vec::new(),
            errors: vec![(dir.display().to_string(), format!("{e:#}"))],
        };
    }
    files.sort();

    let mut docs = Vec::with_capacity(files.len());
    let mut errors = Vec::new();
    for file in files {
        let rel = file.strip_prefix(dir).unwrap_or(&file);
        let id = doc_id_from_path(rel);
        match build_local_doc(&file, &id) {
            Ok(doc) => docs.push(doc),
            Err(e) => errors.push((id, format!("{e:#}"))),
        }
    }
    ScanOutcome { docs, errors }
}

fn reconcile<'a>(local: &'a [LocalDoc], server: &[Doc], no_delete: bool) -> Plan<'a> {
    let mut by_id: HashMap<&str, &LocalDoc> = local.iter().map(|d| (d.id.as_str(), d)).collect();
    let mut to_update = Vec::new();
    let mut to_delete = Vec::new();
    let mut up_to_date = 0usize;

    for doc in server {
        match by_id.remove(doc.id.as_str()) {
            Some(local_doc) => {
                if local_doc.version == doc.version {
                    up_to_date += 1;
                } else {
                    to_update.push(local_doc);
                }
            },
            None if !no_delete => to_delete.push(doc.id.clone()),
            None => {},
        }
    }

    let mut to_create: Vec<&LocalDoc> = by_id.into_values().collect();
    to_create.sort_by(|a, b| a.id.cmp(&b.id));
    to_update.sort_by(|a, b| a.id.cmp(&b.id));
    to_delete.sort();

    Plan {
        to_create,
        to_update,
        to_delete,
        up_to_date,
    }
}

async fn create_document(api: &Api, doc: &LocalDoc, kb_id: &str) -> Result<Doc> {
    let data =
        fs::read(&doc.path).with_context(|| format!("failed to read {}", doc.path.display()))?;
    let form = build_doc_form(
        Some(&doc.id),
        Some(kb_id),
        &doc.title,
        &doc.url,
        &doc.version,
        &doc.metadata,
        data,
    );
    api.send_multipart(reqwest::Method::POST, "/internal/api/documents", form)
        .await
}

async fn update_document(api: &Api, doc: &LocalDoc) -> Result<Doc> {
    let data =
        fs::read(&doc.path).with_context(|| format!("failed to read {}", doc.path.display()))?;
    let form = build_doc_form(
        Some(&doc.id),
        None,
        &doc.title,
        &doc.url,
        &doc.version,
        &doc.metadata,
        data,
    );
    api.send_multipart(
        reqwest::Method::PUT,
        &format!("/internal/api/documents/{}", doc.id),
        form,
    )
    .await
}

async fn cmd_sync_kb_markdown(
    api: &Api,
    dir: &Path,
    kb: &str,
    no_progress: bool,
    verbose: bool,
    no_delete: bool,
    dry_run: bool,
    concurrency: usize,
) -> Result<()> {
    if concurrency == 0 {
        bail!("--concurrency must be >= 1");
    }
    let scan = scan_dir(dir);
    let mut failed = scan.errors.len();
    println!(
        "Found {} markdown files in {}",
        scan.docs.len(),
        dir.display()
    );
    for (id, message) in &scan.errors {
        eprintln!("error {id}: {message}");
    }

    let server: Vec<Doc> = api
        .get_json("/internal/api/documents", &[("kb_id", kb.to_string())])
        .await?;
    let plan = reconcile(&scan.docs, &server, no_delete);

    println!(
        "Plan: {} to create, {} to update, {} to delete, {} up to date",
        plan.to_create.len(),
        plan.to_update.len(),
        plan.to_delete.len(),
        plan.up_to_date
    );

    if dry_run {
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

    let show_actions = no_progress && verbose;
    let mp = if no_progress {
        None
    } else {
        Some(MultiProgress::new())
    };

    let mut created = 0usize;
    let mut updated = 0usize;
    let mut deleted = 0usize;

    if !plan.to_create.is_empty() {
        let bar = mp
            .as_ref()
            .map(|m| make_bar(m, plan.to_create.len(), "creating"));
        let outcomes = map_concurrent(plan.to_create, concurrency, |doc| {
            let bar = bar.clone();
            async move {
                let result = create_document(api, doc, kb).await;
                if let Some(bar) = &bar {
                    bar.inc(1);
                }
                (doc.id.clone(), result)
            }
        })
        .await;
        if let Some(bar) = &bar {
            bar.finish();
        }
        for (id, result) in outcomes {
            match result {
                Ok(_) => {
                    created += 1;
                    if show_actions {
                        println!("created {id}");
                    }
                },
                Err(e) => {
                    failed += 1;
                    eprintln!("error {id}: {e:#}");
                },
            }
        }
    }

    if !plan.to_update.is_empty() {
        let bar = mp
            .as_ref()
            .map(|m| make_bar(m, plan.to_update.len(), "updating"));
        let outcomes = map_concurrent(plan.to_update, concurrency, |doc| {
            let bar = bar.clone();
            async move {
                let result = update_document(api, doc).await;
                if let Some(bar) = &bar {
                    bar.inc(1);
                }
                (doc.id.clone(), result)
            }
        })
        .await;
        if let Some(bar) = &bar {
            bar.finish();
        }
        for (id, result) in outcomes {
            match result {
                Ok(_) => {
                    updated += 1;
                    if show_actions {
                        println!("updated {id}");
                    }
                },
                Err(e) => {
                    failed += 1;
                    eprintln!("error {id}: {e:#}");
                },
            }
        }
    }

    if !plan.to_delete.is_empty() {
        let bar = mp
            .as_ref()
            .map(|m| make_bar(m, plan.to_delete.len(), "deleting"));
        for id in &plan.to_delete {
            if let Some(bar) = &bar {
                bar.set_message(id.clone());
            }
            match api.delete(&format!("/internal/api/documents/{id}")).await {
                Ok(()) => {
                    deleted += 1;
                    if show_actions {
                        println!("deleted {id}");
                    }
                },
                Err(e) => {
                    failed += 1;
                    eprintln!("error {id}: {e:#}");
                },
            }
            if let Some(bar) = &bar {
                bar.inc(1);
            }
        }
        if let Some(bar) = &bar {
            bar.finish();
        }
    }

    println!(
        "Synced: {created} created, {updated} updated, {deleted} deleted, {up_to_date} up to date, {failed} failed",
        created = created,
        updated = updated,
        deleted = deleted,
        up_to_date = plan.up_to_date,
        failed = failed,
    );

    if failed > 0 {
        bail!("sync finished with {failed} failures");
    }

    Ok(())
}
