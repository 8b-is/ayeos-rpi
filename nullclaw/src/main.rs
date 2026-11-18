// ayeos-rpi / nullclaw / src / main.rs — rpi nullclaw agent (on-device + LAN mesh worker)
// Reads skills from ~/.agents/skills/ (or /etc/agents/skills on initramfs),
// exposes newline-delimited JSON-RPC 2.0 MCP over TCP (--listen) or stdio (--stdio).
// Target: aarch64-unknown-linux-musl (static RAM initramfs on Pi 3B / Pi 4B).
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

use anyhow::{Context, Result};
use clap::Parser;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::signal;

#[derive(Parser, Debug)]
#[command(author, version, about = "rpi nullclaw on-device & LAN mesh worker")]
struct Cli {
    /// Mesh node identifier advertised to Hermes coordinator
    #[arg(long, default_value = "rpi-node-1")]
    node_id: String,
    /// Skills directory (defaults to ~/.agents/skills)
    #[arg(long, default_value = "~/.agents/skills")]
    skills_dir: PathBuf,
    /// Serial TTY fallback when neither --listen nor --stdio is set
    #[arg(long, default_value = "/dev/ttyAMA0")]
    tty: PathBuf,
    /// Serial TTY baud rate
    #[arg(long, default_value = "115200")]
    baud: u32,
    /// Listen on TCP host:port for newline-delimited JSON-RPC 2.0 MCP calls
    #[arg(long)]
    listen: Option<String>,
    /// Serve newline-delimited JSON-RPC 2.0 MCP over stdin/stdout
    #[arg(long)]
    stdio: bool,
    /// Handle a single TCP connection (until EOF) and exit cleanly
    #[arg(long)]
    once: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RationalPq {
    pub p: u64,
    pub q: u64,
}

impl RationalPq {
    pub const fn gcd_u64(mut a: u64, mut b: u64) -> u64 {
        while b != 0 {
            let r = a % b;
            a = b;
            b = r;
        }
        a
    }

    pub const fn reduced(p: u64, q: u64) -> Self {
        if q == 0 {
            return Self { p: 0, q: 1 };
        }
        if p == 0 {
            return Self { p: 0, q: 1 };
        }
        let g = Self::gcd_u64(p, q);
        Self { p: p / g, q: q / g }
    }
}

/// Rung 0 Bonobo resonance trit over exact integers (`p/q >= 3/2` => `+1`).
pub const fn bonobo_resonance_trit(raw_bytes: u64, packed_bytes: u64) -> i8 {
    if raw_bytes == 0 || packed_bytes == 0 || raw_bytes == packed_bytes {
        return 0;
    }
    if raw_bytes.saturating_mul(2) >= packed_bytes.saturating_mul(3) {
        1
    } else {
        -1
    }
}

/// Deterministic 64-bit XOR-rotate fold seeded at the 0x62 gate.
pub fn gate_0x62_seal(payload: &[u8]) -> u64 {
    let mut acc: u64 = 0x6262_6262_6262_6262;
    for (idx, &byte) in payload.iter().enumerate() {
        let lane = ((idx as u64) & 7).wrapping_mul(8);
        acc = acc.rotate_left(7) ^ ((byte as u64) << lane) ^ 0x62;
    }
    (acc & !0xFF) | 0x62
}

#[derive(Debug, Clone, Serialize)]
struct Skill {
    name: String,
    description: String,
    path: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let skills_dir = shellexpand::tilde(&cli.skills_dir.to_string_lossy())
        .parse::<PathBuf>()
        .context("invalid skills dir")?;

    let skills = load_skills(&skills_dir)?;
    eprintln!(
        "[nullclaw] node_id={} skills_dir={} loaded={}",
        cli.node_id,
        skills_dir.display(),
        skills.len()
    );

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    tokio::spawn(async move {
        let _ = signal::ctrl_c().await;
        r.store(false, Ordering::SeqCst);
    });

    if cli.stdio {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        serve_stream(
            BufReader::new(stdin),
            stdout,
            &cli.node_id,
            &skills,
            &running,
        )
        .await?;
    } else if let Some(addr) = &cli.listen {
        let listener = TcpListener::bind(addr)
            .await
            .with_context(|| format!("binding TCP listener on {addr}"))?;
        eprintln!("[nullclaw] MCP TCP listening on {addr}");
        serve_tcp_listener(listener, &cli.node_id, &skills, cli.once, running).await?;
    } else {
        eprintln!("[nullclaw] tty={} @ {}", cli.tty.display(), cli.baud);
        serve_tty(&cli.tty, cli.baud, &skills, running).await?;
    }

    eprintln!("[nullclaw] shutdown");
    Ok(())
}

async fn serve_tcp_listener(
    listener: TcpListener,
    node_id: &str,
    skills: &[Skill],
    once: bool,
    running: Arc<AtomicBool>,
) -> Result<()> {
    while running.load(Ordering::SeqCst) {
        let accept_fut = listener.accept();
        let (stream, peer) = tokio::select! {
            res = accept_fut => res?,
            _ = wait_until_stopped(&running) => break,
        };
        eprintln!("[nullclaw] peer connected: {peer}");
        let (read_half, write_half) = stream.into_split();
        serve_stream(
            BufReader::new(read_half),
            write_half,
            node_id,
            skills,
            &running,
        )
        .await?;
        if once {
            break;
        }
    }
    Ok(())
}

async fn wait_until_stopped(running: &AtomicBool) {
    while running.load(Ordering::SeqCst) {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

async fn serve_stream<R, W>(
    mut reader: R,
    mut writer: W,
    node_id: &str,
    skills: &[Skill],
    running: &AtomicBool,
) -> Result<()>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut line = String::new();
    while running.load(Ordering::SeqCst) {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;
        if bytes_read == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(response) = handle_jsonrpc_line(trimmed, node_id, skills) {
            writer.write_all(response.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;
        }
    }
    Ok(())
}

fn handle_jsonrpc_line(raw_line: &str, node_id: &str, skills: &[Skill]) -> Option<String> {
    let req: Value = match serde_json::from_str(raw_line) {
        Ok(v) => v,
        Err(err) => {
            return Some(
                json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": { "code": -32700, "message": format!("parse error: {err}") }
                })
                .to_string(),
            );
        }
    };

    let id = req.get("id").cloned();
    let method = req.get("method").and_then(Value::as_str).unwrap_or("");

    if method == "notifications/initialized" && id.is_none() {
        return None;
    }

    let id_val = id?;
    let params = req.get("params");

    let reply = match method {
        "initialize" => json!({
            "jsonrpc": "2.0",
            "id": id_val,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": {
                    "name": "nullclaw",
                    "version": env!("CARGO_PKG_VERSION"),
                    "nodeId": node_id,
                }
            }
        }),
        "tools/list" => json!({
            "jsonrpc": "2.0",
            "id": id_val,
            "result": {
                "tools": [
                    {
                        "name": "mesh_ping",
                        "description": "Report Raspberry Pi node health, RAM KiB, skill count, 0x62 seal, and exact p/q wire ratio.",
                        "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
                    },
                    {
                        "name": "skills_list",
                        "description": "List locally loaded g8t / superpowers SKILL.md manifests.",
                        "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
                    },
                    {
                        "name": "soc_probe",
                        "description": "Return version-anchored SoC model, kernel version, serial, and integer millidegree temperature.",
                        "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
                    }
                ]
            }
        }),
        "tools/call" => {
            let tool_name = params
                .and_then(|p| p.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("");
            match dispatch_tool(tool_name, raw_line.len() as u64, node_id, skills) {
                Ok(structured) => {
                    let text = structured.to_string();
                    json!({
                        "jsonrpc": "2.0",
                        "id": id_val,
                        "result": {
                            "content": [{ "type": "text", "text": text }],
                            "structuredContent": structured,
                            "isError": false
                        }
                    })
                }
                Err((code, msg)) => json!({
                    "jsonrpc": "2.0",
                    "id": id_val,
                    "error": { "code": code, "message": msg }
                }),
            }
        }
        other => json!({
            "jsonrpc": "2.0",
            "id": id_val,
            "error": { "code": -32601, "message": format!("unknown method: {other}") }
        }),
    };

    Some(reply.to_string())
}

fn dispatch_tool(
    tool_name: &str,
    raw_req_bytes: u64,
    node_id: &str,
    skills: &[Skill],
) -> std::result::Result<Value, (i64, String)> {
    match tool_name {
        "mesh_ping" => {
            let model = read_trimmed_or("/proc/device-tree/model", "host-sim");
            let mem_available_kib = read_mem_available_kib();
            let seal_u64 = gate_0x62_seal(node_id.as_bytes());
            let seal = format!("0x62:{seal_u64:016x}");
            let base = json!({
                "node_id": node_id,
                "model": model,
                "arch": std::env::consts::ARCH,
                "os": std::env::consts::OS,
                "mem_available_kib": mem_available_kib,
                "skills_loaded": skills.len(),
                "seal": seal,
            });
            let compact_resp_bytes = base.to_string().len() as u64;
            let pq = RationalPq::reduced(raw_req_bytes, compact_resp_bytes);
            let bonobo_trit = bonobo_resonance_trit(raw_req_bytes, compact_resp_bytes);
            let mut out = base;
            if let Some(obj) = out.as_object_mut() {
                obj.insert(
                    "wire_pq".to_string(),
                    json!({
                        "raw_req_bytes": raw_req_bytes,
                        "compact_resp_bytes": compact_resp_bytes,
                        "p": pq.p,
                        "q": pq.q,
                        "bonobo_trit": bonobo_trit,
                    }),
                );
            }
            Ok(out)
        }
        "skills_list" => {
            let entries: Vec<Value> = skills
                .iter()
                .map(|s| {
                    json!({
                        "name": s.name,
                        "description": s.description,
                        "path": s.path,
                    })
                })
                .collect();
            Ok(json!({
                "node_id": node_id,
                "count": entries.len(),
                "skills": entries,
            }))
        }
        "soc_probe" => {
            let model = read_trimmed_or("/proc/device-tree/model", "host-sim");
            let kernel = read_trimmed_or(
                "/proc/version",
                &format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH),
            );
            let temp_millic = std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp")
                .ok()
                .and_then(|s| s.trim().parse::<i64>().ok());
            let serial = read_cpuinfo_field("Serial");
            let hardware = read_cpuinfo_field("Hardware");
            Ok(json!({
                "node_id": node_id,
                "model": model,
                "kernel": kernel,
                "hardware": hardware,
                "serial": serial,
                "temp_millic": temp_millic,
                "mem_available_kib": read_mem_available_kib(),
            }))
        }
        other => Err((-32602, format!("unknown tool: {other}"))),
    }
}

fn read_trimmed_or(path: &str, fallback: &str) -> String {
    match std::fs::read_to_string(path) {
        Ok(raw) => {
            let cleaned = raw.trim_matches(|c: char| c == '\0' || c.is_whitespace());
            if cleaned.is_empty() {
                fallback.to_string()
            } else {
                cleaned.to_string()
            }
        }
        Err(_) => fallback.to_string(),
    }
}

fn read_mem_available_kib() -> u64 {
    let Ok(content) = std::fs::read_to_string("/proc/meminfo") else {
        return 0;
    };
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("MemAvailable:") {
            if let Some(num_str) = rest.split_whitespace().next() {
                if let Ok(val) = num_str.parse::<u64>() {
                    return val;
                }
            }
        }
    }
    0
}

fn read_cpuinfo_field(field: &str) -> Option<String> {
    let content = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    for line in content.lines() {
        if let Some((k, v)) = line.split_once(':') {
            if k.trim() == field {
                let trimmed = v.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

fn load_skills(dir: &Path) -> Result<Vec<Skill>> {
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
                    .with_context(|| format!("reading {:?}", skill_md))?;
                if let Some((name, desc)) = parse_frontmatter(&content) {
                    skills.push(Skill {
                        name,
                        description: desc,
                        path,
                    });
                }
            }
        }
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));
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
            if trimmed.is_empty()
                || trimmed.starts_with("name:")
                || trimmed.starts_with("license:")
                || trimmed.starts_with("version:")
            {
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
            description = Some(first);
            in_desc = continues;
        } else if line.strip_prefix("license:").is_some() || line.strip_prefix("version:").is_some()
        {
            in_desc = false;
        }
    }
    match (name, description) {
        (Some(n), Some(d)) => Some((n, d)),
        _ => None,
    }
}

async fn serve_tty(
    _tty: &Path,
    _baud: u32,
    _skills: &[Skill],
    running: Arc<AtomicBool>,
) -> Result<()> {
    eprintln!("[nullclaw] TTY MCP server standby (pass --listen or --stdio for active MCP)");
    wait_until_stopped(&running).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpStream;

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

    #[test]
    fn rational_pq_reduces_and_bonobo_trit_exact() {
        let r = RationalPq::reduced(300, 200);
        assert_eq!(r, RationalPq { p: 3, q: 2 });
        assert_eq!(bonobo_resonance_trit(300, 200), 1);
        assert_eq!(bonobo_resonance_trit(299, 200), -1);
        assert_eq!(bonobo_resonance_trit(200, 200), 0);
        assert_eq!(RationalPq::reduced(10, 0), RationalPq { p: 0, q: 1 });
    }

    #[test]
    fn gate_0x62_seal_has_low_byte_0x62_and_is_deterministic() {
        let a = gate_0x62_seal(b"rpi-node-1");
        let b = gate_0x62_seal(b"rpi-node-1");
        let c = gate_0x62_seal(b"rpi-node-2");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a & 0xFF, 0x62);
    }

    #[tokio::test]
    async fn tcp_jsonrpc_mesh_ping_and_skills_list_roundtrip() {
        let temp_root = std::env::temp_dir().join(format!(
            "nullclaw_test_skills_{}_{}",
            std::process::id(),
            gate_0x62_seal(b"tcp-test")
        ));
        let sbc_dir = temp_root.join("sbc-pro");
        std::fs::create_dir_all(&sbc_dir).unwrap();
        std::fs::write(
            sbc_dir.join("SKILL.md"),
            "---\nname: sbc-pro\ndescription: >\n  Raspberry Pi board engineering\n---\n",
        )
        .unwrap();

        let skills = load_skills(&temp_root).unwrap();
        assert_eq!(skills.len(), 1);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let running = Arc::new(AtomicBool::new(true));
        let server_task = tokio::spawn({
            let running = running.clone();
            let skills = skills.clone();
            async move { serve_tcp_listener(listener, "pi3-test", &skills, true, running).await }
        });

        let stream = TcpStream::connect(addr).await.unwrap();
        let (read_half, mut write_half) = stream.into_split();
        let mut reader = BufReader::new(read_half);

        let ping_req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": { "name": "mesh_ping", "arguments": {} }
        })
        .to_string();
        write_half
            .write_all(format!("{ping_req}\n").as_bytes())
            .await
            .unwrap();
        write_half.flush().await.unwrap();

        let mut ping_line = String::new();
        reader.read_line(&mut ping_line).await.unwrap();
        let ping_resp: Value = serde_json::from_str(&ping_line).unwrap();
        let ping_sc = &ping_resp["result"]["structuredContent"];
        assert_eq!(ping_sc["node_id"], "pi3-test");
        assert!(ping_sc["seal"].as_str().unwrap().starts_with("0x62:"));
        assert_eq!(ping_sc["skills_loaded"], 1);

        let skills_req = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": { "name": "skills_list", "arguments": {} }
        })
        .to_string();
        write_half
            .write_all(format!("{skills_req}\n").as_bytes())
            .await
            .unwrap();
        write_half.flush().await.unwrap();

        let mut skills_line = String::new();
        reader.read_line(&mut skills_line).await.unwrap();
        let skills_resp: Value = serde_json::from_str(&skills_line).unwrap();
        let skills_sc = &skills_resp["result"]["structuredContent"];
        assert_eq!(skills_sc["count"], 1);
        assert_eq!(skills_sc["skills"][0]["name"], "sbc-pro");

        drop(write_half);
        drop(reader);
        server_task.await.unwrap().unwrap();
        let _ = std::fs::remove_dir_all(&temp_root);
    }
}
