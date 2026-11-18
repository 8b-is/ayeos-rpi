// ayeos-rpi / burner / src / main.rs — OS-agnostic SD-card burner CLI
// Package-manager detection: apt / apt-get / apk / brew. TUI via ratatui (feature "tui"),
// otherwise plain text mode (compiles offline).
#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

mod pkgmgr;
use pkgmgr::PackageManager;

/// Burn ayeos-rpi images (kernel8.img + ayeos.img) to an SD card.
#[derive(Parser, Debug)]
#[command(author, version, about = "ayeos-rpi SD-card burner")]
struct Cli {
    /// Path to kernel8.img
    #[arg(long, default_value = "../dist/kernel8.img")]
    kernel: PathBuf,
    /// Path to ayeos.img
    #[arg(long, default_value = "../dist/ayeos.img")]
    ayeos: PathBuf,
    /// Target SD card device (e.g. /dev/disk2 or /dev/mmcblk0)
    #[arg(long, short = 'd')]
    device: Option<PathBuf>,
    /// List block devices and exit
    #[arg(long)]
    list: bool,
    /// Force write without confirmation
    #[arg(long, short = 'f')]
    force: bool,
    /// Use plain text mode (no TUI)
    #[arg(long)]
    text: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Detect available system package manager for installing burner deps.
    let pm = PackageManager::detect();
    if let Some(pm) = &pm {
        eprintln!("[burner] package manager: {}", pm.name());
    } else {
        eprintln!("[burner] no supported package manager found (apt/apt-get/apk/brew)");
    }

    if cli.list {
        return list_devices();
    }

    let kernel = cli.kernel.canonicalize().with_context(|| format!("kernel {:?}", cli.kernel))?;
    let ayeos = cli.ayeos.canonicalize().with_context(|| format!("ayeos {:?}", cli.ayeos))?;

    let device = cli.device.context("missing --device")?;

    if cli.text {
        burn_text(&kernel, &ayeos, &device, cli.force)
    } else {
        #[cfg(feature = "tui")]
        {
            burn_tui(&kernel, &ayeos, &device, cli.force)
        }
        #[cfg(not(feature = "tui"))]
        {
            eprintln!("[burner] ratatui TUI disabled; rerun with --text");
            burn_text(&kernel, &ayeos, &device, cli.force)
        }
    }
}

fn list_devices() -> Result<()> {
    // Best-effort: enumerate /dev/disk* (macOS) and /dev/mmcblk* / /dev/sd* (Linux)
    let mut found = Vec::new();
    for pattern in ["/dev/disk", "/dev/mmcblk", "/dev/sd"] {
        if let Ok(entries) = std::fs::read_dir(std::path::Path::new(pattern).parent().unwrap_or(std::path::Path::new("/dev"))) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with(pattern.rsplit('/').next().unwrap_or("")) {
                    found.push(format!("/dev/{}", name));
                }
            }
        }
    }
    found.sort();
    found.dedup();
    for d in found {
        println!("{}", d);
    }
    Ok(())
}

fn burn_text(_kernel: &std::path::Path, _ayeos: &std::path::Path, _device: &std::path::Path, _force: bool) -> Result<()> {
    eprintln!("[burner] text-mode burn not implemented in this scaffold");
    anyhow::bail!("burn_text not implemented");
}

#[cfg(feature = "tui")]
fn burn_tui(_kernel: &std::path::Path, _ayeos: &std::path::Path, _device: &std::path::Path, _force: bool) -> Result<()> {
    eprintln!("[burner] ratatui TUI not implemented in this scaffold");
    anyhow::bail!("burn_tui not implemented");
}
