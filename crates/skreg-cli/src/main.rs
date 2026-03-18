use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "skreg", about = "skreg skill package manager", version)]
struct Cli {
    /// Named context from ~/.skreg/config.toml to use for this invocation.
    #[arg(long, global = true, value_name = "NAME")]
    context: Option<String>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Pack the current directory into a .skill tarball
    Pack {
        /// Path to PEM private key (overrides auto-generated key)
        #[arg(long, value_name = "FILE")]
        key: Option<PathBuf>,
        /// Path to PEM certificate (overrides auto-generated cert)
        #[arg(long, value_name = "FILE")]
        cert: Option<PathBuf>,
    },
    /// Register a namespace or re-authenticate
    Login { namespace: String },
    /// Publish a skill to the registry
    Publish,
    /// Search the registry for skills
    Search {
        /// Search query string
        query: String,
        /// Only show skills from verified (CA-signed) publishers
        #[arg(long)]
        verified: bool,
    },
    /// Download and install a skill
    Install {
        #[arg(value_name = "PACKAGE")]
        package_ref: String,
        /// Trust policy enforcement level (hint | confirm | strict)
        #[arg(long, value_name = "LEVEL")]
        enforcement: Option<String>,
    },
    /// List all tracked skill symlinks
    Links,
    /// Launch the interactive terminal UI
    Tui,
    /// Remove an installed skill
    Uninstall {
        /// Package reference (namespace/name)
        #[arg(value_name = "PACKAGE")]
        package_ref: String,
    },
    /// Obtain a CA-issued publisher certificate
    Certify {
        /// Path to existing PEM private key (uses ~/.skreg/keys/publisher.key if omitted)
        #[arg(long, value_name = "FILE")]
        key: Option<PathBuf>,
    },
    /// Rotate the publisher key (requires email confirmation)
    Rotate {
        /// Path to new PEM private key (generates a fresh RSA-2048 key if omitted)
        #[arg(long, value_name = "FILE")]
        new_key: Option<PathBuf>,
    },
    /// Manage registry contexts
    Context {
        #[command(subcommand)]
        command: skreg_cli::commands::context::ContextCommands,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let cli = Cli::parse();
    match cli.command {
        Commands::Pack { key, cert } => {
            skreg_cli::commands::pack::run_pack(
                std::env::current_dir()?.as_path(),
                key.as_deref(),
                cert.as_deref(),
            )?;
        }
        Commands::Login { namespace } => {
            skreg_cli::commands::login::run_login(&namespace).await?;
        }
        Commands::Publish => {
            skreg_cli::commands::publish::run_publish(cli.context.as_deref()).await?;
        }
        Commands::Search { query, verified } => {
            skreg_cli::commands::search::run_search(&query, verified, cli.context.as_deref())
                .await?;
        }
        Commands::Install {
            package_ref,
            enforcement,
        } => {
            let level = match enforcement.as_deref() {
                None => None,
                Some("hint") => Some(skreg_core::config::EnforcementLevel::Hint),
                Some("confirm") => Some(skreg_core::config::EnforcementLevel::Confirm),
                Some("strict") => Some(skreg_core::config::EnforcementLevel::Strict),
                Some(other) => anyhow::bail!(
                    "unknown enforcement level {other:?} — expected hint, confirm, or strict"
                ),
            };
            skreg_cli::commands::install::run_install(&package_ref, level, cli.context.as_deref())
                .await?;
        }
        Commands::Links => {
            skreg_cli::commands::links::run_links()?;
        }
        Commands::Tui => {
            skreg_cli::commands::tui::run_tui()?;
        }
        Commands::Uninstall { package_ref } => {
            skreg_cli::commands::uninstall::run_uninstall(&package_ref)?;
        }
        Commands::Certify { key } => {
            skreg_cli::commands::certify::run_certify(key.as_deref(), cli.context.as_deref())
                .await?;
        }
        Commands::Rotate { new_key } => {
            skreg_cli::commands::rotate::run_rotate(new_key.as_deref(), cli.context.as_deref())
                .await?;
        }
        Commands::Context { command } => {
            skreg_cli::commands::context::handle(command)?;
        }
    }
    Ok(())
}
