// ayeos-rpi / cli / src / constellation.rs — Wires 8b-is/ultrameshai + full 8b-is constellation
use crate::mesh::NodeStatus;
use crate::tftp::gate_0x62_seal;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Debug, Clone, Serialize)]
pub struct RepoEntry {
    pub name: &'static str,
    pub github_url: &'static str,
    pub local_dirs: &'static [&'static str],
    pub role: &'static str,
    pub default_port: Option<u16>,
}

pub const RELEVANT_8B_IS_REPOS: &[RepoEntry] = &[
    RepoEntry {
        name: "ultrameshai",
        github_url: "https://github.com/8b-is/ultrameshai",
        local_dirs: &["8b-is-repos/ultrameshai", "ultrameshai"],
        role: "Layer 1-4 capability router (topology.toml + node-registry + custodian provenance)",
        default_port: Some(3010),
    },
    RepoEntry {
        name: "foken",
        github_url: "https://github.com/8b-is/foken",
        local_dirs: &["foken"],
        role: "Token-savings & provenance API + Foken.ai web service",
        default_port: Some(3000),
    },
    RepoEntry {
        name: "ayeos-rpi",
        github_url: "https://github.com/8b-is/ayeos-rpi",
        local_dirs: &["8b-is-repos/ayeos-rpi"],
        role: "USB-dongle PXE/DHCP+TFTP launcher, nullclaw :18815 worker, bare-metal kernel8/ayeOS",
        default_port: Some(18815),
    },
    RepoEntry {
        name: "g8t-plugins",
        github_url: "https://github.com/8b-is/g8t-plugins",
        local_dirs: &["g8t-plugins"],
        role: "Epistemic SBC/ARM/ASM skill registry (sbc-pro, arm-master, esp32-pro, assembly-pro, retro-game-dev)",
        default_port: None,
    },
    RepoEntry {
        name: "g8t",
        github_url: "https://github.com/8b-is/g8t",
        local_dirs: &["g8t"],
        role: "AI-first Git forge + Capsela + g8t.is Foken cycle-golf leaderboard",
        default_port: Some(8430),
    },
    RepoEntry {
        name: "ultra-graph",
        github_url: "https://github.com/8b-is/ultra-graph",
        local_dirs: &["ultra-graph"],
        role: "1.58-bit ternary byte-graph LLM MCP server (stdio)",
        default_port: None,
    },
    RepoEntry {
        name: "smart-tree",
        github_url: "https://github.com/8b-is/smart-tree",
        local_dirs: &["8b-is-repos/smart-tree", "smart-tree"],
        role: "AST-aware codebase context + MEM8 wave memory (`st --mcp`)",
        default_port: None,
    },
    RepoEntry {
        name: "eightb-agy-fanout",
        github_url: "https://github.com/8b-is/eightb-agy-fanout",
        local_dirs: &["eightb-agy-fanout"],
        role: "BitmaskDag + 64B@0x62 LanFrame64 UDP :18815 + Ollama Wave Router",
        default_port: None,
    },
    RepoEntry {
        name: "kompress-super-ultra",
        github_url: "https://github.com/8b-is/kompress-super-ultra",
        local_dirs: &["kompress-super-ultra"],
        role: "4-role semantic fold context compressor & live WS core",
        default_port: Some(8799),
    },
    RepoEntry {
        name: "bonobo",
        github_url: "https://github.com/8b-is/bonobo",
        local_dirs: &["bonobo", "caveman"],
        role: "Caveman / Bonobo reverse proxy & Rung-0 p/q = 3/2 gateway",
        default_port: Some(8787),
    },
    RepoEntry {
        name: "omni-todos",
        github_url: "https://github.com/8b-is/omni-todos",
        local_dirs: &["8b-is-repos/omni-todos", "omni-todos"],
        role: "Autonomous DAG task ledger REST + MCP server",
        default_port: Some(8099),
    },
    RepoEntry {
        name: "wa-stream",
        github_url: "https://github.com/8b-is/wa-stream",
        local_dirs: &["wa-stream"],
        role: "Full-duplex 80ms WhatsApp/Discord leader socket & wa-mcp.mjs",
        default_port: Some(8790),
    },
    RepoEntry {
        name: "omnispherepop",
        github_url: "https://github.com/8b-is/omnispherepop",
        local_dirs: &["omnispherepop"],
        role: "Weight programming CLI & ospop-mcp",
        default_port: None,
    },
    RepoEntry {
        name: "ayeos",
        github_url: "https://github.com/8b-is/ayeos",
        local_dirs: &["ayeos-mcp"],
        role: "Ternary matrix inference daemon (`ayeosd` + `ayeos-mcp`)",
        default_port: Some(9876),
    },
    RepoEntry {
        name: "upz",
        github_url: "https://github.com/8b-is/upz",
        local_dirs: &["upz", "up", "wupz"],
        role: "Zig/Rust incremental packer, LitanyWireFrame 64B@0x62 & `up` helper",
        default_port: None,
    },
    RepoEntry {
        name: "felix",
        github_url: "https://github.com/8b-is/felix",
        local_dirs: &["8b-is-repos/felix", "felix", "felix-codegen"],
        role: "Zero-float NAND-SSA / Ada SPARK verified codegen",
        default_port: None,
    },
    RepoEntry {
        name: "a6000-orchestrator",
        github_url: "https://github.com/8b-is/a6000-orchestrator",
        local_dirs: &["a6000-orchestrator"],
        role: "Remote RTX 6000 Ada compute orchestrator",
        default_port: None,
    },
    RepoEntry {
        name: "sovereign-bootstrap",
        github_url: "https://github.com/8b-is/sovereign-bootstrap",
        local_dirs: &["sovereign-bootstrap"],
        role: "One-shot workspace bootstrap & Homebrew tap",
        default_port: None,
    },
    RepoEntry {
        name: "mempalace",
        github_url: "https://github.com/8b-is/mempalace",
        local_dirs: &["mempalace"],
        role: "Long-term associative memory & `mempalace-mcp`",
        default_port: None,
    },
    RepoEntry {
        name: "dark-claw",
        github_url: "https://github.com/8b-is/dark-claw",
        local_dirs: &["dark-claw"],
        role: "Rust agent workspace & schema contracts",
        default_port: None,
    },
    RepoEntry {
        name: "space-bender",
        github_url: "https://github.com/8b-is/space-bender",
        local_dirs: &["8b-is-repos/space-bender", "space-bender"],
        role: "Kata verification engine (`space-bender` MCP)",
        default_port: None,
    },
];

#[derive(Debug, Clone, Serialize)]
pub struct ServiceProbe {
    pub id: String,
    pub host: String,
    pub port: u16,
    pub up: bool,
    pub capability: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BinaryProbe {
    pub name: String,
    pub path: Option<String>,
    pub present: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepoStatus {
    pub name: String,
    pub github_url: String,
    pub local_path: Option<String>,
    pub present_locally: bool,
    pub role: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConstellationManifest {
    pub spec: String,
    pub seal: String,
    pub coordinator_ip: String,
    pub ultrameshai_topology_path: String,
    pub services: Vec<ServiceProbe>,
    pub binaries: Vec<BinaryProbe>,
    pub pi_workers: Vec<NodeStatus>,
    pub repos: Vec<RepoStatus>,
}

const SERVICE_TARGETS: &[(&str, &str, u16, &str)] = &[
    ("honcho", "127.0.0.1", 8000, "honcho-memory"),
    ("foken", "127.0.0.1", 3000, "foken"),
    ("foken-sys", "127.0.0.1", 3001, "foken-sys"),
    ("foken-web", "127.0.0.1", 8080, "foken-web"),
    ("foken-bridge", "127.0.0.1", 8088, "foken-bridge"),
    ("ultrameshai-registry", "127.0.0.1", 3010, "ultrameshai-registry"),
    ("dyad-studio", "127.0.0.1", 8089, "dyad-studio"),
    ("omni-todos", "127.0.0.1", 8099, "omni-todos"),
    ("bonobo-gateway", "127.0.0.1", 8787, "bonobo-gateway"),
    ("wa-stream", "127.0.0.1", 8790, "wa-stream"),
    ("kompress-super-ultra", "127.0.0.1", 8799, "kompress-super-ultra"),
    ("ayeosd", "127.0.0.1", 9876, "ayeos-matrix"),
    ("ollama-fleet", "127.0.0.1", 11434, "ollama-fleet"),
];

const BINARY_TARGETS: &[&str] = &[
    "st",
    "8b-agy",
    "ayeos",
    "ayeosd",
    "g8t",
    "ospop",
    "ospop-mcp",
    "mempalace-mcp",
    "up",
    "pldev",
    "felix",
    "ollama",
];

async fn tcp_port_up(host: &str, port: u16) -> bool {
    let addr = format!("{host}:{port}");
    tokio::time::timeout(Duration::from_millis(150), TcpStream::connect(&addr))
        .await
        .map(|r| r.is_ok())
        .unwrap_or(false)
}

pub fn find_binary(name: &str) -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if let Some(h) = &home {
        for dir in [h.join(".cargo/bin"), h.join(".local/bin")] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

pub async fn probe_services() -> Vec<ServiceProbe> {
    let mut out = Vec::with_capacity(SERVICE_TARGETS.len());
    for &(id, host, port, cap) in SERVICE_TARGETS {
        let up = tcp_port_up(host, port).await;
        out.push(ServiceProbe {
            id: id.to_string(),
            host: host.to_string(),
            port,
            up,
            capability: cap.to_string(),
        });
    }
    out
}

pub fn probe_binaries() -> Vec<BinaryProbe> {
    BINARY_TARGETS
        .iter()
        .map(|&name| {
            let path = find_binary(name).map(|p| p.display().to_string());
            BinaryProbe {
                name: name.to_string(),
                present: path.is_some(),
                path,
            }
        })
        .collect()
}

pub fn probe_repos(workspace_root: &Path) -> Vec<RepoStatus> {
    RELEVANT_8B_IS_REPOS
        .iter()
        .map(|r| {
            let found = r
                .local_dirs
                .iter()
                .map(|rel| workspace_root.join(rel))
                .find(|p| p.is_dir());
            RepoStatus {
                name: r.name.to_string(),
                github_url: r.github_url.to_string(),
                local_path: found.as_ref().map(|p| p.display().to_string()),
                present_locally: found.is_some(),
                role: r.role.to_string(),
            }
        })
        .collect()
}

fn extract_ip(addr: &str) -> Option<IpAddr> {
    if let Ok(sock) = addr.parse::<SocketAddr>() {
        return Some(sock.ip());
    }
    if let Ok(ip) = addr.parse::<IpAddr>() {
        return Some(ip);
    }
    addr.split(':').next()?.parse::<IpAddr>().ok()
}

fn format_toml_array(items: &[String]) -> String {
    let quoted: Vec<String> = items
        .iter()
        .map(|s| format!("\"{}\"", s.replace('"', "\\\"")))
        .collect();
    format!("[{}]", quoted.join(", "))
}

pub fn build_ultramesh_topology_toml(
    coordinator_ip: &str,
    services: &[ServiceProbe],
    binaries: &[BinaryProbe],
    pi_workers: &[NodeStatus],
    a6000_ip: Option<&str>,
) -> String {
    let mut coord_caps: Vec<String> = vec![
        "hermes".to_string(),
        "ultrameshai".to_string(),
        "ayeos-rpi".to_string(),
        "ultragraph".to_string(),
        "g8t-plugins".to_string(),
        "sdd-cavecrew".to_string(),
        "pxe-dhcp-tftp".to_string(),
    ];
    for svc in services {
        if svc.up {
            coord_caps.push(svc.capability.clone());
        }
    }
    for bin in binaries {
        if bin.present {
            coord_caps.push(format!("bin:{}", bin.name));
        }
    }
    coord_caps.sort();
    coord_caps.dedup();

    let mut out = format!(
        "# Auto-generated by `ayeos mesh sync-topology` for 8b-is/ultrameshai ClusterTopologyRouter\n\
         coordinator_ip = \"{coordinator_ip}\"\n\
         coordinator_port = 8080\n\n\
         [[nodes]]\n\
         ip = \"{coordinator_ip}\"\n\
         role = \"Compute\"\n\
         capabilities = {}\n\
         runtime = \"RustNative\"\n",
        format_toml_array(&coord_caps)
    );

    if let Some(gpu_ip) = a6000_ip {
        if let Some(parsed) = extract_ip(gpu_ip) {
            let gpu_caps = vec![
                "cuda".to_string(),
                "rtx-6000-ada".to_string(),
                "llm-inference".to_string(),
                "a6000-orchestrator".to_string(),
            ];
            out.push_str(&format!(
                "\n[[nodes]]\nip = \"{parsed}\"\nrole = \"Compute\"\ncapabilities = {}\nruntime = \"RustNative\"\n",
                format_toml_array(&gpu_caps)
            ));
        }
    }

    for worker in pi_workers {
        let Some(worker_ip) = extract_ip(&worker.addr) else {
            continue;
        };
        let mut caps = vec![
            "arm64".to_string(),
            "aarch64-linux-musl".to_string(),
            "nullclaw-mcp".to_string(),
            "sbc-pro".to_string(),
            "arm-master".to_string(),
            format!("node:{}", worker.node_id),
        ];
        let model_lower = worker.model.to_ascii_lowercase();
        if model_lower.contains("pi 3") {
            caps.push("pi3b".to_string());
        } else if model_lower.contains("pi 4") {
            caps.push("pi4b".to_string());
        } else {
            caps.push("rpi-sim".to_string());
        }
        out.push_str(&format!(
            "\n[[nodes]]\nip = \"{worker_ip}\"\nrole = \"Worker\"\ncapabilities = {}\nruntime = \"RustNative\"\n",
            format_toml_array(&caps)
        ));
    }

    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutedNode {
    pub ip: String,
    pub role: String,
    pub capabilities: Vec<String>,
}

fn parse_toml_string(line: &str) -> Option<String> {
    let (_, rhs) = line.split_once('=')?;
    let trimmed = rhs.trim();
    if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
        Some(trimmed[1..trimmed.len() - 1].to_string())
    } else {
        None
    }
}

fn parse_toml_array(line: &str) -> Vec<String> {
    let Some((_, rhs)) = line.split_once('=') else {
        return Vec::new();
    };
    let inner = rhs.trim().trim_start_matches('[').trim_end_matches(']');
    inner
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn route_capability_from_toml(toml_text: &str, capability: &str) -> Option<RoutedNode> {
    let mut nodes: Vec<RoutedNode> = Vec::new();
    let mut cur_ip: Option<String> = None;
    let mut cur_role: Option<String> = None;
    let mut cur_caps: Vec<String> = Vec::new();
    let mut in_node = false;

    for raw in toml_text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "[[nodes]]" {
            if in_node {
                if let (Some(ip), Some(role)) = (cur_ip.take(), cur_role.take()) {
                    nodes.push(RoutedNode {
                        ip,
                        role,
                        capabilities: std::mem::take(&mut cur_caps),
                    });
                }
            }
            in_node = true;
            cur_ip = None;
            cur_role = None;
            cur_caps.clear();
            continue;
        }
        if !in_node {
            continue;
        }
        if line.starts_with("ip") {
            cur_ip = parse_toml_string(line);
        } else if line.starts_with("role") {
            cur_role = parse_toml_string(line);
        } else if line.starts_with("capabilities") {
            cur_caps = parse_toml_array(line);
        }
    }
    if in_node {
        if let (Some(ip), Some(role)) = (cur_ip, cur_role) {
            nodes.push(RoutedNode {
                ip,
                role,
                capabilities: cur_caps,
            });
        }
    }

    nodes
        .into_iter()
        .filter(|n| n.capabilities.iter().any(|c| c == capability))
        .min_by_key(|n| if n.role == "Compute" { 0u8 } else { 1u8 })
}

pub fn write_atomic(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)
        .with_context(|| format!("writing temp file {}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("committing file {}", path.display()))?;
    Ok(())
}

pub fn build_manifest(
    coordinator_ip: &str,
    topology_path: &Path,
    services: Vec<ServiceProbe>,
    binaries: Vec<BinaryProbe>,
    pi_workers: Vec<NodeStatus>,
    repos: Vec<RepoStatus>,
) -> ConstellationManifest {
    let seal_input = format!(
        "{coordinator_ip}:{}:{}:{}",
        services.iter().filter(|s| s.up).count(),
        pi_workers.len(),
        repos.iter().filter(|r| r.present_locally).count()
    );
    let seal = format!("0x62:{:016x}", gate_0x62_seal(seal_input.as_bytes()));
    ConstellationManifest {
        spec: "8b-is-constellation/0.3.0".to_string(),
        seal,
        coordinator_ip: coordinator_ip.to_string(),
        ultrameshai_topology_path: topology_path.display().to_string(),
        services,
        binaries,
        pi_workers,
        repos,
    }
}

pub async fn sync_topology_and_manifest(
    coordinator_ip: &str,
    topology_out: &Path,
    manifest_out: &Path,
    pi_workers: &[NodeStatus],
    workspace_root: &Path,
    a6000_ip: Option<&str>,
) -> Result<(String, ConstellationManifest)> {
    let services = probe_services().await;
    let binaries = probe_binaries();
    let repos = probe_repos(workspace_root);

    let toml_text =
        build_ultramesh_topology_toml(coordinator_ip, &services, &binaries, pi_workers, a6000_ip);
    write_atomic(topology_out, &toml_text)?;

    let manifest = build_manifest(
        coordinator_ip,
        topology_out,
        services,
        binaries,
        pi_workers.to_vec(),
        repos,
    );
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    write_atomic(manifest_out, &format!("{manifest_json}\n"))?;

    Ok((toml_text, manifest))
}

// Pure-Rust SHA-256 + HMAC-SHA256 for signed heartbeats to 8b-is/ultrameshai node-registry
const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

pub fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut padded = data.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (dst, bytes4) in w.iter_mut().take(16).zip(chunk.as_chunks::<4>().0) {
            *dst = u32::from_be_bytes(*bytes4);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        let next = [a, b, c, d, e, f, g, h];
        for (dst, add) in state.iter_mut().zip(next) {
            *dst = dst.wrapping_add(add);
        }
    }

    let mut out = [0u8; 32];
    for (i, word) in state.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

pub fn hmac_sha256_hex(secret: &[u8], payload: &[u8]) -> String {
    let mut key_block = [0u8; 64];
    if secret.len() > 64 {
        let digest = sha256_bytes(secret);
        key_block[..32].copy_from_slice(&digest);
    } else {
        key_block[..secret.len()].copy_from_slice(secret);
    }

    let mut i_pad = [0x36u8; 64];
    let mut o_pad = [0x5cu8; 64];
    for i in 0..64 {
        i_pad[i] ^= key_block[i];
        o_pad[i] ^= key_block[i];
    }

    let mut inner = Vec::with_capacity(64 + payload.len());
    inner.extend_from_slice(&i_pad);
    inner.extend_from_slice(payload);
    let inner_hash = sha256_bytes(&inner);

    let mut outer = Vec::with_capacity(64 + 32);
    outer.extend_from_slice(&o_pad);
    outer.extend_from_slice(&inner_hash);
    let final_hash = sha256_bytes(&outer);

    let mut hex = String::with_capacity(64);
    for b in final_hash {
        hex.push_str(&format!("{b:02x}"));
    }
    hex
}

pub async fn post_signed_heartbeat(
    host_port: &str,
    secret: &str,
    node_id: &str,
    capabilities: &[String],
    memory_mb: u64,
) -> Result<u16> {
    let body = serde_json::json!({
        "node_id": node_id,
        "capabilities": capabilities,
        "memory_mb": memory_mb,
        "load_avg": null,
        "region": "local-dongle-mesh",
    })
    .to_string();
    let sig = hmac_sha256_hex(secret.as_bytes(), body.as_bytes());
    let req = format!(
        "POST /heartbeat HTTP/1.1\r\nHost: {host_port}\r\nContent-Type: application/json\r\nx-signature: hmac-sha256={sig}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );

    let mut stream = tokio::time::timeout(Duration::from_millis(1500), TcpStream::connect(host_port))
        .await
        .with_context(|| format!("timeout connecting to ultrameshai node-registry {host_port}"))??;
    stream.write_all(req.as_bytes()).await?;
    stream.flush().await?;

    let mut resp = [0u8; 256];
    let n = tokio::time::timeout(Duration::from_millis(1500), stream.read(&mut resp)).await??;
    let first_line = String::from_utf8_lossy(&resp[..n])
        .lines()
        .next()
        .unwrap_or("")
        .to_string();
    let code = first_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    if code == 0 {
        bail!("unexpected HTTP response from {host_port}: {first_line}");
    }
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_sha256_matches_rfc4231_test_vector() {
        let hex = hmac_sha256_hex(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(
            hex,
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn builds_valid_ultramesh_topology_toml_and_routes_capabilities() {
        let services = vec![ServiceProbe {
            id: "honcho".to_string(),
            host: "127.0.0.1".to_string(),
            port: 8000,
            up: true,
            capability: "honcho-memory".to_string(),
        }];
        let binaries = vec![BinaryProbe {
            name: "st".to_string(),
            path: Some("/Users/peterlodri/.local/bin/st".to_string()),
            present: true,
        }];
        let workers = vec![NodeStatus {
            addr: "10.88.0.10:18815".to_string(),
            node_id: "rpi-0001".to_string(),
            model: "Raspberry Pi 3 Model B".to_string(),
            arch: "aarch64".to_string(),
            mem_available_kib: 819_200,
            skills_loaded: 2,
            seal: "0x62:1122334455667762".to_string(),
        }];

        let toml_text = build_ultramesh_topology_toml(
            "10.88.0.1",
            &services,
            &binaries,
            &workers,
            Some("49.13.101.247"),
        );

        assert!(toml_text.contains("coordinator_ip = \"10.88.0.1\""));
        assert!(toml_text.contains("ip = \"10.88.0.10\""));
        assert!(toml_text.contains("\"pi3b\""));
        assert!(toml_text.contains("\"a6000-orchestrator\""));

        let arm_target = route_capability_from_toml(&toml_text, "arm64").unwrap();
        assert_eq!(arm_target.ip, "10.88.0.10");
        assert_eq!(arm_target.role, "Worker");

        let gpu_target = route_capability_from_toml(&toml_text, "cuda").unwrap();
        assert_eq!(gpu_target.ip, "49.13.101.247");
        assert_eq!(gpu_target.role, "Compute");

        let honcho_target = route_capability_from_toml(&toml_text, "honcho-memory").unwrap();
        assert_eq!(honcho_target.ip, "10.88.0.1");
        assert_eq!(honcho_target.role, "Compute");
    }

    #[test]
    fn constellation_manifest_has_0x62_seal_and_all_repos() {
        let repos = probe_repos(Path::new("/nonexistent"));
        assert!(repos.len() >= 18);
        let manifest = build_manifest(
            "10.88.0.1",
            Path::new("/tmp/topology.toml"),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            repos,
        );
        assert!(manifest.seal.starts_with("0x62:"));
    }
}
