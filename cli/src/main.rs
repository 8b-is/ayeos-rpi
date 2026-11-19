// ayeos-rpi / cli / src / main.rs — Sovereign MacBook USB-Dongle Mesh Launcher & Burner CLI
#![forbid(unsafe_code)]
#![deny(clippy::float_arithmetic)]

mod burn;
mod capsules;
mod constellation;
mod dhcp;
mod dongle;
mod mesh;
mod tftp;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::sync::{mpsc, watch};

#[derive(Parser, Debug)]
#[command(
    name = "ayeos",
    version,
    about = "ayeos-rpi sovereign USB-dongle PXE/TFTP launcher, SD burner & mesh driver"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Inspect or configure the MacBook USB Ethernet dongle connected to the Pi switch
    Dongle {
        #[command(subcommand)]
        cmd: DongleCmd,
    },
    /// Safe SD card, Pi 3B bootcode.bin FAT32 volume, and TFTP directory staging
    Burn {
        #[command(subcommand)]
        cmd: BurnCmd,
    },
    /// Start pure-Rust BOOTP/DHCP (Option 43) + TFTP + nullclaw :18815 mesh watcher
    Launch {
        /// USB Ethernet dongle interface (e.g. en4, en5, en7)
        #[arg(long)]
        iface: Option<String>,
        /// MacBook coordinator IPv4 on the switch subnet
        #[arg(long, default_value = "10.88.0.1")]
        server_ip: String,
        /// Subnet mask for the dongle switch LAN
        #[arg(long, default_value = "255.255.255.0")]
        netmask: String,
        /// DHCP UDP bind address
        #[arg(long, default_value = "0.0.0.0:67")]
        dhcp_bind: String,
        /// TFTP UDP bind address
        #[arg(long, default_value = "0.0.0.0:69")]
        tftp_bind: String,
        /// TFTP root directory (auto-staged if missing and netboot sources exist)
        #[arg(long, default_value = "dist/tftp")]
        tftp_dir: PathBuf,
        /// Repository root containing netboot/ and dist/
        #[arg(long, default_value = ".")]
        repo_root: PathBuf,
        /// Worker nullclaw TCP MCP port
        #[arg(long, default_value_t = 18815)]
        worker_port: u16,
        /// Hermes discovered worker node registry JSON path
        #[arg(long, default_value = "~/.hermes/rpi_mesh_nodes.json")]
        registry: PathBuf,
        /// Skip ifconfig IP setup (use when interface is already configured or testing)
        #[arg(long)]
        no_ifconfig: bool,
        /// Allow non-dongle interfaces (bypasses en0/lo0/Thunderbolt guard)
        #[arg(long)]
        allow_any_iface: bool,
    },
    /// Ping one or more booted Raspberry Pi nullclaw workers (:18815) and update Hermes registry
    Ping {
        /// Comma-separated host:port worker addresses (falls back to --registry entries)
        #[arg(long, value_delimiter = ',')]
        addrs: Vec<String>,
        /// Hermes worker node registry JSON path
        #[arg(long, default_value = "~/.hermes/rpi_mesh_nodes.json")]
        registry: PathBuf,
    },
    /// Wire and inspect 8b-is/ultrameshai topology.toml + live 8b-is constellation
    Mesh {
        #[command(subcommand)]
        cmd: MeshCmd,
    },
    /// Verify published g8t.is pure-Rust scalar capsules locally
    Capsules,
    /// Push main + tags to GitHub origin, mirror to Codeberg, and push to g8t.is with transient G8T_TOKEN auth
    ForgePush {
        /// Path to git repository
        #[arg(long, default_value = ".")]
        repo_dir: PathBuf,
        /// Forge owner/org name on Codeberg and g8t.is
        #[arg(long, default_value = "8b-is")]
        owner: String,
        /// Forge repository name
        #[arg(long, default_value = "ayeos-rpi")]
        repo: String,
    },
}

#[derive(Subcommand, Debug)]
enum DongleCmd {
    /// List hardware ports and highlight USB Ethernet dongle candidates
    List,
    /// Configure an isolated IPv4 subnet on a USB Ethernet dongle
    Up {
        #[arg(long)]
        iface: String,
        #[arg(long, default_value = "10.88.0.1")]
        ip: String,
        #[arg(long, default_value = "255.255.255.0")]
        netmask: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        allow_any_iface: bool,
    },
}

#[derive(Subcommand, Debug)]
enum BurnCmd {
    /// List removable physical disks (never lists internal system drives for burning)
    ListDisks,
    /// Stage config.txt, cmdline.txt, kernel8.img, initramfs.cpio.zst into a TFTP directory
    StageTftp {
        #[arg(long, default_value = "dist/tftp")]
        dir: PathBuf,
        #[arg(long, default_value = ".")]
        repo_root: PathBuf,
    },
    /// Prepare a FAT32 SD volume with Pi 3B bootcode.bin + timeout (no OTP burn)
    Pi3Sd {
        #[arg(long)]
        dir: PathBuf,
        #[arg(long)]
        bootcode: Option<PathBuf>,
        #[arg(long)]
        force_dir: bool,
    },
    /// Stream a raw .img directly to an external SD block device (refuses disk0/disk1)
    Raw {
        #[arg(long)]
        image: PathBuf,
        #[arg(long)]
        device: PathBuf,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand, Debug)]
enum MeshCmd {
    /// Probe live 8b-is cluster ports, binaries, repos, and booted Pi workers
    Status {
        #[arg(long, default_value = "/Users/peterlodri/workspace/peterlodri-sec")]
        workspace: PathBuf,
        #[arg(long, default_value = "~/.hermes/rpi_mesh_nodes.json")]
        registry: PathBuf,
    },
    /// Emit ultrameshai hot-reload topology.toml and 8b-is constellation manifest
    SyncTopology {
        #[arg(long, default_value = "10.88.0.1")]
        server_ip: String,
        #[arg(long, default_value = "~/.hermes/ultramesh_topology.toml")]
        out: PathBuf,
        #[arg(long, default_value = "~/.hermes/8b_is_constellation.json")]
        manifest: PathBuf,
        #[arg(long, default_value = "~/.hermes/rpi_mesh_nodes.json")]
        registry: PathBuf,
        #[arg(long, default_value = "/Users/peterlodri/workspace/peterlodri-sec")]
        workspace: PathBuf,
        #[arg(long)]
        a6000_ip: Option<String>,
        /// Optional host:port for signed heartbeat POST to 8b-is/ultrameshai node-registry
        #[arg(long)]
        node_registry: Option<String>,
    },
    /// Route a capability across ultrameshai topology.toml (Compute-first, Worker-fallback)
    Route {
        capability: String,
        #[arg(long, default_value = "~/.hermes/ultramesh_topology.toml")]
        topology: PathBuf,
    },
    /// Emit JSON manifest of all wired 8b-is constellation repositories
    Manifest {
        #[arg(long, default_value = "10.88.0.1")]
        server_ip: String,
        #[arg(long, default_value = "~/.hermes/ultramesh_topology.toml")]
        topology: PathBuf,
        #[arg(long, default_value = "~/.hermes/8b_is_constellation.json")]
        out: PathBuf,
        #[arg(long, default_value = "~/.hermes/rpi_mesh_nodes.json")]
        registry: PathBuf,
        #[arg(long, default_value = "/Users/peterlodri/workspace/peterlodri-sec")]
        workspace: PathBuf,
    },
}

fn expand_tilde(path: &Path) -> Result<PathBuf> {
    let s = path.to_string_lossy();
    let expanded = shellexpand::tilde(&s);
    Ok(PathBuf::from(expanded.as_ref()))
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Dongle { cmd } => match cmd {
            DongleCmd::List => {
                let ports = dongle::discover_dongles()?;
                println!(
                    "{:<8} {:<10} {:<19} {:<16} {:<8} PORT",
                    "DONGLE", "DEV", "MAC", "IPv4", "LINK"
                );
                for p in ports {
                    println!(
                        "{:<8} {:<10} {:<19} {:<16} {:<8} {}",
                        if p.is_dongle_candidate { "YES" } else { "-" },
                        p.device,
                        p.mac.as_deref().unwrap_or("-"),
                        p.ipv4.as_deref().unwrap_or("-"),
                        if p.link_active { "up" } else { "down" },
                        p.port_name
                    );
                }
            }
            DongleCmd::Up {
                iface,
                ip,
                netmask,
                dry_run,
                allow_any_iface,
            } => {
                let cmd_str =
                    dongle::configure_dongle_ip(&iface, &ip, &netmask, dry_run, allow_any_iface)?;
                println!(
                    "{} {cmd_str}",
                    if dry_run { "[dry-run]" } else { "[ok]" }
                );
            }
        },
        Commands::Burn { cmd } => match cmd {
            BurnCmd::ListDisks => {
                let out = burn::list_removable_disks()?;
                print!("{out}");
            }
            BurnCmd::StageTftp { dir, repo_root } => {
                burn::stage_tftp(&dir, &repo_root)?;
                println!("[burn] staged TFTP root -> {}", dir.display());
            }
            BurnCmd::Pi3Sd {
                dir,
                bootcode,
                force_dir,
            } => {
                burn::burn_pi3_bootcode_volume(&dir, bootcode.as_deref(), force_dir)?;
                println!("[burn] Pi 3B netboot volume ready -> {}", dir.display());
            }
            BurnCmd::Raw { image, device, yes } => {
                let bytes = burn::burn_raw_image(&image, &device, yes)?;
                println!("[burn] wrote {bytes} bytes to {}", device.display());
            }
        },
        Commands::Launch {
            iface,
            server_ip,
            netmask,
            dhcp_bind,
            tftp_bind,
            tftp_dir,
            repo_root,
            worker_port,
            registry,
            no_ifconfig,
            allow_any_iface,
        } => {
            run_launch(
                iface,
                server_ip,
                netmask,
                dhcp_bind,
                tftp_bind,
                tftp_dir,
                repo_root,
                worker_port,
                expand_tilde(&registry)?,
                no_ifconfig,
                allow_any_iface,
            )
            .await?;
        }
        Commands::Ping { addrs, registry } => {
            run_ping(addrs, expand_tilde(&registry)?).await?;
        }
        Commands::Mesh { cmd } => {
            run_mesh_cmd(cmd).await?;
        }
        Commands::Capsules => {
            capsules::run_capsule_self_check()?;
            println!(
                "{:<30} {:<34} {:<18} {:<8} FKN",
                "CAPSULE", "G8T_ID", "SIG", "CYC/OP"
            );
            for c in capsules::PUBLISHED_CAPSULES {
                println!(
                    "{:<30} {:<34} {:<18} {:<8} {}",
                    c.name, c.id, c.signature, c.cycles_per_op_pq, c.foken_fkn
                );
            }
        }
        Commands::ForgePush {
            repo_dir,
            owner,
            repo,
        } => {
            run_forge_push(&repo_dir, &owner, &repo)?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn run_launch(
    iface: Option<String>,
    server_ip: String,
    netmask: String,
    dhcp_bind: String,
    tftp_bind: String,
    tftp_dir: PathBuf,
    repo_root: PathBuf,
    worker_port: u16,
    registry_path: PathBuf,
    no_ifconfig: bool,
    allow_any_iface: bool,
) -> Result<()> {
    let parsed_server_ip: Ipv4Addr = server_ip
        .parse()
        .with_context(|| format!("invalid --server-ip {server_ip}"))?;
    let parsed_mask: Ipv4Addr = netmask
        .parse()
        .with_context(|| format!("invalid --netmask {netmask}"))?;

    if !no_ifconfig {
        let chosen = match iface {
            Some(i) => i,
            None => {
                let ports = dongle::discover_dongles()?;
                let candidates: Vec<_> = ports
                    .into_iter()
                    .filter(|p| p.is_dongle_candidate)
                    .collect();
                let active: Vec<_> = candidates.iter().filter(|p| p.link_active).collect();
                if active.len() == 1 {
                    active[0].device.clone()
                } else if candidates.len() == 1 {
                    candidates[0].device.clone()
                } else {
                    let list: Vec<String> = candidates
                        .iter()
                        .map(|p| {
                            format!(
                                "{} ({})",
                                p.device,
                                if p.link_active { "up" } else { "down" }
                            )
                        })
                        .collect();
                    bail!(
                        "multiple or zero USB Ethernet dongles detected [{}]. Pass --iface <dev> or --no-ifconfig.",
                        list.join(", ")
                    );
                }
            }
        };
        let cmd = dongle::configure_dongle_ip(
            &chosen,
            &server_ip,
            &netmask,
            false,
            allow_any_iface,
        )?;
        eprintln!("[dongle] configured: {cmd}");
    }

    if !tftp_dir.join("config.txt").is_file() && repo_root.join("netboot/config.netboot.txt").is_file()
    {
        burn::stage_tftp(&tftp_dir, &repo_root)?;
        eprintln!("[tftp] auto-staged netboot artifacts -> {}", tftp_dir.display());
    }
    if !tftp_dir.is_dir() {
        bail!(
            "TFTP directory missing: {} (run `ayeos burn stage-tftp` first)",
            tftp_dir.display()
        );
    }

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (lease_tx, mut lease_rx) = mpsc::unbounded_channel();

    let dhcp_cfg = dhcp::DhcpConfig {
        bind_addr: dhcp_bind,
        server_ip: parsed_server_ip,
        subnet_mask: parsed_mask,
        ..dhcp::DhcpConfig::default()
    };

    let (dhcp_addr, dhcp_handle) =
        dhcp::run_dhcp_server(dhcp_cfg, Some(lease_tx), shutdown_rx.clone()).await?;
    let (tftp_addr, tftp_handle) =
        tftp::run_tftp_server(&tftp_bind, tftp_dir.clone(), shutdown_rx).await?;

    eprintln!(
        "[launch] PXE/DHCP listening on {dhcp_addr} | TFTP listening on {tftp_addr} | root={}",
        tftp_dir.display()
    );
    eprintln!(
        "[launch] registry={} | press Ctrl-C to stop",
        registry_path.display()
    );

    let topology_path = expand_tilde(Path::new("~/.hermes/ultramesh_topology.toml"))?;
    let manifest_path = expand_tilde(Path::new("~/.hermes/8b_is_constellation.json"))?;
    let workspace_root = PathBuf::from("/Users/peterlodri/workspace/peterlodri-sec");
    let existing_nodes = mesh::read_hermes_registry(&registry_path);
    if let Err(err) = constellation::sync_topology_and_manifest(
        &server_ip,
        &topology_path,
        &manifest_path,
        &existing_nodes,
        &workspace_root,
        None,
    )
    .await
    {
        eprintln!("[ultrameshai] initial topology sync skipped: {err}");
    }

    let watcher = tokio::spawn(async move {
        while let Some(ev) = lease_rx.recv().await {
            if ev.msg_type == dhcp::DHCP_ACK {
                eprintln!(
                    "[mesh] lease ACK ip={} mac={} pi_oui={} -> probing :{}",
                    ev.ip,
                    dhcp::format_mac(&ev.mac),
                    ev.is_pi_oui,
                    worker_port
                );
                let reg = registry_path.clone();
                let topo = topology_path.clone();
                let man = manifest_path.clone();
                let coord_ip = server_ip.clone();
                let ws = workspace_root.clone();
                tokio::spawn(async move {
                    mesh::probe_on_lease(ev.ip, worker_port, reg, topo, man, coord_ip, ws).await;
                });
            }
        }
    });

    tokio::signal::ctrl_c().await?;
    eprintln!("\n[launch] shutting down...");
    let _ = shutdown_tx.send(true);
    let _ = dhcp_handle.await;
    let _ = tftp_handle.await;
    watcher.abort();
    Ok(())
}

async fn run_ping(addrs: Vec<String>, registry_path: PathBuf) -> Result<()> {
    let targets = if addrs.is_empty() {
        let existing = mesh::read_hermes_registry(&registry_path);
        if existing.is_empty() {
            bail!(
                "no --addrs provided and no nodes recorded in {}",
                registry_path.display()
            );
        }
        existing.into_iter().map(|n| n.addr).collect()
    } else {
        addrs
    };

    println!(
        "{:<20} {:<22} {:<16} {:<10} {:<7} SEAL",
        "NODE_ID", "ADDR", "MODEL", "RAM_KIB", "SKILLS"
    );
    let mut ok_count = 0usize;
    for addr in &targets {
        match mesh::ping_worker(addr).await {
            Ok(st) => {
                println!(
                    "{:<20} {:<22} {:<16} {:<10} {:<7} {}",
                    st.node_id,
                    st.addr,
                    st.model,
                    st.mem_available_kib,
                    st.skills_loaded,
                    st.seal
                );
                mesh::upsert_registry_node(st, &registry_path)?;
                ok_count += 1;
            }
            Err(err) => {
                println!("{:<20} {:<22} ERROR: {err}", "-", addr);
            }
        }
    }

    if ok_count == 0 {
        bail!("all {} worker pings failed", targets.len());
    }
    Ok(())
}

async fn run_mesh_cmd(cmd: MeshCmd) -> Result<()> {
    match cmd {
        MeshCmd::Status {
            workspace,
            registry,
        } => {
            let services = constellation::probe_services().await;
            let binaries = constellation::probe_binaries();
            let repos = constellation::probe_repos(&workspace);
            let workers = mesh::read_hermes_registry(&expand_tilde(&registry)?);

            println!("=== LIVE 8b-is SERVICES ===");
            for s in &services {
                println!(
                    "{:<22} {:<16}:{:<6} {}",
                    s.id,
                    s.host,
                    s.port,
                    if s.up { "UP" } else { "down" }
                );
            }
            println!("\n=== INSTALLED 8b-is BINARIES ===");
            for b in &binaries {
                println!(
                    "{:<16} {}",
                    b.name,
                    b.path.as_deref().unwrap_or("missing")
                );
            }
            println!("\n=== 8b-is REPOSITORIES ({}) ===", repos.len());
            for r in &repos {
                println!(
                    "{:<22} {:<7} {}",
                    r.name,
                    if r.present_locally { "LOCAL" } else { "remote" },
                    r.local_path.as_deref().unwrap_or(&r.github_url)
                );
            }
            println!("\n=== BOOTED PI WORKERS ({}) ===", workers.len());
            for w in &workers {
                println!(
                    "{:<18} {:<20} {:<16} seal={}",
                    w.node_id, w.addr, w.model, w.seal
                );
            }
        }
        MeshCmd::SyncTopology {
            server_ip,
            out,
            manifest,
            registry,
            workspace,
            a6000_ip,
            node_registry,
        } => {
            let out_p = expand_tilde(&out)?;
            let man_p = expand_tilde(&manifest)?;
            let reg_p = expand_tilde(&registry)?;
            let workers = mesh::read_hermes_registry(&reg_p);
            let (toml_text, man) = constellation::sync_topology_and_manifest(
                &server_ip,
                &out_p,
                &man_p,
                &workers,
                &workspace,
                a6000_ip.as_deref(),
            )
            .await?;

            if let Some(nr_addr) = node_registry {
                if let Ok(secret) = std::env::var("HEARTBEAT_SECRET") {
                    if !secret.is_empty() {
                        let caps: Vec<String> = man
                            .services
                            .iter()
                            .filter(|s| s.up)
                            .map(|s| s.capability.clone())
                            .collect();
                        let code = constellation::post_signed_heartbeat(
                            &nr_addr,
                            &secret,
                            &server_ip,
                            &caps,
                            16384,
                        )
                        .await?;
                        println!("[ultrameshai] signed POST /heartbeat -> HTTP {code}");
                    }
                }
            }

            println!("[ultrameshai] topology -> {}", out_p.display());
            println!(
                "[constellation] manifest -> {} (seal={})",
                man_p.display(),
                man.seal
            );
            print!("{toml_text}");
        }
        MeshCmd::Route {
            capability,
            topology,
        } => {
            let topo_p = expand_tilde(&topology)?;
            let text = std::fs::read_to_string(&topo_p)
                .with_context(|| format!("reading topology {}", topo_p.display()))?;
            match constellation::route_capability_from_toml(&text, &capability) {
                Some(node) => println!(
                    "ROUTE {capability} -> {} ({}) caps={}",
                    node.ip,
                    node.role,
                    node.capabilities.join(",")
                ),
                None => bail!("no node advertises capability `{capability}` in {}", topo_p.display()),
            }
        }
        MeshCmd::Manifest {
            server_ip,
            topology,
            out,
            registry,
            workspace,
        } => {
            let out_p = expand_tilde(&out)?;
            let topo_p = expand_tilde(&topology)?;
            let reg_p = expand_tilde(&registry)?;
            let workers = mesh::read_hermes_registry(&reg_p);
            let (_, man) = constellation::sync_topology_and_manifest(
                &server_ip,
                &topo_p,
                &out_p,
                &workers,
                &workspace,
                None,
            )
            .await?;
            println!("{}", serde_json::to_string_pretty(&man)?);
        }
    }
    Ok(())
}

fn run_forge_push(repo_dir: &Path, owner: &str, repo: &str) -> Result<()> {
    let repo_dir_str = repo_dir.to_string_lossy();
    let check = Command::new("git")
        .args(["-C", &repo_dir_str, "rev-parse", "--is-inside-work-tree"])
        .output()
        .context("running git rev-parse")?;
    if !check.status.success() {
        bail!("{} is not a git repository", repo_dir.display());
    }

    let push_origin = Command::new("git")
        .args(["-C", &repo_dir_str, "push", "origin", "main", "--tags"])
        .status()
        .context("pushing main and tags to GitHub origin")?;
    if !push_origin.success() {
        bail!("git push origin main --tags failed");
    }
    println!("[forge] GitHub origin synced (main + tags)");

    let codeberg_git_url = format!("git@codeberg.org:{owner}/{repo}.git");
    let cb_remote_check = Command::new("git")
        .args(["-C", &repo_dir_str, "remote", "get-url", "codeberg"])
        .output()?;
    if !cb_remote_check.status.success() {
        let _ = Command::new("git")
            .args([
                "-C",
                &repo_dir_str,
                "remote",
                "add",
                "codeberg",
                &codeberg_git_url,
            ])
            .status()?;
    }
    let push_codeberg = Command::new("git")
        .args(["-C", &repo_dir_str, "push", "codeberg", "main", "--tags"])
        .status()
        .context("pushing main and tags to Codeberg")?;
    if !push_codeberg.success() {
        bail!("git push codeberg main --tags failed (ensure {codeberg_git_url} exists)");
    }
    println!("[forge] Codeberg synced ({codeberg_git_url}, main + tags)");

    let g8t_git_url = format!("https://g8t.is/git/{owner}/{repo}");
    let remote_check = Command::new("git")
        .args(["-C", &repo_dir_str, "remote", "get-url", "g8t"])
        .output()?;
    if !remote_check.status.success() {
        let _ = Command::new("git")
            .args(["-C", &repo_dir_str, "remote", "add", "g8t", &g8t_git_url])
            .status()?;
    }

    let token = std::env::var("G8T_TOKEN").unwrap_or_default();
    if token.trim().is_empty() {
        let probe = Command::new("curl")
            .args([
                "-s",
                "-o",
                "/dev/null",
                "-w",
                "%{http_code}",
                &format!("https://g8t.is/repos/{owner}/{repo}"),
            ])
            .output();
        let code = probe
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_else(|_| "unreachable".to_string());
        println!(
            "[forge] g8t.is auth required (HTTP {code}; G8T_TOKEN is unset; remote g8t -> {g8t_git_url})"
        );
        println!(
            "[forge] next: G8T_TOKEN=<token> ayeos forge-push --repo-dir {}",
            repo_dir.display()
        );
        return Ok(());
    }

    let auth_header = format!("Authorization: Bearer {token}");
    let create_out = Command::new("curl")
        .args([
            "-sS",
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
            "-X",
            "POST",
            "-H",
            &auth_header,
            &format!("https://g8t.is/repos/{owner}/{repo}"),
        ])
        .output()
        .context("creating or verifying repository on g8t.is")?;
    let http_code = String::from_utf8_lossy(&create_out.stdout)
        .trim()
        .to_string();
    if !matches!(http_code.as_str(), "200" | "201" | "409") {
        bail!("POST https://g8t.is/repos/{owner}/{repo} returned HTTP {http_code}");
    }

    let push_g8t = Command::new("git")
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "http.extraHeader")
        .env("GIT_CONFIG_VALUE_0", &auth_header)
        .args(["-C", &repo_dir_str, "push", "g8t", "main", "--tags"])
        .status()
        .context("pushing main and tags to g8t.is")?;
    if !push_g8t.success() {
        bail!("git push g8t main --tags failed");
    }
    println!("[forge] g8t.is synced ({g8t_git_url}, main + tags)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use tokio::net::UdpSocket;

    fn build_discover_pkt(xid: [u8; 4], mac: [u8; 6]) -> Vec<u8> {
        let mut pkt = vec![0u8; 240];
        pkt[0] = 1; // BOOTREQUEST
        pkt[1] = 1;
        pkt[2] = 6;
        pkt[4..8].copy_from_slice(&xid);
        pkt[28..34].copy_from_slice(&mac);
        pkt[236..240].copy_from_slice(&[99, 130, 83, 99]);
        pkt.extend_from_slice(&[53, 1, dhcp::DHCP_DISCOVER]);
        let pxe = b"PXEClient:Arch:00000:UNDI:002001";
        pkt.push(60);
        pkt.push(pxe.len() as u8);
        pkt.extend_from_slice(pxe);
        pkt.push(255);
        pkt
    }

    #[tokio::test]
    async fn dhcp_and_tftp_full_pxe_boot_simulation() {
        let base = std::env::temp_dir().join(format!("ayeos_pxe_sim_{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let expected_cfg = b"arm_64bit=1\ninitramfs initramfs.cpio.zst followkernel\n";
        std::fs::write(base.join("config.txt"), expected_cfg).unwrap();
        std::fs::write(base.join("initramfs.cpio.zst"), b"ZST-MOCK").unwrap();

        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let dhcp_cfg = dhcp::DhcpConfig {
            bind_addr: "127.0.0.1:0".to_string(),
            server_ip: Ipv4Addr::new(10, 88, 0, 1),
            ..dhcp::DhcpConfig::default()
        };
        let (dhcp_addr, dhcp_handle) =
            dhcp::run_dhcp_server(dhcp_cfg, None, shutdown_rx.clone())
                .await
                .unwrap();
        let (tftp_addr, tftp_handle) =
            tftp::run_tftp_server("127.0.0.1:0", base.clone(), shutdown_rx)
                .await
                .unwrap();

        // 1. Send Raspberry Pi (dc:a6:32:aa:bb:cc) DHCPDISCOVER and verify Option 43 Offer
        let dhcp_client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let discover = build_discover_pkt([0x12, 0x34, 0x56, 0x78], [0xdc, 0xa6, 0x32, 0xaa, 0xbb, 0xcc]);
        dhcp_client.send_to(&discover, dhcp_addr).await.unwrap();

        let mut offer_buf = [0u8; 1500];
        let (n, _) = dhcp_client.recv_from(&mut offer_buf).await.unwrap();
        assert!(n >= 240);
        assert_eq!(offer_buf[0], 2); // BOOTREPLY
        assert_eq!(&offer_buf[16..20], &[10, 88, 0, 10]); // yiaddr = 10.88.0.10
        assert!(offer_buf[..n]
            .windows(b"Raspberry Pi Boot".len())
            .any(|w| w == b"Raspberry Pi Boot"));

        // 2. Send TFTP RRQ for "dca632aa/config.txt" with blksize=1024 + tsize=0
        let tftp_client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let mut rrq = Vec::new();
        rrq.extend_from_slice(&1u16.to_be_bytes());
        rrq.extend_from_slice(b"dca632aa/config.txt\x00octet\x00blksize\x001024\x00tsize\x000\x00");
        tftp_client.send_to(&rrq, tftp_addr).await.unwrap();

        let mut rx = [0u8; 1500];
        let (on, transfer_peer): (usize, SocketAddr) = tftp_client.recv_from(&mut rx).await.unwrap();
        assert_eq!(u16::from_be_bytes([rx[0], rx[1]]), 6); // OACK
        assert!(rx[2..on].windows(b"tsize".len()).any(|w| w == b"tsize"));

        // ACK block 0 to trigger DATA block 1
        tftp_client
            .send_to(&[0, 4, 0, 0], transfer_peer)
            .await
            .unwrap();

        let (dn, data_peer) = tftp_client.recv_from(&mut rx).await.unwrap();
        assert_eq!(data_peer, transfer_peer);
        assert_eq!(u16::from_be_bytes([rx[0], rx[1]]), 3); // DATA
        assert_eq!(u16::from_be_bytes([rx[2], rx[3]]), 1); // block 1
        assert_eq!(&rx[4..dn], expected_cfg);

        // ACK block 1 cleanly
        tftp_client
            .send_to(&[0, 4, 0, 1], transfer_peer)
            .await
            .unwrap();

        let _ = shutdown_tx.send(true);
        let _ = dhcp_handle.await;
        let _ = tftp_handle.await;
        let _ = std::fs::remove_dir_all(&base);
    }
}
