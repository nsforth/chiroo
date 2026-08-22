use clap::{Parser, Subcommand};

use crate::doc::DocCommand;
use crate::kb::KbCommand;
use crate::sync_kb::SyncKbCommand;
use crate::user::UserCommand;

pub(crate) const DEFAULT_API_URL: &str = "http://127.0.0.1:3001";

#[derive(Parser)]
#[command(
    name = "chiroo-cli",
    version,
    about = "Manage a chiroo server via its internal API"
)]
pub(crate) struct Cli {
    /// Internal API base URL
    #[arg(long, global = true, env = "CHIROO_API_URL", default_value = DEFAULT_API_URL)]
    pub(crate) api_url: String,

    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Knowledge base operations
    Kb {
        #[command(subcommand)]
        command: KbCommand,
    },
    /// Document operations
    Doc {
        #[command(subcommand)]
        command: DocCommand,
    },
    /// Synchronize a knowledge base with a local directory
    SyncKb {
        #[command(subcommand)]
        command: SyncKbCommand,
    },
    /// User operations
    User {
        #[command(subcommand)]
        command: UserCommand,
    },
}
