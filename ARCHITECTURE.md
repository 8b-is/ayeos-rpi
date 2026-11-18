# ayeos-rpi — system architecture

## Component breakdown

| Component | Responsibility | Language | Target |
|---|---|---|---|
| `kernel8` | Bare-metal ARM kernel (Pi 3 BCM2837, Pi 4 BCM2711), boot block, exception vectors, MMU, timer, UART | ARM asm + C | `aarch64-none-elf` / `arm-none-eabi` |
| `ayeOS` | Minimal OS: init, drivers (UART, GPIO, SD/MMC), syscalls, fs (FAT), shell | C + asm | `aarch64-none-elf` |
| `burner` | OS-agnostic SD-card image burner CLI, ratatui TUI, pkgmgr detection (apt/apk/brew) | Rust | host (Linux/macOS/BSD) |
| `nullclaw` | On-device assistance agent, reads `~/.agents/skills/`, exposes local MCP-over-serial/TTY | Rust | `aarch64-unknown-linux-gnu` (on-device) |

## Interfaces

- **kernel8 → ayeOS**: kernel loads ayeOS image from SD/FAT into memory at `0x80000`, jumps to entry.
- **ayeOS → nullclaw**: ayeOS provides a minimal POSIX-ish syscall layer; nullclaw runs as a userspace daemon reading skill files from `/mnt/skills` (SD card) or `~/.agents/skills`.
- **burner → host pkgmgr**: detects `apt`/`apt-get`/`apk`/`brew` at runtime, uses the available one to install burner deps (`libusb`, `udev`).
- **burner → SD card**: libusb/block-device write of `kernel8.img` + `ayeos.img` + config.

## Version matrix

| Board | SoC | CPU | Arch | kernel8 target |
|---|---|---|---|---|
| Pi 3 / 3+ | BCM2837 | Cortex-A53 | AArch64 | `aarch64-none-elf` |
| Pi 4 / 400 | BCM2711 | Cortex-A72 | AArch64 | `aarch64-none-elf` |

Both use the same `kernel8.img` entry convention (`kernel8.img` = 64-bit, `kernel7.img` = 32-bit Pi 3).

## Boot flow (Pi 3/4)

1. VideoCore GPU boots, reads `config.txt`, loads `kernel8.img`.
2. `kernel8` entry (`_start`) in `kernel/arch/aarch64/entry.S`: set up stack, clear BSS, set exception vectors (VBAR), enable timer, init UART (PL011 on Pi 3/4 miniuart vs PL011), jump to `kernel_main`.
3. `kernel_main` (C): init drivers, mount FAT, load `ayeos.bin`, jump to `ayeos_entry`.
4. `ayeOS`: init syscalls, fs shell, spawn `nullclaw` on `/dev/ttyAMA0` or `/dev/ttyS0`.

## Files produced

- `kernel8.img` — bootable kernel image
- `ayeos.img` — ayeOS payload
- `config.txt` — Pi boot config
- `burner` — host CLI binary
- `nullclaw` — on-device agent binary
