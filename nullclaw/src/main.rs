// ayeos-rpi / nullclaw / src / main.rs — rpi nullclaw agent (on-device assistance)
// Reads skills from ~/.agents/skills/ (or /mnt/skills on-device), exposes a local
// MCP-over-TTY/serial interface for on-device help.
// Target: aarch64-unknown-linux-gnu (Pi 3/4 userspace under ayeOS/linux).
#![forbid(unsafe_code)]

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::signal;

#[derive(Parser, Debug)]
#[command(author, version, about = "rpi nullclaw on-device assistance agent")]
struct Cli {
    /// Skills directory (defaults to ~/.agents/skills)
    #[arg(long, default_value = "~/.agents/skills")]
    skills_dir: PathBuf,
    /// Serial TTY to expose MCP on (e.g. /dev/ttyAMA0)
    #[arg(long, default_value = "/dev/ttyAMA0")]
    tty: PathBuf,
    /// Baud rate
    #[arg(long, default_value = "115200")]
    baud: u32,
}

static RUNNING: AtomicBool = AtomicBool::new(true);

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let skills_dir = shellexpand::tilde(&cli.skills_dir.to_string_lossy())
        .parse::<std::path::PathBuf>()
        .context("invalid skills dir")?;

    eprintln!("[nullclaw] skills_dir = {}", skills_dir.display());
    eprintln!("[nullclaw] tty = {} @ {}", cli.tty.display(), cli.baud);

    // Load available skills (index SKILL.md frontmatter).
    let skills = load_skills(&skills_dir)?;
    eprintln!("[nullclaw] loaded {} skills", skills.len());

    // Set up signal handler for graceful shutdown.
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    tokio::spawn(async move {
        let _ = signal::ctrl_c().await;
        r.store(false, Ordering::SeqCst);
    });

    // Expose MCP-over-TTY loop.
    serve_tty(&cli.tty, cli.baud, &skills, running).await?;

    eprintln!("[nullclaw] shutdown");
    Ok(())
}

struct Skill {
    name: String,
    description: String,
    path: PathBuf,
}

fn load_skills(dir: &std::path::Path) -> Result<Vec<Skill>> {
    let mut skills = Vec::new();
    if !dir.is_dir() {
        return Ok(skills);
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let skill_md = path.join("SKILL.md");
            if skill_md.is_file() {
                let content = std::fs::read_to_string(&skill_md)
                    .context(format!("reading {:?}", skill_md))?;
                if let Some((name, desc)) = parse_frontmatter(&content) {
                    skills.push(Skill { name, description: desc, path });
                }
            }
        }
    }
    Ok(skills)
}
fn parse_frontmatter(content: &str) -> Option<(String, String)> {
    let mut lines = content.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut name = None;
    let mut description = None;
    let mut in_desc = false;
    for line in lines.by_ref() {
        if line.trim() == "---" {
            break;
        }
        if in_desc {
            // Continuation of a folded block scalar: indented lines belong to description.
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("name:") || trimmed.starts_with("license:") || trimmed.starts_with("version:") {
                in_desc = false;
                // fall through to re-process this line below
            } else {
                let d = description.get_or_insert_with(String::new);
                if !d.is_empty() {
                    d.push(' ');
                }
                d.push_str(trimmed);
                continue;
            }
        }
        if let Some(rest) = line.strip_prefix("name:") {
            name = Some(rest.trim().to_string());
            in_desc = false;
        } else if let Some(rest) = line.strip_prefix("description:") {
            let trimmed = rest.trim();
            let first = trimmed.trim_matches('>').trim().to_string();
            let continues = !first.is_empty() || trimmed.ends_with('>');
            description = Some(if first.is_empty() { String::new() } else { first });
            in_desc = continues;
        } else if line.strip_prefix("license:").is_some() || line.strip_prefix("version:").is_some() {
            in_desc = false;
        }
    }
    match (name, description) {
        (Some(n), Some(d)) => Some((n, d)),
        _ => None,
    }
}

async fn serve_tty(
    _tty: &std::path::Path,
    _baud: u32,
    _skills: &[Skill],
    _running: Arc<AtomicBool>,
) -> Result<()> {
    // Placeholder: in a real build, open the TTY, frame MCP messages,
    // and route nullclaw requests to the loaded skills.
    eprintln!("[nullclaw] TTY MCP server placeholder (no-op)");
    while _running.load(Ordering::SeqCst) {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_frontmatter_works() {
        let content = "---\nname: esp32-pro\ndescription: >\n  ESP32 firmware\nlicense: MIT\n---\n";
        let (n, d) = parse_frontmatter(content).unwrap();
        assert_eq!(n, "esp32-pro");
        assert_eq!(d, "ESP32 firmware");
    }
    #[test]
    fn no_frontmatter_returns_none() {
        assert!(parse_frontmatter("just text").is_none());
    }
}
