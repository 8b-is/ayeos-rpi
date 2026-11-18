// ayeos-rpi / burner / src / pkgmgr.rs — OS-agnostic package-manager detection
// Supports apt / apt-get / apk / brew.

use std::process::Command;

pub struct PackageManager {
    kind: PmKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PmKind {
    Apt,
    AptGet,
    Apk,
    Brew,
}

impl PackageManager {
    pub fn detect() -> Option<Self> {
        // Order matters: prefer the most common per-platform.
        if which("brew").is_some() {
            return Some(Self { kind: PmKind::Brew });
        }
        if which("apt").is_some() {
            return Some(Self { kind: PmKind::Apt });
        }
        if which("apt-get").is_some() {
            return Some(Self { kind: PmKind::AptGet });
        }
        if which("apk").is_some() {
            return Some(Self { kind: PmKind::Apk });
        }
        None
    }

    pub fn name(&self) -> &'static str {
        match self.kind {
            PmKind::Apt => "apt",
            PmKind::AptGet => "apt-get",
            PmKind::Apk => "apk",
            PmKind::Brew => "brew",
        }
    }

    #[allow(dead_code)]
    pub fn install(&self, pkg: &str) -> Result<(), std::io::Error> {
        let cmd = match self.kind {
            PmKind::Apt => "apt",
            PmKind::AptGet => "apt-get",
            PmKind::Apk => "apk",
            PmKind::Brew => "brew",
        };
        let status = if self.kind == PmKind::Apk {
            Command::new(cmd).arg("add").arg(pkg).status()?
        } else if self.kind == PmKind::Brew {
            Command::new(cmd).arg("install").arg(pkg).status()?
        } else {
            Command::new(cmd).arg("install").arg("-y").arg(pkg).status()?
        };
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other("install failed"))
        }
    }
}

fn which(cmd: &str) -> Option<std::path::PathBuf> {
    if let Ok(paths) = std::env::var("PATH") {
        for dir in paths.split(':') {
            let p = std::path::Path::new(dir).join(cmd);
            if p.is_file() && std::fs::metadata(&p).map(|m| m.is_file()).unwrap_or(false) {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detect_returns_some_or_none() {
        // Just asserts it doesn't panic.
        let _ = PackageManager::detect();
    }
}
