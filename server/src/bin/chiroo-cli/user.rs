use anyhow::{bail, Result};
use clap::{Subcommand, ValueEnum};

use crate::api::Api;
use crate::models::{UpdateUserRequest, User};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum AdminValue {
    Yes,
    No,
}

impl AdminValue {
    fn as_bool(self) -> bool {
        matches!(self, AdminValue::Yes)
    }
}

#[derive(Subcommand)]
pub(crate) enum UserCommand {
    /// Change a user's admin flag
    Change {
        /// User UUID
        #[arg(
            long,
            required_unless_present = "username",
            conflicts_with = "username"
        )]
        id: Option<String>,
        /// Username to look up the user by
        #[arg(long, required_unless_present = "id", conflicts_with = "id")]
        username: Option<String>,
        /// Grant or revoke admin access
        #[arg(long, value_enum)]
        admin: AdminValue,
    },
}

pub(crate) async fn run(api: &Api, command: UserCommand) -> Result<()> {
    match command {
        UserCommand::Change {
            id,
            username,
            admin,
        } => cmd_user_change(api, &id, &username, admin).await,
    }
}

async fn cmd_user_change(
    api: &Api,
    id: &Option<String>,
    username: &Option<String>,
    admin: AdminValue,
) -> Result<()> {
    let user_id = match id {
        Some(id) => id.clone(),
        None => {
            let username = match username {
                Some(username) => username,
                None => bail!("either --id or --username is required"),
            };
            let user: User = api
                .get_json("/internal/api/users", &[("username", username.to_string())])
                .await?;
            user.id
        },
    };

    let user: User = api
        .send_json(
            reqwest::Method::PATCH,
            &format!("/internal/api/users/{user_id}"),
            &UpdateUserRequest {
                admin: admin.as_bool(),
            },
        )
        .await?;

    println!(
        "Updated user {}: admin={}",
        user.id,
        if user.admin { "yes" } else { "no" }
    );
    Ok(())
}
