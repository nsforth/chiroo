mod api;
mod cli;
mod doc;
mod kb;
mod models;
mod output;
mod sync_kb;
mod util;

use clap::Parser;

use crate::api::Api;
use crate::cli::{Cli, Command};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let api = Api::new(cli.api_url);

    match cli.command {
        Command::Kb { command } => kb::run(&api, command).await?,
        Command::Doc { command } => doc::run(&api, command).await?,
        Command::SyncKb { command } => sync_kb::run(&api, command).await?,
    }

    Ok(())
}
