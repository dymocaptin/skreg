use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "skreg", about = "skreg skill package manager")]
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
    },
    /// Download and install a skill
    Install {
        /// Package reference (namespace/name or namespace/name@version)
        #[arg(value_name = "PACKAGE")]
        package_ref: String,
    },
    /// Launch the interactive terminal UI
    Tui,
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
        Commands::Search { query } => {
            skreg_cli::commands::search::run_search(&query).await?;
        }
        Commands::Install { package_ref } => {
            skreg_cli::commands::install::run_install(&package_ref).await?;
        }
        Commands::Tui => {
            skreg_cli::commands::tui::run_tui()?;
        }
    }
    Ok(())
}
