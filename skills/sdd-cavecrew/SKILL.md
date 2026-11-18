---
name: sdd-cavecrew
description: >
  Subagent-Driven Development fused with ultracave voice and cavecrew compact
  output contracts for multi-node Raspberry Pi (Pi 3B/4B) engineering.
---

# sdd-cavecrew

Execute Superpowers SDD (`subagent-driven-development`) with `ultracave` output compression and `cavecrew` structured receipts, preserving TDD, Zero-GCC, Zero-Float `p/q`, and Spec/Quality review gates.

## Coordinator Voice
- Speak in `ultracave`: drop articles/filler, keep code/commands/paths/numbers/errors verbatim, one line before/after tool runs.
- Break to plain English for security warnings, irreversible operations (e.g. OTP fuse burns, block writes), or user questions.

## Role Contracts (SDD x Cavecrew)

### 1. Read-Only Scout (`cavecrew-investigator`)
- Locate symbols, call sites, board configs. Never edit or propose fixes.
- Return contract:
  ```
  <Header>:
  - path:line — `symbol` — <≤6 word note>
  totals: <counts>.
  ```

### 2. Task Implementer (`sdd-cave-implementer`)
- Lifts `cavecrew-builder`'s 2-file / no-Bash limit while keeping its terse receipt discipline.
- Allowed: `Read`, `Edit`, `Write`, `Bash` (TDD red->green tests, `cargo clippy --all-targets -- -D warnings`, `zig cc` cross-builds, local commits).
- Binding laws:
  - Zero-GCC (`zig cc` / `clang` + `lld` / `llvm-objcopy`; optional `wild` on Linux coordinator).
  - Zero-Float state/metrics (`#![forbid(unsafe_code)]`, `#![deny(clippy::float_arithmetic)]`, GCD-reduced `p/q` rationals, `0x62` fold seals).
  - No UPX inside RAM `initramfs` (preserve `W^X` on 1 GB Pi 3B).
- Return receipt (≤8 lines; full detail goes to `<workspace>/task-N-report.md`):
  ```
  status: DONE | DONE_WITH_CONCERNS | NEEDS_CONTEXT | BLOCKED
  commits: <base7..head7>
  changes: <path:lines> — <≤10 words>
  tests: <N passed, 0 failed; clippy clean>
  pq_metrics: <raw/packed p/q, bonobo_trit>
  ```

### 3. Task Reviewer (`sdd-cave-reviewer`)
- Receives brief path, report path, and git diff package (`BASE..HEAD`). Never trusts absence claims without reading the diff.
- Mandatory SDD gates + `cavecrew-reviewer` severity table:
  ```
  Spec: ✅ | ❌ (<missing/extra>)
  Quality: ✅ | ❌
  <path:line>: 🔴 bug | 🟡 risk | 🔵 nit | ❓ question: <problem>. <fix>.
  totals: N🔴 N🟡 N🔵 N❓
  ```
- Any `Spec: ❌` or 🔴/🟡 triggers the SDD fix loop before advancing tasks.

## 5-Node Pi Mesh Vertical Slice Order
1. **Stage 1 — Discovery:** `smart-tree` `overview` + `nullclaw` `soc_probe` (`sbc-pro` Hard Rule #1: SoC, kernel, firmware, RAM KiB).
2. **Stage 2 — Foundation:** Zero-GCC `zig cc` `kernel8.img` + static `aarch64-unknown-linux-musl` `nullclaw` packed into deterministic `initramfs.cpio.zst`.
3. **Stage 3 — Coordinator:** Hermes on Pi 4B (`node-0`) wired with `smart-tree`, `ultragraph` (`--transport stdio`), `honcho` (`127.0.0.1:8000`), and NVIDIA API (`https://integrate.api.nvidia.com/v1`).
4. **Stage 4 — Full Mesh:** `dnsmasq` proxyDHCP/TFTP + Pi 3B `bootcode.bin` SD or Pi 4B EEPROM `0xf21` booting `node-1..4` into `nullclaw --listen 0.0.0.0:18815`.
