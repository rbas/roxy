use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

mod application;
mod cli;
mod daemon;
mod domain;
mod infrastructure;

use infrastructure::config::{Config, ConfigStore};
use infrastructure::paths::RoxyPaths;

#[derive(Parser)]
#[command(name = "roxy")]
#[command(about = "Local development proxy with custom .roxy domains and HTTPS")]
#[command(version)]
#[command(
    after_help = concat!("Heads up: Roxy is still finding her feet (v", env!("CARGO_PKG_VERSION"), ").\nThings may shift around. If something bites, let me know!\nhttps://github.com/rbas/roxy/issues")
)]
struct Cli {
    /// Path to the config file
    #[arg(short, long, global = true, default_value = "/etc/roxy/config.toml")]
    config: PathBuf,

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

        /// Register wildcard subdomains for this domain (matches myapp.roxy and *.myapp.roxy)
        #[arg(long)]
        wildcard: bool,

        /// Route in format PATH=TARGET (e.g., "/=3000" or "/api=3001")
        /// TARGET can be: port (3000), host:port (192.168.1.50:3000), or path (/var/www)
        #[arg(long, short = 'r', value_name = "PATH=TARGET", required = true)]
        route: Vec<String>,
    },

    /// Unregister a domain
    Unregister {
        /// Domain name to unregister
        domain: String,

        /// Unregister wildcard subdomains for this domain (removes *.myapp.roxy registration)
        #[arg(long)]
        wildcard: bool,

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

        /// Follow log output (like tail -f)
        #[arg(short = 'f', long)]
        follow: bool,
    },

    /// Reload daemon configuration
    Reload,

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
}

#[derive(Subcommand)]
enum RouteCommands {
    /// Add a route to an existing domain
    Add {
        /// Manage wildcard registration for this domain (matches myapp.roxy and *.myapp.roxy)
        #[arg(long)]
        wildcard: bool,

        /// Domain name
        domain: String,

        /// Path prefix (e.g., "/api")
        path: String,

        /// Target: port, host:port, or filesystem path
        target: String,
    },

    /// Remove a route from a domain
    Remove {
        /// Manage wildcard registration for this domain (matches myapp.roxy and *.myapp.roxy)
        #[arg(long)]
        wildcard: bool,

        /// Domain name
        domain: String,

        /// Path prefix to remove
        path: String,
    },

    /// List routes for a domain
    List {
        /// Manage wildcard registration for this domain (matches myapp.roxy and *.myapp.roxy)
        #[arg(long)]
        wildcard: bool,

        /// Domain name
        domain: String,
    },
}

/// Load config from file, or return defaults if the file doesn't exist.
/// For `install`, the config file may not exist yet, so defaults are fine.
fn load_config_and_paths(config_path: &Path) -> Result<(Config, RoxyPaths)> {
    let config_store = ConfigStore::new(config_path.to_path_buf());
    let config = config_store.load()?;
    let paths = config.paths.clone();
    Ok((config, paths))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_path = &cli.config;

    let (config, paths) = load_config_and_paths(config_path)?;

    match cli.command {
        Commands::Install => cli::install::execute(config_path, &paths, &config),
        Commands::Uninstall { force } => cli::uninstall::execute(force, config_path, &paths),
        Commands::Register {
            domain,
            wildcard,
            route,
        } => cli::register::execute(domain, wildcard, route, config_path, &paths),
        Commands::Unregister {
            domain,
            wildcard,
            force,
        } => cli::unregister::execute(domain, wildcard, force, config_path, &paths),
        Commands::Route { command } => match command {
            RouteCommands::Add {
                wildcard,
                domain,
                path,
                target,
            } => cli::route::add(domain, wildcard, path, target, config_path),
            RouteCommands::Remove {
                wildcard,
                domain,
                path,
            } => cli::route::remove(domain, wildcard, path, config_path),
            RouteCommands::List { wildcard, domain } => {
                cli::route::list(domain, wildcard, config_path)
            }
        },
        Commands::List => cli::list::execute(config_path, &paths),
        Commands::Start { foreground } => {
            cli::start::execute(foreground, cli.verbose, config_path, &paths, &config)
        }
        Commands::Stop => cli::stop::execute(&paths),
        Commands::Restart => cli::restart::execute(cli.verbose, config_path, &paths),
        Commands::Status => cli::status::execute(config_path, &paths),
        Commands::Logs {
            lines,
            clear,
            follow,
        } => cli::logs::execute(lines, clear, follow, &paths),
        Commands::Reload => cli::reload::execute(cli.verbose, config_path, &paths),
        Commands::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "roxy", &mut std::io::stdout());
            Ok(())
        }
    }
}
