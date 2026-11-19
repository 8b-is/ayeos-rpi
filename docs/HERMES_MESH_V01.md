# Hermes + `8b-is/ultrameshai` + 5-Node Raspberry Pi Sovereign Mesh — v0.3.0

This guide wires **Hermes** (`~/.hermes/`) with **`8b-is/ultrameshai`** (`ClusterTopologyRouter` + `node-registry` + `custodian`), **`8b-is/ayeos-rpi` (`ayeos` CLI)**, **`8b-is/g8t-plugins`** (`sbc-pro`, `arm-master`, `esp32-pro`, `assembly-pro`, `retro-game-dev`), **`8b-is/ultra-graph`**, **`8b-is/smart-tree`**, **`8b-is/eightb-agy-fanout` (`8b-agy`)**, **`8b-is/kompress-super-ultra` / `bonobo`**, **`8b-is/omni-todos`**, **`8b-is/wa-stream`**, **`8b-is/omnispherepop`**, **`8b-is/mempalace`**, **`8b-is/space-bender`**, and **Honcho**.

## 0. MacBook USB-Dongle + `8b-is/ultrameshai` Quick Launch (v0.3.0)

Plug a USB Ethernet dongle (`en4` / `en5` / `en7`) into your MacBook and connect it to the Ethernet switch holding the Raspberry Pis while keeping `en0` (Wi-Fi) online for NVIDIA API / Honcho:

```bash
# 1. Inspect live 8b-is cluster ports, binaries, repos, and booted Pis
ayeos mesh status

# 2. Emit ~/.hermes/ultramesh_topology.toml (for 8b-is/ultrameshai) & ~/.hermes/8b_is_constellation.json
ayeos mesh sync-topology

# 3. Discover attached USB Ethernet dongles (excludes Wi-Fi en0 / loopback / Thunderbolt)
ayeos dongle list

# 4. (Optional, Pi 3B without OTP netboot) Stage a FAT32 SD card with bootcode.bin
ayeos burn pi3-sd --dir /Volumes/PI3BOOT --bootcode /path/to/bootcode.bin

# 5. Assign 10.88.0.1/24, serve PXE DHCP (Option 43) + TFTP, probe :18815 & hot-update ultramesh_topology.toml
sudo ayeos launch --iface en5 --tftp-dir dist/tftp

# 6. Probe booted workers or route a capability via ultrameshai rules
ayeos ping
ayeos mesh route arm64
```

---

## 1. Zero-GCC Toolchain Architecture

| Component | Host Cross-Build (macOS) | Coordinator Native (Pi 4B Linux) | Notes |
|---|---|---|---|
| **Bare-metal C/Asm** (`kernel8.img`, `ayeos.img`) | `zig cc -target aarch64-freestanding-none` + `llvm-objcopy` | `clang` / `zig cc` + `llvm-objcopy` | `-mcpu=cortex_a53` runs on both Pi 3B (`BCM2837`) and Pi 4B (`BCM2711`). |
| **Worker Daemon** (`nullclaw`) | `zig cc -target aarch64-linux-musl` (static binary) | `wild` linker (`make -C nullclaw build-wild`) | `opt-level="z"`, LTO, stripped, `#![forbid(unsafe_code)]`, `#![deny(clippy::float_arithmetic)]`. |
| **Initramfs Packer** | `cpio --format newc` + `zstd -19` | `cpio` + `zstd -19` | No `UPX` inside RAM `initramfs`: preserves `W^X` pages on 1 GB Pi 3B and avoids double compression. |

---

## 2. Node Hardware Preparation

### Pi 4 Model B Coordinator (`node-0`) + Pi 4B Workers
1. Ensure SPI EEPROM netboot order tries SD (`1`), then Network (`2`), then restart (`f`):
   ```bash
   sudo rpi-eeprom-config --edit
   # Set: BOOT_ORDER=0xf21
   ```
2. On `node-0` (Pi 4B Linux), stage TFTP files and start `dnsmasq` proxyDHCP/TFTP:
   ```bash
   cd ayeos-rpi
   make kernel ayeos
   make -C nullclaw cross-musl
   make netboot
   cargo run --manifest-path burner/Cargo.toml -- --stage-tftp /srv/tftp --netboot-dir netboot --dist-dir dist
   sudo cp netboot/dnsmasq-pxe.conf /etc/dnsmasq.d/ayeos-pxe.conf
   sudo systemctl restart dnsmasq
   ```
   > **Note:** `dist/kernel8.img` in this repo is the bare-metal `ayeos-rpi` UART scaffold. For Linux RAM `initramfs` boot on Pi 3B/4B workers, copy your 64-bit Linux `Image`/`kernel8.img` + Broadcom `.dtb` (`bcm2710-rpi-3-b.dtb`, `bcm2711-rpi-4-b.dtb`) + firmware `start4.elf`/`fixup4.dat`/`start.elf`/`fixup.dat` into `/srv/tftp` (or pass `--kernel <linux-kernel8.img>` to `burner --stage-tftp`).

### Pi 3 Model B Workers (`node-1..4`) — Reversible vs. OTP Boot
- **Recommended (Reversible):** Format a tiny SD card as FAT32 containing **only** official Raspberry Pi `bootcode.bin` (and optional empty `timeout` file). Stage instructions with:
  ```bash
  cargo run --manifest-path burner/Cargo.toml -- --stage-pi3-bootcode /tmp/pi3-sd-stage --bootcode /path/to/bootcode.bin
  ```
- **Pure SD-less (Irreversible OTP bit 17):** Boot Raspberry Pi OS once on the Pi 3B with `program_usb_boot_mode=1` in `/boot/config.txt`, then confirm `vcgencmd otp_dump | grep 17:` prints `17:3020000a`.

---

## 3. Hermes Configuration (NVIDIA API + UltraGraph `stdio` + Honcho)

`~/.hermes/config.yaml` uses `stdio` for UltraGraph so it never collides with Honcho on `127.0.0.1:8000`:

```yaml
model:
  default: meta/llama-3.3-70b-instruct # or deepseek-ai/deepseek-v3.2 via NVIDIA NIM
  provider: nvidia
  base_url: https://integrate.api.nvidia.com/v1
  api_mode: chat_completions

mcp_servers:
  smart-tree:
    command: st
    args: ["--mcp"]
    supports_parallel_tool_calls: true
  ultragraph:
    command: /Users/peterlodri/.local/bin/uv
    args:
      - run
      - --frozen
      - --directory
      - /Users/peterlodri/workspace/peterlodri-sec/ultra-graph
      - --extra
      - mcp
      - python
      - mcp_server/server.py
      - --transport
      - stdio
  nullclaw-local:
    command: /Users/peterlodri/workspace/peterlodri-sec/8b-is-repos/ayeos-rpi/nullclaw/target/release/nullclaw
    args: ["--stdio", "--node-id", "coordinator-sim"]
```

Put `NVIDIA_API_KEY=...` in `~/.hermes/.env` (never commit keys).

---

## 4. Single Hermes Kickoff Prompt (Start Fresh Session)

```text
Load skills `sdd-cavecrew`, `sbc-pro`, `arm-master`, `ultracave`, and `cavecrew`.
We are building and verifying our 5-node Raspberry Pi (Pi 3 Model B + Pi 4 Model B) sovereign mesh:
- Coordinator: Hermes on node-0 (Pi 4B), `smart-tree` for project context, `ultragraph` MCP (`stdio`) as the 1.58-bit capability layer, Honcho (`127.0.0.1:8000`) for shared memory, NVIDIA API for inference.
- Workers: static musl `nullclaw` over TCP `:18815` running from stateless `initramfs.cpio.zst` served by `dnsmasq` proxyDHCP/TFTP.
- Engineering method: Superpowers SDD with `ultracave` + `cavecrew` compact receipts, Zero-GCC (`zig cc`/`clang`/`wild`), Zero-Float exact `p/q` accounting.

Execute one vertical slice at a time:
1. Discovery: run `smart-tree` `overview` on `ayeos-rpi` and call `nullclaw` `soc_probe` + `mesh_ping` on `nullclaw-local`.
2. Foundation: verify `kernel8.img`, `nullclaw` (`aarch64-unknown-linux-musl`), and `dist/netboot-manifest.json`.
3. Coordinator: verify `ultragraph` (`ultragraph_info`) and sync node capability state to Honcho.
4. Mesh expansion: discover and `mesh_ping` Pi 3B/4B workers on `:18815` one node at a time.
```
