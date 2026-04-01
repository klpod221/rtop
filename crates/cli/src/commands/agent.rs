//! `rtop agent` — run the telemetry daemon.

use std::path::{Path};

use anyhow::Result;
use clap::Args;

use app_agent::RunOptions;

#[derive(Args)]
pub struct AgentArgs {
    /// Print collected payload to stderr instead of POSTing (no HTTP required)
    #[arg(long)]
    dry_run: bool,
    /// Exit after a single collection cycle
    #[arg(long)]
    once: bool,
}

pub async fn run(args: AgentArgs, cfg_path: &Path) -> Result<()> {
    app_agent::run(RunOptions {
        config_path: Some(cfg_path.to_path_buf()),
        dry_run: args.dry_run,
        once: args.once,
    })
    .await
}
