//! rtop — Linux system monitor
//!
//! Entry point: parses CLI args and dispatches to the appropriate command.

mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

const BANNER: &str = r"
  ██████╗ ████████╗ ██████╗ ██████╗
  ██╔══██╗╚══██╔══╝██╔═══██╗██╔══██╗
  ██████╔╝   ██║   ██║   ██║██████╔╝
  ██╔══██╗   ██║   ██║   ██║██╔═══╝
  ██║  ██║   ██║   ╚██████╔╝██║
  ╚═╝  ╚═╝   ╚═╝    ╚═════╝ ╚═╝
  Linux System Monitor             by klpod221
";

#[derive(Parser)]
#[command(
    name = "rtop",
    about = "High-performance Linux system monitor",
    long_about = BANNER,
    version
)]
struct Cli {
    /// Path to config.json (default: ~/.config/rtop/config.json)
    #[arg(long, global = true)]
    config: Option<std::path::PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Collect and print system metrics (JSON or flat)
    Get(commands::get::GetArgs),
    /// Run the telemetry agent daemon
    Agent(commands::agent::AgentArgs),
    /// Install / uninstall / manage the systemd service
    Service(commands::service::ServiceArgs),
    /// Start the embedded Web UI server
    Web(commands::web::WebArgs),
    /// Start the MCP JSON-RPC 2.0 server (stdio)
    Mcp,
    /// Interactive terminal UI (btop-style)
    Tui,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialise tracing — level controlled by RUST_LOG env var
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .compact()
        .init();

    let cfg_path = resolve_config(cli.config)?;

    match cli.command.unwrap_or(Commands::Tui) {
        Commands::Get(args)     => commands::get::run(args, &cfg_path).await,
        Commands::Agent(args)   => commands::agent::run(args, &cfg_path).await,
        Commands::Service(args) => commands::service::run(args),
        Commands::Web(args)     => commands::web::run(args, &cfg_path).await,
        Commands::Mcp           => commands::mcp::run(&cfg_path),
        Commands::Tui           => commands::tui::run(&cfg_path).await,
    }
}

fn resolve_config(override_path: Option<std::path::PathBuf>) -> Result<std::path::PathBuf> {
    if let Some(p) = override_path { return Ok(p); }
    app_config::default_config_path().map_err(|e| anyhow::anyhow!("{e}"))
}
