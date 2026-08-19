use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::Subcommand;
use uuid::Uuid;

use crate::api::{build_doc_form, Api};
use crate::models::{Doc, ReindexRequest, ReindexResponse};
use crate::output::{print_json, print_kv, print_table};
use crate::util::{file_stem, parse_metadata, parse_reindex_mode, sha256_hex};

#[derive(Subcommand)]
pub(crate) enum DocCommand {
    /// List documents in a knowledge base
    List {
        /// Knowledge base id
        #[arg(long)]
        kb: String,
        /// Filter by substring in title or url (server-side)
        #[arg(long)]
        filter: Option<String>,
        /// Include the metadata column
        #[arg(long)]
        metadata: bool,
        /// Print raw JSON instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Show document metadata
    Get {
        id: String,
        /// Print raw JSON instead of key/value lines
        #[arg(long)]
        json: bool,
    },
    /// Print document content to stdout or a file
    Cat {
        id: String,
        /// Write content to this file instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Upload a file as a document; creates a new one unless --id is given
    Upload {
        file: PathBuf,
        /// Knowledge base id (required when creating a new document)
        #[arg(long)]
        kb: Option<String>,
        /// Existing document id to update
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        url: Option<String>,
        /// JSON metadata, e.g. '{"author":"alice"}'
        #[arg(long)]
        metadata: Option<String>,
    },
    /// Delete a document
    Delete { id: String },
    /// Reindex a document's chunks in Qdrant from its DB content
    Reindex {
        id: String,
        /// compare | force | full
        #[arg(long, default_value = "compare")]
        mode: String,
    },
}

pub(crate) async fn run(api: &Api, command: DocCommand) -> Result<()> {
    match command {
        DocCommand::List {
            kb,
            filter,
            metadata,
            json,
        } => cmd_doc_list(api, &kb, &filter, metadata, json).await,
        DocCommand::Get { id, json } => cmd_doc_get(api, &id, json).await,
        DocCommand::Cat { id, output } => cmd_doc_cat(api, &id, &output).await,
        DocCommand::Upload {
            file,
            kb,
            id,
            title,
            url,
            metadata,
        } => cmd_doc_upload(api, &file, &kb, &id, &title, &url, &metadata).await,
        DocCommand::Delete { id } => cmd_doc_delete(api, &id).await,
        DocCommand::Reindex { id, mode } => cmd_doc_reindex(api, &id, &mode).await,
    }
}

async fn cmd_doc_list(
    api: &Api,
    kb: &str,
    filter: &Option<String>,
    with_metadata: bool,
    json: bool,
) -> Result<()> {
    let mut query: Vec<(&str, String)> = vec![("kb_id", kb.to_string())];
    if let Some(filter) = filter {
        query.push(("filter", filter.clone()));
    }
    let docs: Vec<Doc> = api.get_json("/internal/api/documents", &query).await?;
    if json {
        return print_json(&docs);
    }
    if docs.is_empty() {
        println!("No documents found");
        return Ok(());
    }

    let mut header: Vec<&str> = vec!["ID", "VERSION", "TITLE", "URL", "CREATED", "UPDATED"];
    if with_metadata {
        header.push("METADATA");
    }
    let rows: Vec<Vec<String>> = docs
        .iter()
        .map(|doc| {
            let mut row = vec![
                doc.id.clone(),
                doc.version.clone(),
                doc.title.clone(),
                doc.url.clone(),
                doc.created_at.clone(),
                doc.updated_at.clone(),
            ];
            if with_metadata {
                row.push(doc.metadata.to_string());
            }
            row
        })
        .collect();
    print_table(&header, &rows);
    Ok(())
}

async fn cmd_doc_get(api: &Api, id: &str, json: bool) -> Result<()> {
    let doc: Doc = api
        .get_json(&format!("/internal/api/documents/{id}"), &[])
        .await?;
    if json {
        return print_json(&doc);
    }
    print_kv(&[
        ("id", doc.id),
        ("kbId", doc.kb_id),
        ("version", doc.version),
        ("title", doc.title),
        ("url", doc.url),
        ("mimeType", doc.mime_type),
        ("metadata", doc.metadata.to_string()),
        ("createdAt", doc.created_at),
        ("updatedAt", doc.updated_at),
    ]);
    Ok(())
}

async fn cmd_doc_cat(api: &Api, id: &str, output: &Option<PathBuf>) -> Result<()> {
    let data = api
        .get_bytes(&format!("/internal/api/documents/{id}/data"))
        .await?;
    if let Some(path) = output {
        fs::write(path, &data).with_context(|| format!("failed to write {}", path.display()))?;
        println!("Wrote {} bytes to {}", data.len(), path.display());
    } else {
        io::stdout()
            .write_all(&data)
            .context("failed to write to stdout")?;
    }
    Ok(())
}

async fn cmd_doc_upload(
    api: &Api,
    file: &Path,
    kb: &Option<String>,
    id: &Option<String>,
    title: &Option<String>,
    url: &Option<String>,
    metadata: &Option<String>,
) -> Result<()> {
    let data = fs::read(file).with_context(|| format!("failed to read {}", file.display()))?;
    let version = sha256_hex(&data);
    let mut parsed_metadata = parse_metadata(metadata)?;

    if let Some(doc_id) = id {
        let existing: Doc = api
            .get_json(&format!("/internal/api/documents/{doc_id}"), &[])
            .await?;
        if parsed_metadata.is_null() {
            parsed_metadata = existing.metadata;
        }
        let resolved_title = title.clone().unwrap_or(existing.title);
        let resolved_url = url.clone().unwrap_or(existing.url);
        let form = build_doc_form(
            Some(doc_id),
            None,
            &resolved_title,
            &resolved_url,
            &version,
            &parsed_metadata,
            data,
        );
        let doc: Doc = api
            .send_multipart(
                reqwest::Method::PUT,
                &format!("/internal/api/documents/{doc_id}"),
                form,
            )
            .await?;
        println!("Updated document {} (version {})", doc.id, doc.version);
        return Ok(());
    }

    let kb_id = kb
        .clone()
        .ok_or_else(|| anyhow!("--kb is required when creating a new document"))?;
    if parsed_metadata.is_null() {
        parsed_metadata = serde_json::json!({});
    }
    let resolved_title = title.clone().unwrap_or_else(|| file_stem(file));
    let resolved_url = url.clone().unwrap_or_default();
    let doc_id = Uuid::new_v4().to_string();
    let form = build_doc_form(
        Some(&doc_id),
        Some(&kb_id),
        &resolved_title,
        &resolved_url,
        &version,
        &parsed_metadata,
        data,
    );
    let doc: Doc = api
        .send_multipart(reqwest::Method::POST, "/internal/api/documents", form)
        .await?;
    println!("Created document {} (version {})", doc.id, doc.version);
    Ok(())
}

async fn cmd_doc_delete(api: &Api, id: &str) -> Result<()> {
    api.delete(&format!("/internal/api/documents/{id}")).await?;
    println!("Deleted document {id}");
    Ok(())
}

async fn cmd_doc_reindex(api: &Api, id: &str, mode: &str) -> Result<()> {
    let mode = parse_reindex_mode(mode)?;
    let response: ReindexResponse = api
        .send_json(
            reqwest::Method::POST,
            &format!("/internal/api/documents/{id}/reindex"),
            &ReindexRequest {
                mode: mode.to_string(),
            },
        )
        .await?;
    if response.reindexed {
        println!("Reindexed document {id} ({} chunks)", response.chunks);
    } else {
        println!(
            "Document {id} already up to date ({} chunks)",
            response.chunks
        );
    }
    Ok(())
}
