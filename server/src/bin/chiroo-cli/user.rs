use anyhow::{bail, Result};
use clap::{Subcommand, ValueEnum};

use crate::api::Api;
use crate::models::{CreateUserRequest, UpdateUserRequest, User};

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
    /// Create or update a user by SSO identity (always as a non-admin;
    /// grant admin separately with `user change`)
    Create {
        /// SSO identity (users.sso_id, e.g. the OIDC `sub` claim)
        #[arg(long)]
        sso_id: String,
        /// SSO provider (defaults to oidc)
        #[arg(long, default_value = "oidc")]
        sso_provider: String,
        /// Username
        #[arg(long)]
        username: String,
        /// Display name
        #[arg(long)]
        name: Option<String>,
        /// Email
        #[arg(long)]
        email: Option<String>,
    },
    /// Delete a user
    Delete {
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
        /// Also delete the user's chats
        #[arg(long)]
        purge_chats: bool,
    },
}

pub(crate) async fn run(api: &Api, command: UserCommand) -> Result<()> {
    match command {
        UserCommand::Change {
            id,
            username,
            admin,
        } => cmd_user_change(api, &id, &username, admin).await,
        UserCommand::Create {
            sso_id,
            sso_provider,
            username,
            name,
            email,
        } => cmd_user_create(api, &sso_id, &sso_provider, &username, name, email).await,
        UserCommand::Delete {
            id,
            username,
            purge_chats,
        } => cmd_user_delete(api, &id, &username, purge_chats).await,
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

async fn cmd_user_create(
    api: &Api,
    sso_id: &str,
    sso_provider: &str,
    username: &str,
    name: Option<String>,
    email: Option<String>,
) -> Result<()> {
    let user: User = api
        .send_json(
            reqwest::Method::POST,
            "/internal/api/users",
            &CreateUserRequest {
                sso_id: sso_id.to_string(),
                sso_provider: sso_provider.to_string(),
                username: username.to_string(),
                name,
                email,
            },
        )
        .await?;

    println!("Created/updated user {} ({})", user.id, user.username);
    Ok(())
}

async fn cmd_user_delete(
    api: &Api,
    id: &Option<String>,
    username: &Option<String>,
    purge_chats: bool,
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

    let path = if purge_chats {
        format!("/internal/api/users/{user_id}?purgeChats=true")
    } else {
        format!("/internal/api/users/{user_id}")
    };
    api.delete(&path).await?;

    println!("Deleted user {user_id}");
    Ok(())
}
