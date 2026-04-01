//! `rtop mcp` — start the MCP JSON-RPC 2.0 stdio server.

use std::path::Path;

use anyhow::Result;

pub fn run(cfg_path: &Path) -> Result<()> {
    let cfg = app_config::load(cfg_path).unwrap_or_default();
    app_mcp::run(&cfg)
}
