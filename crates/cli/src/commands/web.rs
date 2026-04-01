//! `rtop web` — start the embedded Web UI server.

use std::path::Path;

use anyhow::Result;
use clap::Args;

#[derive(Args)]
pub struct WebArgs {
    /// Override the HTTP port from config
    #[arg(long)]
    port: Option<u16>,
}

pub async fn run(args: WebArgs, cfg_path: &Path) -> Result<()> {
    let mut cfg = app_config::load(cfg_path).unwrap_or_default();
    if let Some(port) = args.port {
        cfg.web.port = port;
    }
    app_server::start(&cfg, cfg_path.to_path_buf()).await
}
