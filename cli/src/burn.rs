use anyhow::{bail, Context, Result};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;

pub fn assert_not_system_disk(dev: &Path) -> Result<()> {
    let raw = dev.to_string_lossy().to_ascii_lowercase();
    let name = dev
        .file_name()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_else(|| raw.clone());
    let base = name.strip_prefix('r').unwrap_or(name.as_str());

    let is_apple_sys = base == "disk0"
        || base == "disk1"
        || base.starts_with("disk0s")
        || base.starts_with("disk1s");
    let is_linux_sys = base == "sda"
        || base.starts_with("sda") && base[3..].chars().all(|c| c.is_ascii_digit())
        || base == "nvme0n1"
        || base.starts_with("nvme0n1p");

    if is_apple_sys || is_linux_sys {
        bail!(
            "REFUSING internal/system disk `{}` (hardware safety guard blocked write to disk0/disk1/sda/nvme0n1).",
            dev.display()
        );
    }
    Ok(())
}

pub fn list_removable_disks() -> Result<String> {
    if cfg!(target_os = "macos") {
        let out = Command::new("diskutil")
            .args(["list", "external", "physical"])
            .output()
            .context("running diskutil list external physical")?;
        if !out.status.success() {
            bail!(
                "diskutil failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        return Ok(String::from_utf8_lossy(&out.stdout).to_string());
    }

    let mut found = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/dev") {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("mmcblk") || name.starts_with("sd") {
                let p = format!("/dev/{name}");
                if assert_not_system_disk(Path::new(&p)).is_ok() {
                    found.push(p);
                }
            }
        }
    }
    found.sort();
    Ok(found.join("\n"))
}

pub fn stage_tftp(target_dir: &Path, repo_root: &Path) -> Result<()> {
    let dist_dir = repo_root.join("dist");
    let netboot_dir = repo_root.join("netboot");
    let initramfs = dist_dir.join("initramfs.cpio.zst");

    if !initramfs.is_file() {
        let pack_script = netboot_dir.join("pack_initramfs.sh");
        if !pack_script.is_file() {
            bail!(
                "missing {} and pack script {}",
                initramfs.display(),
                pack_script.display()
            );
        }
        let status = Command::new("bash")
            .arg(&pack_script)
            .status()
            .with_context(|| format!("running {}", pack_script.display()))?;
        if !status.success() {
            bail!("pack_initramfs.sh failed with {status}");
        }
    }

    std::fs::create_dir_all(target_dir)
        .with_context(|| format!("creating TFTP directory {}", target_dir.display()))?;

    let required = [
        (netboot_dir.join("config.netboot.txt"), "config.txt"),
        (netboot_dir.join("cmdline.txt"), "cmdline.txt"),
        (dist_dir.join("kernel8.img"), "kernel8.img"),
        (dist_dir.join("initramfs.cpio.zst"), "initramfs.cpio.zst"),
    ];
    for (src, dst_name) in required {
        if !src.is_file() {
            bail!("missing required netboot file {}", src.display());
        }
        let dst = target_dir.join(dst_name);
        std::fs::copy(&src, &dst)
            .with_context(|| format!("copying {} -> {}", src.display(), dst.display()))?;
    }

    let manifest = dist_dir.join("netboot-manifest.json");
    if manifest.is_file() {
        std::fs::copy(&manifest, target_dir.join("netboot-manifest.json"))?;
    }
    Ok(())
}

pub fn burn_pi3_bootcode_volume(
    mount_dir: &Path,
    bootcode_src: Option<&Path>,
    force_dir: bool,
) -> Result<()> {
    let path_str = mount_dir.to_string_lossy();
    if !force_dir && !path_str.starts_with("/Volumes/") {
        bail!(
            "refusing Pi 3B SD volume target outside /Volumes/: {} (pass --force-dir to override)",
            mount_dir.display()
        );
    }
    std::fs::create_dir_all(mount_dir)
        .with_context(|| format!("preparing volume {}", mount_dir.display()))?;

    if let Some(src) = bootcode_src {
        if !src.is_file() {
            bail!("missing bootcode.bin at {}", src.display());
        }
        std::fs::copy(src, mount_dir.join("bootcode.bin"))
            .with_context(|| format!("copying {}", src.display()))?;
    }
    std::fs::write(mount_dir.join("timeout"), b"")?;

    let note = "Pi 3 Model B FAT32 LAN netboot card (zero OTP fuse burn):\n\
                - Place official Raspberry Pi bootcode.bin here alongside `timeout`.\n\
                - Plug Pi 3B into the MacBook USB-dongle Ethernet switch and run `sudo ayeos launch --iface <enX>`.\n";
    std::fs::write(mount_dir.join("README_PI3_NETBOOT.txt"), note)?;
    Ok(())
}

pub fn burn_raw_image(image_path: &Path, device_path: &Path, yes: bool) -> Result<u64> {
    assert_not_system_disk(device_path)?;
    if !yes {
        bail!(
            "refusing raw write to {} without explicit --yes confirmation",
            device_path.display()
        );
    }
    if !image_path.is_file() {
        bail!("missing image file {}", image_path.display());
    }

    if cfg!(target_os = "macos") {
        let status = Command::new("diskutil")
            .arg("unmountDisk")
            .arg(device_path)
            .status()
            .with_context(|| format!("running diskutil unmountDisk {}", device_path.display()))?;
        if !status.success() {
            bail!("diskutil unmountDisk {} failed", device_path.display());
        }
    }

    let mut src = OpenOptions::new()
        .read(true)
        .open(image_path)
        .with_context(|| format!("opening image {}", image_path.display()))?;
    let mut dst = OpenOptions::new()
        .write(true)
        .open(device_path)
        .with_context(|| format!("opening target device {}", device_path.display()))?;

    let mut buf = vec![0u8; 1024 * 1024];
    let mut total: u64 = 0;
    loop {
        let n = src.read(&mut buf)?;
        if n == 0 {
            break;
        }
        dst.write_all(&buf[..n])?;
        total = total.saturating_add(n as u64);
    }
    dst.sync_all()?;
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_apple_internal_system_disks() {
        assert!(assert_not_system_disk(Path::new("/dev/disk0")).is_err());
        assert!(assert_not_system_disk(Path::new("/dev/rdisk0")).is_err());
        assert!(assert_not_system_disk(Path::new("/dev/disk1")).is_err());
        assert!(assert_not_system_disk(Path::new("/dev/disk1s2")).is_err());
        assert!(assert_not_system_disk(Path::new("/dev/sda")).is_err());
        assert!(assert_not_system_disk(Path::new("/dev/nvme0n1")).is_err());
        assert!(assert_not_system_disk(Path::new("/dev/rdisk4")).is_ok());
        assert!(assert_not_system_disk(Path::new("/dev/disk10")).is_ok());
    }
}
