use anyhow::Result;
use clap::{Parser, Subcommand};

mod cli;
mod infrastructure;

#[derive(Parser)]
#[command(name = "roxy")]
#[command(about = "Local development proxy with custom .roxy domains and HTTPS")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initial setup - configures DNS and prepares Roxy for use
    Install,

    /// Remove all Roxy configuration from the system
    Uninstall {
        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Install => cli::install::execute(),
        Commands::Uninstall { force } => cli::uninstall::execute(force),
    }
}
