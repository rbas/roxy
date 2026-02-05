use anyhow::Result;
use clap::{Parser, Subcommand};

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

    /// Register a new domain with routes
    Register {
        /// Domain name (must end with .roxy)
        domain: String,

        /// Route in format PATH=TARGET (e.g., "/=3000" or "/api=3001")
        /// TARGET can be: port (3000), host:port (192.168.1.50:3000), or path (/var/www)
        #[arg(long, short = 'r', value_name = "PATH=TARGET", required = true)]
        route: Vec<String>,
    },

    /// Unregister a domain
    Unregister {
        /// Domain name to unregister
        domain: String,

        /// Skip confirmation prompt
        #[arg(long)]
        force: bool,
    },

    /// Manage routes for a domain
    Route {
        #[command(subcommand)]
        command: RouteCommands,
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

#[derive(Subcommand)]
enum RouteCommands {
    /// Add a route to an existing domain
    Add {
        /// Domain name
        domain: String,

        /// Path prefix (e.g., "/api")
        path: String,

        /// Target: port, host:port, or filesystem path
        target: String,
    },

    /// Remove a route from a domain
    Remove {
        /// Domain name
        domain: String,

        /// Path prefix to remove
        path: String,
    },

    /// List routes for a domain
    List {
        /// Domain name
        domain: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Note: verbose flag is available as cli.verbose for future use
    let _ = cli.verbose;

    match cli.command {
        Commands::Install => cli::install::execute(),
        Commands::Uninstall { force } => cli::uninstall::execute(force),
        Commands::Register { domain, route } => cli::register::execute(domain, route),
        Commands::Unregister { domain, force } => cli::unregister::execute(domain, force),
        Commands::Route { command } => match command {
            RouteCommands::Add {
                domain,
                path,
                target,
            } => cli::route::add(domain, path, target),
            RouteCommands::Remove { domain, path } => cli::route::remove(domain, path),
            RouteCommands::List { domain } => cli::route::list(domain),
        },
        Commands::List => cli::list::execute(),
        Commands::Start { foreground } => cli::start::execute(foreground),
        Commands::Stop => cli::stop::execute(),
        Commands::Restart => cli::restart::execute(),
        Commands::Status => cli::status::execute(),
        Commands::Logs { lines, clear } => cli::logs::execute(lines, clear),
        Commands::Reload => cli::reload::execute(),
    }
}
