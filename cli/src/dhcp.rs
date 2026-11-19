use anyhow::{Context, Result};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, watch};

pub const RPI_MAC_OUIS: [[u8; 3]; 6] = [
    [0xb8, 0x27, 0xeb],
    [0xdc, 0xa6, 0x32],
    [0xe4, 0x5f, 0x01],
    [0x28, 0xcd, 0xc1],
    [0xd8, 0x3a, 0xdd],
    [0x2c, 0xcf, 0x67],
];

const DHCP_MAGIC_COOKIE: [u8; 4] = [99, 130, 83, 99];
pub const DHCP_DISCOVER: u8 = 1;
pub const DHCP_OFFER: u8 = 2;
pub const DHCP_REQUEST: u8 = 3;
pub const DHCP_ACK: u8 = 5;

#[derive(Debug, Clone)]
pub struct DhcpConfig {
    pub bind_addr: String,
    pub server_ip: Ipv4Addr,
    pub subnet_mask: Ipv4Addr,
    pub pool_start_octet: u8,
    pub pool_end_octet: u8,
    pub serve_all_macs: bool,
}

impl Default for DhcpConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:67".to_string(),
            server_ip: Ipv4Addr::new(10, 88, 0, 1),
            subnet_mask: Ipv4Addr::new(255, 255, 255, 0),
            pool_start_octet: 10,
            pool_end_octet: 250,
            serve_all_macs: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LeaseEvent {
    pub ip: Ipv4Addr,
    pub mac: [u8; 6],
    pub is_pi_oui: bool,
    pub msg_type: u8,
}

#[derive(Debug, Clone)]
pub struct ParsedDhcp {
    pub xid: [u8; 4],
    pub flags: [u8; 2],
    pub chaddr: [u8; 6],
    pub msg_type: u8,
    pub is_pxe_client: bool,
}

pub struct LeaseTable {
    leases: HashMap<[u8; 6], Ipv4Addr>,
    prefix: [u8; 3],
    next_octet: u8,
    end_octet: u8,
}

impl LeaseTable {
    pub fn new(server_ip: Ipv4Addr, start_octet: u8, end_octet: u8) -> Self {
        let oct = server_ip.octets();
        Self {
            leases: HashMap::new(),
            prefix: [oct[0], oct[1], oct[2]],
            next_octet: start_octet,
            end_octet,
        }
    }

    pub fn allocate(&mut self, mac: [u8; 6]) -> Option<Ipv4Addr> {
        if let Some(&existing) = self.leases.get(&mac) {
            return Some(existing);
        }
        if self.next_octet > self.end_octet {
            return None;
        }
        let ip = Ipv4Addr::new(self.prefix[0], self.prefix[1], self.prefix[2], self.next_octet);
        self.next_octet = self.next_octet.saturating_add(1);
        self.leases.insert(mac, ip);
        Some(ip)
    }
}

pub fn is_raspberry_pi_mac(mac: &[u8; 6]) -> bool {
    let oui = [mac[0], mac[1], mac[2]];
    RPI_MAC_OUIS.contains(&oui)
}

pub fn format_mac(mac: &[u8; 6]) -> String {
    format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
    )
}

pub fn parse_dhcp_request(buf: &[u8]) -> Option<ParsedDhcp> {
    if buf.len() < 240 || buf[0] != 1 {
        return None;
    }
    if buf[236..240] != DHCP_MAGIC_COOKIE {
        return None;
    }
    let xid = [buf[4], buf[5], buf[6], buf[7]];
    let flags = [buf[10], buf[11]];
    let chaddr = [buf[28], buf[29], buf[30], buf[31], buf[32], buf[33]];

    let mut msg_type = 0u8;
    let mut is_pxe_client = false;
    let mut idx = 240usize;
    while idx < buf.len() {
        let code = buf[idx];
        if code == 255 {
            break;
        }
        if code == 0 {
            idx += 1;
            continue;
        }
        if idx + 1 >= buf.len() {
            break;
        }
        let len = buf[idx + 1] as usize;
        let val_start = idx + 2;
        let val_end = val_start.saturating_add(len);
        if val_end > buf.len() {
            break;
        }
        let val = &buf[val_start..val_end];
        if code == 53 && len >= 1 {
            msg_type = val[0];
        } else if code == 60 {
            let s = String::from_utf8_lossy(val);
            if s.contains("PXEClient") {
                is_pxe_client = true;
            }
        }
        idx = val_end;
    }

    if msg_type == 0 {
        return None;
    }
    Some(ParsedDhcp {
        xid,
        flags,
        chaddr,
        msg_type,
        is_pxe_client,
    })
}

pub fn build_dhcp_reply(
    req: &ParsedDhcp,
    assigned_ip: Ipv4Addr,
    cfg: &DhcpConfig,
    reply_msg_type: u8,
) -> Vec<u8> {
    let mut pkt = vec![0u8; 240];
    pkt[0] = 2; // BOOTREPLY
    pkt[1] = 1; // Ethernet
    pkt[2] = 6; // MAC len
    pkt[4..8].copy_from_slice(&req.xid);
    pkt[10..12].copy_from_slice(&req.flags);
    pkt[16..20].copy_from_slice(&assigned_ip.octets());
    pkt[20..24].copy_from_slice(&cfg.server_ip.octets());
    pkt[28..34].copy_from_slice(&req.chaddr);

    let bootfile = b"bootcode.bin";
    pkt[108..108 + bootfile.len()].copy_from_slice(bootfile);
    pkt[236..240].copy_from_slice(&DHCP_MAGIC_COOKIE);

    // Option 53: DHCP Message Type
    pkt.extend_from_slice(&[53, 1, reply_msg_type]);
    // Option 54: Server Identifier
    pkt.push(54);
    pkt.push(4);
    pkt.extend_from_slice(&cfg.server_ip.octets());
    // Option 51: Lease Time (3600s)
    pkt.push(51);
    pkt.push(4);
    pkt.extend_from_slice(&3600u32.to_be_bytes());
    // Option 1: Subnet Mask
    pkt.push(1);
    pkt.push(4);
    pkt.extend_from_slice(&cfg.subnet_mask.octets());
    // Option 3: Router
    pkt.push(3);
    pkt.push(4);
    pkt.extend_from_slice(&cfg.server_ip.octets());
    // Option 66: TFTP Server Name
    let server_str = cfg.server_ip.to_string();
    let server_bytes = server_str.as_bytes();
    pkt.push(66);
    pkt.push(server_bytes.len() as u8);
    pkt.extend_from_slice(server_bytes);
    // Option 60: Vendor Class Identifier ("PXEClient")
    let pxe = b"PXEClient";
    pkt.push(60);
    pkt.push(pxe.len() as u8);
    pkt.extend_from_slice(pxe);

    // Option 43: Broadcom VideoCore PXE Vendor-Specific Information
    // Suboption 6 (len 1, val 3) + Suboption 9 (PXE boot menu "Raspberry Pi Boot") + 0xff end
    let menu = b"Raspberry Pi Boot";
    let mut opt43 = Vec::with_capacity(32);
    opt43.extend_from_slice(&[6, 1, 3]);
    opt43.push(9);
    opt43.push((3 + menu.len()) as u8);
    opt43.extend_from_slice(&[0, 0, menu.len() as u8]);
    opt43.extend_from_slice(menu);
    opt43.push(0xff);

    pkt.push(43);
    pkt.push(opt43.len() as u8);
    pkt.extend_from_slice(&opt43);

    // Option 255: End
    pkt.push(255);
    if pkt.len() < 300 {
        pkt.resize(300, 0);
    }
    pkt
}

pub async fn run_dhcp_server(
    cfg: DhcpConfig,
    lease_tx: Option<mpsc::UnboundedSender<LeaseEvent>>,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(SocketAddr, tokio::task::JoinHandle<Result<()>>)> {
    let socket = UdpSocket::bind(&cfg.bind_addr)
        .await
        .with_context(|| format!("binding DHCP UDP socket on {}", cfg.bind_addr))?;
    let _ = socket.set_broadcast(true);
    let local_addr = socket.local_addr()?;

    let handle = tokio::spawn(async move {
        let mut table = LeaseTable::new(cfg.server_ip, cfg.pool_start_octet, cfg.pool_end_octet);
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

            let Some(req) = parse_dhcp_request(&buf[..len]) else {
                continue;
            };
            let pi_oui = is_raspberry_pi_mac(&req.chaddr);
            if !cfg.serve_all_macs && !pi_oui && !req.is_pxe_client {
                continue;
            }

            let reply_type = match req.msg_type {
                DHCP_DISCOVER => DHCP_OFFER,
                DHCP_REQUEST => DHCP_ACK,
                _ => continue,
            };

            let Some(assigned_ip) = table.allocate(req.chaddr) else {
                eprintln!("[dhcp] pool exhausted for {}", format_mac(&req.chaddr));
                continue;
            };

            let reply = build_dhcp_reply(&req, assigned_ip, &cfg, reply_type);
            let target = if peer.ip().is_loopback() {
                peer
            } else {
                SocketAddr::from((Ipv4Addr::BROADCAST, 68))
            };
            if let Err(err) = socket.send_to(&reply, target).await {
                eprintln!("[dhcp] send error to {target}: {err}");
            } else {
                eprintln!(
                    "[dhcp] {} -> {} ({}) mac={} pi_oui={}",
                    if reply_type == DHCP_OFFER { "OFFER" } else { "ACK" },
                    assigned_ip,
                    target,
                    format_mac(&req.chaddr),
                    pi_oui
                );
            }

            if let Some(tx) = &lease_tx {
                let _ = tx.send(LeaseEvent {
                    ip: assigned_ip,
                    mac: req.chaddr,
                    is_pi_oui: pi_oui,
                    msg_type: reply_type,
                });
            }
        }
        Ok(())
    });

    Ok((local_addr, handle))
}
