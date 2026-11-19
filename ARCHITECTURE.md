# ayeos-rpi — system architecture (v0.3.0)

## Component breakdown

| Component | Responsibility | Language | Target |
|---|---|---|---|
| `kernel8` | Bare-metal ARM kernel (Pi 3 BCM2837, Pi 4 BCM2711), boot block, exception vectors, MMU, timer, UART | ARM asm + C (`zig cc` + `llvm-objcopy`) | `aarch64-freestanding-none` |
| `ayeOS` | Minimal OS: init, drivers (UART, GPIO, SD/MMC), syscalls, fs (FAT), shell | C + asm (`zig cc`) | `aarch64-freestanding-none` |
| `ayeos` (`cli/`) | Sovereign MacBook USB-dongle mesh driver: pure-Rust BOOTP/DHCP (Option 43), RFC-1350/2348 TFTP, safe SD/bootcode burner, TCP `:18815` ping, `8b-is/ultrameshai` `topology.toml` sync/router, & `forge-push` | Rust | host (macOS/Linux) |
| `burner` | Legacy SD-card / TFTP staging CLI, ratatui TUI, pkgmgr detection (`apt`/`apk`/`brew`) | Rust | host (Linux/macOS/BSD) |
| `nullclaw` | On-device + LAN mesh worker daemon, reads `~/.agents/skills/` or `/etc/agents/skills`, exposes JSON-RPC 2.0 MCP over TCP `:18815` & stdio | Rust (`zig cc` musl / `wild`) | `aarch64-unknown-linux-musl` (static RAM initramfs) |

## MacBook USB-Dongle → Ethernet Switch → `8b-is/ultrameshai` Constellation (v0.3.0)

```
  [MacBook Pro]
    ├── en0 (Wi-Fi) ──▶ Hermes + UltraGraph + Honcho (:8000) + OmniTodos (:8099) + Bonobo (:8787) + WA-Stream (:8790) + Ollama (:11434)
    ├── ~/.hermes/ultramesh_topology.toml ◀── hot-reloaded by 8b-is/ultrameshai ClusterTopologyRouter
    └── enX (USB Ethernet Dongle, 10.88.0.1/24)
          └──▶ [5-Port Ethernet Switch]
                 ├── node-1 (Pi 3B / Pi 4B) ── PXE DHCP Option 43 + TFTP ──▶ initramfs + nullclaw :18815
                 ├── node-2 .. node-5        ── 10.88.0.10..250             ──▶ ~/.hermes/rpi_mesh_nodes.json
```

## Version matrix

| Board | SoC | CPU | Arch | Netboot Path |
|---|---|---|---|---|
| Pi 3 / 3+ | BCM2837 | Cortex-A53 (1 GB RAM) | AArch64 | FAT32 SD `bootcode.bin` + `timeout` (or OTP bit 17) → PXE + RAM `initramfs.cpio.zst` |
| Pi 4 / 400 | BCM2711 | Cortex-A72 (2–8 GB RAM) | AArch64 | SPI EEPROM `BOOT_ORDER=0xf21` → PXE + RAM `initramfs.cpio.zst` |

## Files produced

- `dist/kernel8.img` — bare-metal ARM64 kernel image (zero-GCC Zig/LLVM build)
- `dist/ayeos.img` — ayeOS payload
- `dist/initramfs.cpio.zst` — deterministic stateless worker RAM rootfs (`nullclaw` musl + `sbc-pro` + `arm-master`)
- `dist/netboot-manifest.json` — SHA-256 digests, `p/q` compression ratio, and Bonobo resonance trit
- `cli/target/release/ayeos` (`~/.cargo/bin/ayeos`) — MacBook USB-dongle launcher, burner & mesh CLI
- `nullclaw/target/aarch64-unknown-linux-musl/release/nullclaw` — static musl on-device MCP worker
