// ayeos-rpi / burner / src / main.rs — OS-agnostic SD-card & TFTP/Pi3 staging CLI
// Package-manager detection: apt / apt-get / apk / brew. TUI via ratatui (feature "tui"),
// otherwise plain text mode (compiles offline).
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

use anyhow::{Context, Result};
use clap::Parser;
use std::path::{Path, PathBuf};

mod pkgmgr;
use pkgmgr::PackageManager;

/// Burn ayeos-rpi images or stage LAN netboot TFTP / Pi 3B bootcode.bin directories.
#[derive(Parser, Debug)]
#[command(author, version, about = "ayeos-rpi SD-card burner & LAN netboot stager")]
struct Cli {
    /// Path to kernel8.img (or Linux ARM64 Image)
    #[arg(long, default_value = "../dist/kernel8.img")]
    kernel: PathBuf,
    /// Path to ayeos.img
    #[arg(long, default_value = "../dist/ayeos.img")]
    ayeos: PathBuf,
    /// Path to netboot config directory (config.netboot.txt, cmdline.txt)
    #[arg(long, default_value = "../netboot")]
    netboot_dir: PathBuf,
    /// Path to dist directory (kernel8.img, initramfs.cpio.zst)
    #[arg(long, default_value = "../dist")]
    dist_dir: PathBuf,
    /// Optional path to official Raspberry Pi bootcode.bin for Pi 3B SD staging
    #[arg(long)]
    bootcode: Option<PathBuf>,
    /// Stage TFTP root directory (config.txt, cmdline.txt, kernel8.img, initramfs.cpio.zst)
    #[arg(long)]
    stage_tftp: Option<PathBuf>,
    /// Stage a Pi 3B FAT32 SD bootcode.bin folder safely (no raw block writes)
    #[arg(long)]
    stage_pi3_bootcode: Option<PathBuf>,
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

    if let Some(tftp_dir) = &cli.stage_tftp {
        let kernel_override = cli.kernel.exists().then_some(cli.kernel.as_path());
        stage_tftp_dir(tftp_dir, &cli.netboot_dir, &cli.dist_dir, kernel_override)?;
        eprintln!("[burner] staged TFTP root -> {}", tftp_dir.display());
        if cli.stage_pi3_bootcode.is_none() && cli.device.is_none() {
            return Ok(());
        }
    }

    if let Some(pi3_dir) = &cli.stage_pi3_bootcode {
        let fallback_bootcode = cli.dist_dir.join("bootcode.bin");
        let bootcode_src = cli
            .bootcode
            .as_deref()
            .filter(|p| p.exists())
            .or_else(|| fallback_bootcode.exists().then_some(fallback_bootcode.as_path()));
        stage_pi3_bootcode_dir(pi3_dir, bootcode_src)?;
        eprintln!("[burner] staged Pi 3B netboot folder -> {}", pi3_dir.display());
        if cli.device.is_none() {
            return Ok(());
        }
    }

    let kernel = cli
        .kernel
        .canonicalize()
        .with_context(|| format!("kernel {:?}", cli.kernel))?;
    let ayeos = cli
        .ayeos
        .canonicalize()
        .with_context(|| format!("ayeos {:?}", cli.ayeos))?;

    let device = cli
        .device
        .context("missing --device (or pass --stage-tftp / --stage-pi3-bootcode)")?;

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

pub fn stage_tftp_dir(
    target_dir: &Path,
    netboot_dir: &Path,
    dist_dir: &Path,
    kernel_override: Option<&Path>,
) -> Result<()> {
    std::fs::create_dir_all(target_dir)
        .with_context(|| format!("creating TFTP target dir {}", target_dir.display()))?;

    let config_src = netboot_dir.join("config.netboot.txt");
    let cmdline_src = netboot_dir.join("cmdline.txt");
    let initramfs_src = dist_dir.join("initramfs.cpio.zst");
    let dist_kernel = dist_dir.join("kernel8.img");
    let kernel_src = match kernel_override {
        Some(p) if p.exists() => p,
        _ => dist_kernel.as_path(),
    };

    let pairs = [
        (config_src.as_path(), target_dir.join("config.txt")),
        (cmdline_src.as_path(), target_dir.join("cmdline.txt")),
        (kernel_src, target_dir.join("kernel8.img")),
        (
            initramfs_src.as_path(),
            target_dir.join("initramfs.cpio.zst"),
        ),
    ];

    for (src, dst) in pairs {
        if !src.is_file() {
            anyhow::bail!("missing staging artifact: {}", src.display());
        }
        std::fs::copy(src, &dst)
            .with_context(|| format!("copying {} -> {}", src.display(), dst.display()))?;
    }

    Ok(())
}

pub fn stage_pi3_bootcode_dir(target_dir: &Path, bootcode_src: Option<&Path>) -> Result<()> {
    std::fs::create_dir_all(target_dir)
        .with_context(|| format!("creating Pi 3B staging dir {}", target_dir.display()))?;

    let copied_bootcode = if let Some(src) = bootcode_src {
        if !src.is_file() {
            anyhow::bail!("missing bootcode.bin at {}", src.display());
        }
        std::fs::copy(src, target_dir.join("bootcode.bin"))
            .with_context(|| format!("copying bootcode.bin from {}", src.display()))?;
        true
    } else {
        false
    };

    let note = if copied_bootcode {
        "Pi 3B LAN boot ready: copy bootcode.bin onto a FAT32 SD card (no OTP fuse burn needed).\n"
    } else {
        "Pi 3B LAN boot instructions:\n\
         1. Format a small SD card as FAT32.\n\
         2. Copy official Raspberry Pi firmware bootcode.bin onto the root of that SD card\n\
            (optionally create an empty `timeout` file to extend DHCP retry).\n\
         3. Boot Pi 3B over Ethernet — VideoCore fetches config.txt, cmdline.txt, kernel8.img,\n\
            and initramfs.cpio.zst from the Pi 4B dnsmasq TFTP server without blowing OTP bit 17.\n"
    };
    std::fs::write(target_dir.join("README_PI3_NETBOOT.txt"), note)
        .with_context(|| format!("writing Pi 3B note in {}", target_dir.display()))?;

    Ok(())
}

fn list_devices() -> Result<()> {
    // Best-effort: enumerate /dev/disk* (macOS) and /dev/mmcblk* / /dev/sd* (Linux)
    let mut found = Vec::new();
    for pattern in ["/dev/disk", "/dev/mmcblk", "/dev/sd"] {
        if let Ok(entries) = std::fs::read_dir(
            Path::new(pattern)
                .parent()
                .unwrap_or_else(|| Path::new("/dev")),
        ) {
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

fn burn_text(_kernel: &Path, _ayeos: &Path, _device: &Path, _force: bool) -> Result<()> {
    eprintln!("[burner] text-mode burn not implemented in this scaffold");
    anyhow::bail!("burn_text not implemented");
}

#[cfg(feature = "tui")]
fn burn_tui(_kernel: &Path, _ayeos: &Path, _device: &Path, _force: bool) -> Result<()> {
    eprintln!("[burner] ratatui TUI not implemented in this scaffold");
    anyhow::bail!("burn_tui not implemented");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_tftp_copies_required_netboot_artifacts() {
        let base = std::env::temp_dir().join(format!("burner_stage_test_{}", std::process::id()));
        let netboot = base.join("netboot");
        let dist = base.join("dist");
        let tftp = base.join("tftp");
        std::fs::create_dir_all(&netboot).unwrap();
        std::fs::create_dir_all(&dist).unwrap();

        std::fs::write(netboot.join("config.netboot.txt"), "arm_64bit=1\n").unwrap();
        std::fs::write(netboot.join("cmdline.txt"), "rdinit=/init\n").unwrap();
        std::fs::write(dist.join("kernel8.img"), b"KERN").unwrap();
        std::fs::write(dist.join("initramfs.cpio.zst"), b"ZST").unwrap();

        stage_tftp_dir(&tftp, &netboot, &dist, None).unwrap();

        assert_eq!(
            std::fs::read_to_string(tftp.join("config.txt")).unwrap(),
            "arm_64bit=1\n"
        );
        assert_eq!(
            std::fs::read_to_string(tftp.join("cmdline.txt")).unwrap(),
            "rdinit=/init\n"
        );
        assert_eq!(std::fs::read(tftp.join("kernel8.img")).unwrap(), b"KERN");
        assert_eq!(
            std::fs::read(tftp.join("initramfs.cpio.zst")).unwrap(),
            b"ZST"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn stage_pi3_bootcode_writes_readme_and_optional_bootcode() {
        let base = std::env::temp_dir().join(format!("burner_pi3_test_{}", std::process::id()));
        let src_bin = base.join("bootcode.bin");
        let out_dir = base.join("pi3_sd");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(&src_bin, b"BOOTCODE").unwrap();

        stage_pi3_bootcode_dir(&out_dir, Some(&src_bin)).unwrap();
        assert_eq!(std::fs::read(out_dir.join("bootcode.bin")).unwrap(), b"BOOTCODE");
        assert!(out_dir.join("README_PI3_NETBOOT.txt").is_file());

        let _ = std::fs::remove_dir_all(&base);
    }
}
