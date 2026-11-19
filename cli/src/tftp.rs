use anyhow::{Context, Result};
use std::net::{IpAddr, SocketAddr};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::watch;

const OP_RRQ: u16 = 1;
const OP_DATA: u16 = 3;
const OP_ACK: u16 = 4;
const OP_ERROR: u16 = 5;
const OP_OACK: u16 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
        if q == 0 || p == 0 {
            return Self { p: 0, q: 1 };
        }
        let g = Self::gcd_u64(p, q);
        Self { p: p / g, q: q / g }
    }
}

pub fn gate_0x62_seal(payload: &[u8]) -> u64 {
    let mut acc: u64 = 0x6262_6262_6262_6262;
    for (idx, &byte) in payload.iter().enumerate() {
        let lane = ((idx as u64) & 7).wrapping_mul(8);
        acc = acc.rotate_left(7) ^ ((byte as u64) << lane) ^ 0x62;
    }
    (acc & !0xFF) | 0x62
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRrq {
    pub filename: String,
    pub mode: String,
    pub blksize: Option<usize>,
    pub tsize: bool,
}

pub fn parse_rrq(pkt: &[u8]) -> Option<ParsedRrq> {
    if pkt.len() < 6 {
        return None;
    }
    let op = u16::from_be_bytes([pkt[0], pkt[1]]);
    if op != OP_RRQ {
        return None;
    }
    let fields: Vec<&[u8]> = pkt[2..].split(|&b| b == 0).collect();
    if fields.len() < 2 {
        return None;
    }
    let filename = String::from_utf8_lossy(fields[0]).to_string();
    let mode = String::from_utf8_lossy(fields[1]).to_ascii_lowercase();
    if filename.is_empty() {
        return None;
    }

    let mut blksize = None;
    let mut tsize = false;
    let mut i = 2usize;
    while i + 1 < fields.len() {
        let key = String::from_utf8_lossy(fields[i]).to_ascii_lowercase();
        let val = String::from_utf8_lossy(fields[i + 1]);
        if key == "blksize" {
            if let Ok(parsed) = val.parse::<usize>() {
                blksize = Some(parsed.clamp(8, 1468));
            }
        } else if key == "tsize" {
            tsize = true;
        }
        i += 2;
    }

    Some(ParsedRrq {
        filename,
        mode,
        blksize,
        tsize,
    })
}

pub fn resolve_tftp_path(root: &Path, req_path: &str) -> Option<PathBuf> {
    if req_path.contains('\0') || req_path.contains('\\') {
        return None;
    }
    let trimmed = req_path.trim_start_matches('/');
    if trimmed.is_empty() {
        return None;
    }

    let mut rel_parts: Vec<&str> = Vec::new();
    for comp in Path::new(trimmed).components() {
        match comp {
            Component::Normal(seg) => {
                let s = seg.to_str()?;
                if s.is_empty() {
                    return None;
                }
                rel_parts.push(s);
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    if rel_parts.is_empty() {
        return None;
    }

    let mut exact = root.to_path_buf();
    for part in &rel_parts {
        exact.push(part);
    }
    if exact.is_file() {
        return Some(exact);
    }

    // Raspberry Pi VideoCore requests `<8-hex-serial>/<filename>` first; fallback cleanly to `<root>/<filename>`
    if rel_parts.len() == 2 {
        let fallback = root.join(rel_parts[1]);
        if fallback.is_file() {
            return Some(fallback);
        }
    }
    None
}

async fn send_error(socket: &UdpSocket, peer: SocketAddr, code: u16, msg: &str) {
    let mut pkt = Vec::with_capacity(5 + msg.len());
    pkt.extend_from_slice(&OP_ERROR.to_be_bytes());
    pkt.extend_from_slice(&code.to_be_bytes());
    pkt.extend_from_slice(msg.as_bytes());
    pkt.push(0);
    let _ = socket.send_to(&pkt, peer).await;
}

async fn wait_for_ack(
    socket: &UdpSocket,
    expected_peer: SocketAddr,
    expected_block: u16,
    resend_pkt: &[u8],
) -> Result<bool> {
    let mut ack_buf = [0u8; 64];
    for attempt in 0..5 {
        if attempt > 0 {
            socket.send_to(resend_pkt, expected_peer).await?;
        }
        match tokio::time::timeout(Duration::from_millis(1000), socket.recv_from(&mut ack_buf)).await
        {
            Ok(Ok((n, from))) if from == expected_peer && n >= 4 => {
                let op = u16::from_be_bytes([ack_buf[0], ack_buf[1]]);
                let blk = u16::from_be_bytes([ack_buf[2], ack_buf[3]]);
                if op == OP_ACK && blk == expected_block {
                    return Ok(true);
                }
                if op == OP_ERROR {
                    return Ok(false);
                }
            }
            _ => continue,
        }
    }
    Ok(false)
}

async fn serve_rrq(
    local_ip: IpAddr,
    peer: SocketAddr,
    rrq: ParsedRrq,
    tftp_root: PathBuf,
) -> Result<()> {
    let transfer_sock = UdpSocket::bind(SocketAddr::new(local_ip, 0))
        .await
        .context("binding ephemeral TFTP transfer socket")?;

    let Some(file_path) = resolve_tftp_path(&tftp_root, &rrq.filename) else {
        send_error(&transfer_sock, peer, 1, "File not found").await;
        return Ok(());
    };

    let data = std::fs::read(&file_path)
        .with_context(|| format!("reading TFTP payload {}", file_path.display()))?;
    let blksize = rrq.blksize.unwrap_or(512);
    let mut wire_bytes: u64 = 0;

    if rrq.blksize.is_some() || rrq.tsize {
        let mut oack = Vec::with_capacity(64);
        oack.extend_from_slice(&OP_OACK.to_be_bytes());
        if let Some(bs) = rrq.blksize {
            oack.extend_from_slice(b"blksize\0");
            oack.extend_from_slice(bs.to_string().as_bytes());
            oack.push(0);
        }
        if rrq.tsize {
            oack.extend_from_slice(b"tsize\0");
            oack.extend_from_slice(data.len().to_string().as_bytes());
            oack.push(0);
        }
        transfer_sock.send_to(&oack, peer).await?;
        wire_bytes = wire_bytes.saturating_add(oack.len() as u64);
        if !wait_for_ack(&transfer_sock, peer, 0, &oack).await? {
            return Ok(());
        }
    }

    let mut offset = 0usize;
    let mut block: u16 = 1;
    loop {
        let end = (offset + blksize).min(data.len());
        let chunk = &data[offset..end];

        let mut pkt = Vec::with_capacity(4 + chunk.len());
        pkt.extend_from_slice(&OP_DATA.to_be_bytes());
        pkt.extend_from_slice(&block.to_be_bytes());
        pkt.extend_from_slice(chunk);

        transfer_sock.send_to(&pkt, peer).await?;
        wire_bytes = wire_bytes.saturating_add(pkt.len() as u64);

        if !wait_for_ack(&transfer_sock, peer, block, &pkt).await? {
            return Ok(());
        }

        if chunk.len() < blksize {
            break;
        }
        offset = end;
        block = block.wrapping_add(1);
    }

    let pq = RationalPq::reduced(data.len() as u64, wire_bytes);
    let seal = gate_0x62_seal(&data);
    eprintln!(
        "[tftp] sent {} ({} B, mode={}) -> {} blksize={} pq={}/{} seal=0x62:{seal:016x}",
        rrq.filename,
        data.len(),
        rrq.mode,
        peer,
        blksize,
        pq.p,
        pq.q
    );
    Ok(())
}

pub async fn run_tftp_server(
    bind_addr: &str,
    tftp_root: PathBuf,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(SocketAddr, tokio::task::JoinHandle<Result<()>>)> {
    let socket = UdpSocket::bind(bind_addr)
        .await
        .with_context(|| format!("binding TFTP UDP socket on {bind_addr}"))?;
    let local_addr = socket.local_addr()?;

    let handle = tokio::spawn(async move {
        let mut buf = [0u8; 1500];
        loop {
            let (len, peer) = tokio::select! {
                res = socket.recv_from(&mut buf) => res?,
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        break;
                    }
                    continue;
                }
            };

            let Some(rrq) = parse_rrq(&buf[..len]) else {
                continue;
            };
            let root = tftp_root.clone();
            let ip = local_addr.ip();
            tokio::spawn(async move {
                if let Err(err) = serve_rrq(ip, peer, rrq, root).await {
                    eprintln!("[tftp] transfer error ({peer}): {err}");
                }
            });
        }
        Ok(())
    });

    Ok((local_addr, handle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_serial_subdir_fallback_and_blocks_traversal() {
        let base = std::env::temp_dir().join(format!("ayeos_tftp_path_{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let cfg = base.join("config.txt");
        std::fs::write(&cfg, b"arm_64bit=1\n").unwrap();

        assert_eq!(resolve_tftp_path(&base, "config.txt").as_deref(), Some(cfg.as_path()));
        assert_eq!(
            resolve_tftp_path(&base, "dca632aa/config.txt").as_deref(),
            Some(cfg.as_path())
        );
        assert!(resolve_tftp_path(&base, "../etc/passwd").is_none());
        assert!(resolve_tftp_path(&base, "dca632aa/../config.txt").is_none());

        let _ = std::fs::remove_dir_all(&base);
    }
}
