use anyhow::Result;
use clap::Subcommand;

use crate::api::Api;
use crate::models::{Kb, KbRequest};
use crate::output::{print_json, print_kv, print_table};
use crate::util::parse_metadata;

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
    },
    /// Delete a knowledge base
    Delete { id: String },
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
        } => cmd_kb_create(api, &name, &description, public, &metadata).await,
        KbCommand::Update {
            id,
            name,
            description,
            public,
            metadata,
        } => cmd_kb_update(api, &id, &name, &description, &public, &metadata).await,
        KbCommand::Delete { id } => cmd_kb_delete(api, &id).await,
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
                kb.created_at.clone(),
                kb.updated_at.clone(),
            ]
        })
        .collect();
    print_table(
        &["ID", "NAME", "DESCRIPTION", "PUBLIC", "CREATED", "UPDATED"],
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
) -> Result<()> {
    let request = KbRequest {
        name: name.to_string(),
        description: description.clone().unwrap_or_default(),
        public,
        metadata: parse_metadata(metadata)?,
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
) -> Result<()> {
    let existing: Kb = api
        .get_json(&format!("/internal/api/knowledge-bases/{id}"), &[])
        .await?;
    let new_metadata = parse_metadata(metadata)?;
    let request = KbRequest {
        name: name.clone().unwrap_or(existing.name),
        description: description.clone().unwrap_or(existing.description),
        public: public.unwrap_or(existing.public),
        metadata: if new_metadata.is_null() {
            existing.metadata
        } else {
            new_metadata
        },
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
