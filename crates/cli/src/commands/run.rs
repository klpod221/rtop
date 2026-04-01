//! `rtop run` — start all enabled services based on config.
//!
//! This is the command used by the systemd unit file.
//! It reads `enabled_agent` and `enabled_web` from config and
//! starts whichever services are enabled, running them concurrently.

use std::path::Path;

use anyhow::{bail, Result};
use tracing::info;

pub async fn run(cfg_path: &Path) -> Result<()> {
    let cfg = app_config::load(cfg_path).unwrap_or_default();

    let run_agent = cfg.enabled_agent;
    let run_web = cfg.enabled_web;

    if !run_agent && !run_web {
        bail!(
            "Neither agent nor web is enabled in config ({}).\n\
             Set 'enabled_agent' or 'enabled_web' to true.",
            cfg_path.display()
        );
    }

    info!("rtop run: agent={}, web={}", run_agent, run_web);

    match (run_agent, run_web) {
        (true, true) => {
            // Run both concurrently; if either errors, propagate
            let agent_path = cfg_path.to_path_buf();
            let web_cfg = cfg.clone();
            let web_path = cfg_path.to_path_buf();
            let (a, w) = tokio::join!(
                async move {
                    app_agent::run(app_agent::RunOptions {
                        config_path: Some(agent_path),
                        dry_run: false,
                        once: false,
                    })
                    .await
                },
                async move { app_server::start(&web_cfg, web_path).await },
            );
            a?;
            w?;
        }
        (true, false) => {
            app_agent::run(app_agent::RunOptions {
                config_path: Some(cfg_path.to_path_buf()),
                dry_run: false,
                once: false,
            })
            .await?;
        }
        (false, true) => {
            app_server::start(&cfg, cfg_path.to_path_buf()).await?;
        }
        (false, false) => unreachable!(),
    }

    Ok(())
}
