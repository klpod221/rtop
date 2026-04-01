//! `rtop service` — systemd unit file management.

use anyhow::{bail, Context, Result};
use clap::{Args, Subcommand};

#[derive(Args)]
pub struct ServiceArgs {
    #[command(subcommand)]
    action: ServiceAction,
}

#[derive(Subcommand)]
enum ServiceAction {
    /// Install the systemd unit file and enable it
    Install {
        /// Path to the rtop binary (default: current executable)
        #[arg(long)]
        bin: Option<std::path::PathBuf>,
        /// Path to the config file passed to the service
        #[arg(long)]
        config: Option<std::path::PathBuf>,
    },
    /// Disable and remove the systemd unit file
    Uninstall,
    /// Show service status via systemctl
    Status,
    /// Start the service
    Start,
    /// Stop the service
    Stop,
    /// Restart the service
    Restart,
}

const UNIT_NAME: &str = "rtop.service";
const UNIT_PATH: &str = "/etc/systemd/system/rtop.service";

pub fn run(args: ServiceArgs) -> Result<()> {
    match args.action {
        ServiceAction::Install { bin, config } => install(bin, config),
        ServiceAction::Uninstall => uninstall(),
        ServiceAction::Status => systemctl("status"),
        ServiceAction::Start => systemctl("start"),
        ServiceAction::Stop => systemctl("stop"),
        ServiceAction::Restart => systemctl("restart"),
    }
}

fn install(bin: Option<std::path::PathBuf>, config: Option<std::path::PathBuf>) -> Result<()> {
    let bin_path = bin.unwrap_or_else(|| {
        std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("/usr/local/bin/rtop"))
    });

    // Resolve the config to explicitly pass it, avoiding "HOME not set" in systemd
    let resolved_config = config.unwrap_or_else(|| {
        app_config::default_config_path()
            .unwrap_or_else(|_| std::path::PathBuf::from("/etc/rtop/config.json"))
    });
    let cfg_arg = format!(" --config {}", resolved_config.display());

    let unit = format!(
        "[Unit]\n\
         Description=rtop telemetry agent and web server\n\
         After=network.target\n\n\
         [Service]\n\
         ExecStart={bin} run{cfg}\n\
         Restart=on-failure\n\
         RestartSec=5\n\n\
         [Install]\n\
         WantedBy=multi-user.target\n",
        bin = bin_path.display(),
        cfg = cfg_arg,
    );

    std::fs::write(UNIT_PATH, unit).with_context(|| format!("writing unit file to {UNIT_PATH}"))?;

    systemctl_raw(&["daemon-reload"])?;
    systemctl_raw(&["enable", "--now", UNIT_NAME])?;
    println!("rtop service installed and started.");
    Ok(())
}

fn uninstall() -> Result<()> {
    systemctl_raw(&["disable", "--now", UNIT_NAME]).ok();
    if std::path::Path::new(UNIT_PATH).exists() {
        std::fs::remove_file(UNIT_PATH).with_context(|| format!("removing {UNIT_PATH}"))?;
    }

    // Also attempt to remove the legacy rtop.service for clean upgrades
    systemctl_raw(&["disable", "--now", "rtop.service"]).ok();
    let old_unit_path = "/etc/systemd/system/rtop.service";
    if std::path::Path::new(old_unit_path).exists() {
        std::fs::remove_file(old_unit_path).ok();
    }

    systemctl_raw(&["daemon-reload"])?;
    println!("rtop service removed.");
    Ok(())
}

fn systemctl(verb: &str) -> Result<()> {
    systemctl_raw(&[verb, UNIT_NAME])
}

fn systemctl_raw(args: &[&str]) -> Result<()> {
    let status = std::process::Command::new("systemctl")
        .args(args)
        .status()
        .context("running systemctl")?;
    if !status.success() {
        bail!("systemctl {} exited with {status}", args.join(" "));
    }
    Ok(())
}
