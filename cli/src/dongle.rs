use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::net::Ipv4Addr;
use std::process::Command;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DonglePort {
    pub port_name: String,
    pub device: String,
    pub mac: Option<String>,
    pub ipv4: Option<String>,
    pub link_active: bool,
    pub is_dongle_candidate: bool,
}

pub fn is_protected_iface(iface: &str) -> bool {
    let lower = iface.trim().to_ascii_lowercase();
    matches!(lower.as_str(), "en0" | "lo" | "lo0" | "en1" | "en2" | "en3")
        || lower.starts_with("bridge")
}

pub fn validate_dongle_iface(iface: &str, allow_any: bool) -> Result<()> {
    let trimmed = iface.trim();
    if trimmed.is_empty()
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        bail!("invalid network interface name: {iface:?}");
    }
    if !allow_any && is_protected_iface(trimmed) {
        bail!(
            "refusing protected interface `{trimmed}` (Wi-Fi/loopback/Thunderbolt). Pass a USB Ethernet dongle (e.g. en4, en5, en7) or --allow-any-iface."
        );
    }
    Ok(())
}

pub fn is_dongle_candidate(port_name: &str, device: &str) -> bool {
    if is_protected_iface(device) {
        return false;
    }
    let lower = port_name.to_ascii_lowercase();
    lower.contains("ethernet") || lower.contains("usb") || lower.contains("lan") || lower.contains("dongle")
}

pub fn parse_hardware_ports(output: &str) -> Vec<(String, String, Option<String>)> {
    let mut ports = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_dev: Option<String> = None;
    let mut current_mac: Option<String> = None;

    for raw_line in output.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            if let (Some(name), Some(dev)) = (current_name.take(), current_dev.take()) {
                ports.push((name, dev, current_mac.take()));
            }
            current_mac = None;
            continue;
        }
        if let Some(rest) = line.strip_prefix("Hardware Port:") {
            if let (Some(name), Some(dev)) = (current_name.take(), current_dev.take()) {
                ports.push((name, dev, current_mac.take()));
            }
            current_name = Some(rest.trim().to_string());
            current_dev = None;
            current_mac = None;
        } else if let Some(rest) = line.strip_prefix("Device:") {
            current_dev = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("Ethernet Address:") {
            let val = rest.trim();
            if !val.is_empty() && val != "N/A" {
                current_mac = Some(val.to_string());
            }
        }
    }
    if let (Some(name), Some(dev)) = (current_name, current_dev) {
        ports.push((name, dev, current_mac));
    }
    ports
}

fn inspect_macos_ifconfig(device: &str) -> (Option<String>, bool) {
    let Ok(out) = Command::new("ifconfig").arg(device).output() else {
        return (None, false);
    };
    if !out.status.success() {
        return (None, false);
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut ipv4 = None;
    let mut active = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("inet ") {
            if let Some(addr) = rest.split_whitespace().next() {
                ipv4 = Some(addr.to_string());
            }
        } else if trimmed == "status: active" {
            active = true;
        }
    }
    (ipv4, active)
}

#[cfg(target_os = "macos")]
pub fn discover_dongles() -> Result<Vec<DonglePort>> {
    let out = Command::new("networksetup")
        .arg("-listallhardwareports")
        .output()
        .context("running networksetup -listallhardwareports")?;
    if !out.status.success() {
        bail!(
            "networksetup failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let parsed = parse_hardware_ports(&text);
    let mut ports = Vec::with_capacity(parsed.len());
    for (port_name, device, mac) in parsed {
        let (ipv4, link_active) = inspect_macos_ifconfig(&device);
        let candidate = is_dongle_candidate(&port_name, &device);
        ports.push(DonglePort {
            port_name,
            device,
            mac,
            ipv4,
            link_active,
            is_dongle_candidate: candidate,
        });
    }
    Ok(ports)
}

#[cfg(not(target_os = "macos"))]
pub fn discover_dongles() -> Result<Vec<DonglePort>> {
    let mut ports = Vec::new();
    let entries = std::fs::read_dir("/sys/class/net").context("reading /sys/class/net")?;
    for entry in entries.flatten() {
        let device = entry.file_name().to_string_lossy().to_string();
        let mac = std::fs::read_to_string(entry.path().join("address"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && s != "00:00:00:00:00:00");
        let oper = std::fs::read_to_string(entry.path().join("operstate"))
            .unwrap_or_default()
            .trim()
            .to_string();
        let (ipv4, _) = inspect_macos_ifconfig(&device);
        let candidate = !is_protected_iface(&device)
            && (device.starts_with("eth") || device.starts_with("enx") || device.starts_with("usb"));
        ports.push(DonglePort {
            port_name: format!("Linux Net ({device})"),
            device,
            mac,
            ipv4,
            link_active: oper == "up",
            is_dongle_candidate: candidate,
        });
    }
    ports.sort_by(|a, b| a.device.cmp(&b.device));
    Ok(ports)
}

pub fn configure_dongle_ip(
    iface: &str,
    ip: &str,
    netmask: &str,
    dry_run: bool,
    allow_any: bool,
) -> Result<String> {
    validate_dongle_iface(iface, allow_any)?;
    let _parsed_ip: Ipv4Addr = ip.parse().with_context(|| format!("invalid IPv4: {ip}"))?;
    let _parsed_mask: Ipv4Addr = netmask
        .parse()
        .with_context(|| format!("invalid netmask: {netmask}"))?;

    let cmd_str = format!("ifconfig {iface} inet {ip} netmask {netmask} up");
    if dry_run {
        return Ok(cmd_str);
    }

    let out = Command::new("ifconfig")
        .args([iface, "inet", ip, "netmask", netmask, "up"])
        .output()
        .with_context(|| format!("executing `{cmd_str}`"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        bail!(
            "failed `{cmd_str}`: {}. Run with elevated privileges: sudo ayeos launch --iface {iface}",
            stderr.trim()
        );
    }
    Ok(cmd_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_wifi_and_loopback_interfaces() {
        assert!(validate_dongle_iface("en0", false).is_err());
        assert!(validate_dongle_iface("lo0", false).is_err());
        assert!(validate_dongle_iface("bridge0", false).is_err());
        assert!(validate_dongle_iface("en1", false).is_err());
        assert!(validate_dongle_iface("en2", false).is_err());
        assert!(validate_dongle_iface("en3", false).is_err());
        assert!(validate_dongle_iface("en5", false).is_ok());
        assert!(validate_dongle_iface("en0", true).is_ok());
    }

    #[test]
    fn parses_networksetup_hardware_ports_and_candidates() {
        let sample = "\
Hardware Port: Ethernet Adapter (en5)
Device: en5
Ethernet Address: 22:a6:f5:16:22:dc

Hardware Port: Wi-Fi
Device: en0
Ethernet Address: f0:2f:4b:07:5c:c7
";
        let ports = parse_hardware_ports(sample);
        assert_eq!(ports.len(), 2);
        assert!(is_dongle_candidate(&ports[0].0, &ports[0].1));
        assert!(!is_dongle_candidate(&ports[1].0, &ports[1].1));
    }
}
