use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod cli;
mod domain;
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

    /// Register a new domain
    Register {
        /// Domain name (must end with .roxy)
        domain: String,

        /// Path to serve static files from
        #[arg(long, conflicts_with = "port")]
        path: Option<PathBuf>,

        /// Port to proxy requests to
        #[arg(long, conflicts_with = "path")]
        port: Option<u16>,
    },

    /// Unregister a domain
    Unregister {
        /// Domain name to unregister
        domain: String,

        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },

    /// List all registered domains
    List,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Install => cli::install::execute(),
        Commands::Uninstall { force } => cli::uninstall::execute(force),
        Commands::Register { domain, path, port } => cli::register::execute(domain, path, port),
        Commands::Unregister { domain, force } => cli::unregister::execute(domain, force),
        Commands::List => cli::list::execute(),
    }
}
