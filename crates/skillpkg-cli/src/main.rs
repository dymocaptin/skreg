use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "skillpkg", about = "skreg skill package manager")]
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Pack => {
            skillpkg_cli::commands::pack::run_pack(std::env::current_dir()?.as_path())?;
        }
        Commands::Login { namespace } => {
            skillpkg_cli::commands::login::run_login(&namespace).await?;
        }
        Commands::Publish => {
            skillpkg_cli::commands::publish::run_publish().await?;
        }
    }
    Ok(())
}
