use anyhow::{bail, Result};
use clap::Subcommand;
use indicatif::MultiProgress;

use crate::api::{Api, DEFAULT_PAGE_LIMIT};
use crate::models::{Doc, Kb, KbRequest, ReindexRequest, ReindexResponse};
use crate::output::{print_json, print_kv, print_table};
use crate::util::{
    parse_indexes, parse_metadata, parse_reindex_mode, run_phase, PhaseContext, PhaseItem,
    PhaseOutcome, StopFlag,
};

#[derive(Subcommand)]
pub(crate) enum KbCommand {
    /// List knowledge bases
    List {
        /// Print raw JSON instead of a table
        #[arg(long)]
        json: bool,
    },
    /// Show a knowledge base by id
    Get {
        id: String,
        /// Print raw JSON instead of key/value lines
        #[arg(long)]
        json: bool,
    },
    /// Create a knowledge base
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        description: Option<String>,
        /// Mark the knowledge base as public
        #[arg(long)]
        public: bool,
        /// JSON metadata, e.g. '{"team":"ml"}'
        #[arg(long)]
        metadata: Option<String>,
        /// JSON metadata indexes, e.g. '{"reporter":{"type":"keyword"},"updated_at":{"type":"datetime"}}'
        #[arg(long)]
        indexes: Option<String>,
        /// Language for BM25 stemming and stopwords, e.g. "russian"
        #[arg(long)]
        language: Option<String>,
    },
    /// Update a knowledge base; only given fields change
    Update {
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// Set public flag; --public or --public=false
        #[arg(long, num_args = 0..=1, require_equals = true, default_missing_value = "true")]
        public: Option<bool>,
        /// JSON metadata, e.g. '{"team":"ml"}'
        #[arg(long)]
        metadata: Option<String>,
        /// JSON metadata indexes, e.g. '{"reporter":{"type":"keyword"}}'; use '{}' to remove all indexes
        #[arg(long)]
        indexes: Option<String>,
    },
    /// Delete a knowledge base
    Delete { id: String },
    /// Reindex every document in a knowledge base
    Reindex {
        id: String,
        /// compare | force | full
        #[arg(long, default_value = "compare")]
        mode: String,
        /// Disable progress bar; print actions to the console instead
        #[arg(long)]
        no_progress: bool,
        /// Maximum number of concurrent reindex operations
        #[arg(long, default_value_t = 5)]
        concurrency: usize,
    },
}

impl PhaseItem for Doc {
    fn id(&self) -> &str {
        &self.id
    }
}

pub(crate) async fn run(api: &Api, command: KbCommand) -> Result<()> {
    match command {
        KbCommand::List { json } => cmd_kb_list(api, json).await,
        KbCommand::Get { id, json } => cmd_kb_get(api, &id, json).await,
        KbCommand::Create {
            name,
            description,
            public,
            metadata,
            indexes,
            language,
        } => {
            cmd_kb_create(
                api,
                &name,
                &description,
                public,
                &metadata,
                &indexes,
                &language,
            )
            .await
        },
        KbCommand::Update {
            id,
            name,
            description,
            public,
            metadata,
            indexes,
        } => cmd_kb_update(api, &id, &name, &description, &public, &metadata, &indexes).await,
        KbCommand::Delete { id } => cmd_kb_delete(api, &id).await,
        KbCommand::Reindex {
            id,
            mode,
            no_progress,
            concurrency,
        } => cmd_kb_reindex(api, &id, &mode, no_progress, concurrency).await,
    }
}

async fn cmd_kb_list(api: &Api, json: bool) -> Result<()> {
    let kbs: Vec<Kb> = api.get_json("/internal/api/knowledge-bases", &[]).await?;
    if json {
        return print_json(&kbs);
    }
    if kbs.is_empty() {
        println!("No knowledge bases found");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = kbs
        .iter()
        .map(|kb| {
            vec![
                kb.id.clone(),
                kb.name.clone(),
                kb.description.clone(),
                kb.public.to_string(),
                kb.language.clone().unwrap_or_default(),
                kb.created_at.clone(),
                kb.updated_at.clone(),
            ]
        })
        .collect();
    print_table(
        &[
            "ID",
            "NAME",
            "DESCRIPTION",
            "PUBLIC",
            "LANGUAGE",
            "CREATED",
            "UPDATED",
        ],
        &rows,
    );
    Ok(())
}

async fn cmd_kb_get(api: &Api, id: &str, json: bool) -> Result<()> {
    let kb: Kb = api
        .get_json(&format!("/internal/api/knowledge-bases/{id}"), &[])
        .await?;
    if json {
        return print_json(&kb);
    }
    print_kv(&[
        ("id", kb.id),
        ("name", kb.name),
        ("description", kb.description),
        ("public", kb.public.to_string()),
        ("metadata", kb.metadata.to_string()),
        (
            "indexes",
            kb.indexes
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_default(),
        ),
        ("language", kb.language.unwrap_or_default()),
        ("createdAt", kb.created_at),
        ("updatedAt", kb.updated_at),
    ]);
    Ok(())
}

async fn cmd_kb_create(
    api: &Api,
    name: &str,
    description: &Option<String>,
    public: bool,
    metadata: &Option<String>,
    indexes: &Option<String>,
    language: &Option<String>,
) -> Result<()> {
    let request = KbRequest {
        name: name.to_string(),
        description: description.clone().unwrap_or_default(),
        public,
        metadata: parse_metadata(metadata)?,
        indexes: parse_indexes(indexes)?,
        language: language.clone(),
    };
    let kb: Kb = api
        .send_json(
            reqwest::Method::POST,
            "/internal/api/knowledge-bases",
            &request,
        )
        .await?;
    println!("Created knowledge base {}", kb.id);
    Ok(())
}

async fn cmd_kb_update(
    api: &Api,
    id: &str,
    name: &Option<String>,
    description: &Option<String>,
    public: &Option<bool>,
    metadata: &Option<String>,
    indexes: &Option<String>,
) -> Result<()> {
    let existing: Kb = api
        .get_json(&format!("/internal/api/knowledge-bases/{id}"), &[])
        .await?;
    let new_metadata = parse_metadata(metadata)?;
    let new_indexes = parse_indexes(indexes)?;
    let request = KbRequest {
        name: name.clone().unwrap_or(existing.name),
        description: description.clone().unwrap_or(existing.description),
        public: public.unwrap_or(existing.public),
        metadata: if new_metadata.is_null() {
            existing.metadata
        } else {
            new_metadata
        },
        indexes: new_indexes.or(existing.indexes),
        language: None,
    };
    let kb: Kb = api
        .send_json(
            reqwest::Method::PUT,
            &format!("/internal/api/knowledge-bases/{id}"),
            &request,
        )
        .await?;
    println!("Updated knowledge base {}", kb.id);
    Ok(())
}

async fn cmd_kb_delete(api: &Api, id: &str) -> Result<()> {
    api.delete(&format!("/internal/api/knowledge-bases/{id}"))
        .await?;
    println!("Deleted knowledge base {id}");
    Ok(())
}

async fn cmd_kb_reindex(
    api: &Api,
    id: &str,
    mode: &str,
    no_progress: bool,
    concurrency: usize,
) -> Result<()> {
    if concurrency == 0 {
        bail!("--concurrency must be >= 1");
    }
    let mode = parse_reindex_mode(mode)?;
    let docs: Vec<Doc> = api
        .get_json_all_pages(
            "/internal/api/documents",
            &[("kb_id", id.to_string())],
            DEFAULT_PAGE_LIMIT,
        )
        .await?;

    if docs.is_empty() {
        println!("No documents found");
        return Ok(());
    }

    let mp = if no_progress {
        None
    } else {
        Some(MultiProgress::new())
    };

    let stop = StopFlag::arm();
    let ctx = PhaseContext {
        mp: &mp,
        concurrency,
        show_actions: false,
        stop: &stop,
    };

    let outcome: PhaseOutcome<ReindexResponse> =
        run_phase(docs, "reindexing", "reindexed", &ctx, |doc| async move {
            let request = ReindexRequest {
                mode: mode.to_string(),
            };
            api.send_json(
                reqwest::Method::POST,
                &format!("/internal/api/documents/{}/reindex", doc.id),
                &request,
            )
            .await
        })
        .await;

    let reindexed = outcome
        .done
        .iter()
        .filter(|(_, response)| response.reindexed)
        .count();
    let up_to_date = outcome.done.len() - reindexed;
    let failed = outcome.failed.len();
    let skipped = outcome.skipped;

    println!(
        "Reindexed: {reindexed} reindexed, {up_to_date} up to date, {failed} failed, {skipped} skipped"
    );

    if skipped > 0 {
        bail!("reindex interrupted: {skipped} operations skipped");
    }

    if failed > 0 {
        bail!("reindex finished with {failed} failures");
    }

    Ok(())
}
