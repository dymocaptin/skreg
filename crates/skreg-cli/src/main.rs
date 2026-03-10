use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "skreg", about = "skreg skill package manager", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Pack the current directory into a .skill tarball
    Pack,
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let cli = Cli::parse();
    match cli.command {
        Commands::Pack => {
            skreg_cli::commands::pack::run_pack(std::env::current_dir()?.as_path())?;
        }
        Commands::Login { namespace } => {
            skreg_cli::commands::login::run_login(&namespace).await?;
        }
        Commands::Publish => {
            skreg_cli::commands::publish::run_publish().await?;
        }
        Commands::Search { query, verified } => {
            skreg_cli::commands::search::run_search(&query, verified).await?;
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
            skreg_cli::commands::install::run_install(&package_ref, level).await?;
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
    }
    Ok(())
}
