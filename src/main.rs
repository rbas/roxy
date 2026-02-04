use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod cli;
mod daemon;
mod domain;
mod infrastructure;

#[derive(Parser)]
#[command(name = "roxy")]
#[command(about = "Local development proxy with custom .roxy domains and HTTPS")]
#[command(version)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

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

    /// Start the Roxy daemon
    Start {
        /// Run in foreground (don't daemonize)
        #[arg(long)]
        foreground: bool,
    },

    /// Stop the Roxy daemon
    Stop,

    /// Restart the Roxy daemon
    Restart,

    /// Show daemon and domain status
    Status,

    /// View daemon logs
    Logs {
        /// Number of lines to show
        #[arg(short = 'n', long, default_value = "50")]
        lines: usize,

        /// Clear all logs
        #[arg(long)]
        clear: bool,
    },

    /// Reload daemon configuration
    Reload,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Note: verbose flag is available as cli.verbose for future use
    let _ = cli.verbose;

    match cli.command {
        Commands::Install => cli::install::execute(),
        Commands::Uninstall { force } => cli::uninstall::execute(force),
        Commands::Register { domain, path, port } => cli::register::execute(domain, path, port),
        Commands::Unregister { domain, force } => cli::unregister::execute(domain, force),
        Commands::List => cli::list::execute(),
        Commands::Start { foreground } => cli::start::execute(foreground),
        Commands::Stop => cli::stop::execute(),
        Commands::Restart => cli::restart::execute(),
        Commands::Status => cli::status::execute(),
        Commands::Logs { lines, clear } => cli::logs::execute(lines, clear),
        Commands::Reload => cli::reload::execute(),
    }
}
