use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeStatus {
    pub addr: String,
    pub node_id: String,
    pub model: String,
    pub arch: String,
    pub mem_available_kib: u64,
    pub skills_loaded: u64,
    pub seal: String,
}

pub async fn ping_worker(addr: &str) -> Result<NodeStatus> {
    let stream = tokio::time::timeout(Duration::from_millis(1500), TcpStream::connect(addr))
        .await
        .with_context(|| format!("timeout connecting to worker {addr}"))?
        .with_context(|| format!("connecting to worker {addr}"))?;

    let (read_half, mut write_half) = stream.into_split();
    let req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": { "name": "mesh_ping", "arguments": {} }
    })
    .to_string();

    write_half
        .write_all(format!("{req}\n").as_bytes())
        .await
        .with_context(|| format!("writing mesh_ping to {addr}"))?;
    write_half.flush().await?;

    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    let bytes = tokio::time::timeout(Duration::from_millis(1500), reader.read_line(&mut line))
        .await
        .with_context(|| format!("timeout waiting mesh_ping reply from {addr}"))??;

    if bytes == 0 {
        bail!("worker {addr} closed connection without reply");
    }

    let resp: Value =
        serde_json::from_str(line.trim()).with_context(|| format!("invalid JSON from {addr}"))?;
    let sc = &resp["result"]["structuredContent"];
    if !sc.is_object() {
        bail!("missing structuredContent in reply from {addr}: {}", line.trim());
    }

    Ok(NodeStatus {
        addr: addr.to_string(),
        node_id: sc["node_id"].as_str().unwrap_or("unknown").to_string(),
        model: sc["model"].as_str().unwrap_or("unknown").to_string(),
        arch: sc["arch"].as_str().unwrap_or("aarch64").to_string(),
        mem_available_kib: sc["mem_available_kib"].as_u64().unwrap_or(0),
        skills_loaded: sc["skills_loaded"].as_u64().unwrap_or(0),
        seal: sc["seal"].as_str().unwrap_or("").to_string(),
    })
}

pub fn read_hermes_registry(path: &Path) -> Vec<NodeStatus> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<NodeStatus>>(&raw).unwrap_or_default()
}

pub fn write_hermes_registry(nodes: &[NodeStatus], path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating registry dir {}", parent.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    let body = serde_json::to_string_pretty(nodes)?;
    std::fs::write(&tmp, format!("{body}\n"))
        .with_context(|| format!("writing temp registry {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("committing registry {}", path.display()))?;
    Ok(())
}

pub fn upsert_registry_node(status: NodeStatus, path: &Path) -> Result<Vec<NodeStatus>> {
    let mut nodes = read_hermes_registry(path);
    nodes.retain(|n| n.addr != status.addr && n.node_id != status.node_id);
    nodes.push(status);
    nodes.sort_by(|a, b| a.addr.cmp(&b.addr));
    write_hermes_registry(&nodes, path)?;
    Ok(nodes)
}

pub async fn probe_on_lease(ip: Ipv4Addr, port: u16, registry_path: PathBuf) {
    let addr = format!("{ip}:{port}");
    for delay_ms in [500u64, 2000, 5000] {
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        if let Ok(status) = ping_worker(&addr).await {
            eprintln!(
                "[mesh] online: {} ({}, {} KiB free, {} skills) seal={}",
                status.node_id,
                status.addr,
                status.mem_available_kib,
                status.skills_loaded,
                status.seal
            );
            if let Err(err) = upsert_registry_node(status, &registry_path) {
                eprintln!("[mesh] registry write error: {err}");
            }
            return;
        }
    }
    eprintln!("[mesh] lease {addr} did not answer mesh_ping yet (run `ayeos ping --addrs {addr}`)");
}
