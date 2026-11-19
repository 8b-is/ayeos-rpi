#!/usr/bin/env bash
set -euo pipefail

# ayeos-rpi v0.1 PoC — Deterministic Zero-GCC initramfs.cpio.zst packer
# Builds a clean RAM-only rootfs containing static musl nullclaw + g8t skills (sbc-pro, arm-master).

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"
STAGE_DIR="${ROOT_DIR}/build/initramfs_root"
G8T_PLUGINS_DIR="${G8T_PLUGINS_DIR:-/Users/peterlodri/workspace/peterlodri-sec/g8t-plugins/plugins}"
NULLCLAW_BIN="${NULLCLAW_BIN:-${ROOT_DIR}/nullclaw/target/aarch64-unknown-linux-musl/release/nullclaw}"

if [ ! -f "$NULLCLAW_BIN" ]; then
    FALLBACK_BIN="/Users/peterlodri/forge-workspace/target/aarch64-unknown-linux-musl/release/nullclaw"
    if [ -f "$FALLBACK_BIN" ]; then
        NULLCLAW_BIN="$FALLBACK_BIN"
    else
        echo "[pack_initramfs] missing static musl nullclaw at $NULLCLAW_BIN" >&2
        echo "[pack_initramfs] run: make -C \"$ROOT_DIR/nullclaw\" cross-musl" >&2
        exit 1
    fi
fi

command -v cpio >/dev/null 2>&1 || { echo "[pack_initramfs] cpio required" >&2; exit 1; }
command -v zstd >/dev/null 2>&1 || { echo "[pack_initramfs] zstd required" >&2; exit 1; }

rm -rf "$STAGE_DIR"
mkdir -p "$DIST_DIR" \
    "$STAGE_DIR/bin" \
    "$STAGE_DIR/sbin" \
    "$STAGE_DIR/proc" \
    "$STAGE_DIR/sys" \
    "$STAGE_DIR/dev" \
    "$STAGE_DIR/etc/agents/skills" \
    "$STAGE_DIR/tmp" \
    "$STAGE_DIR/var"

cp "$NULLCLAW_BIN" "$STAGE_DIR/bin/nullclaw"
chmod 0755 "$STAGE_DIR/bin/nullclaw"

cp "$SCRIPT_DIR/init" "$STAGE_DIR/init"
chmod 0755 "$STAGE_DIR/init"

# Stage g8t-plugins skills (sbc-pro + arm-master) for on-device nullclaw discovery
for skill in sbc-pro arm-master; do
    SRC_SKILL="$G8T_PLUGINS_DIR/$skill"
    if [ -d "$SRC_SKILL" ]; then
        DST_SKILL="$STAGE_DIR/etc/agents/skills/$skill"
        mkdir -p "$DST_SKILL/references"
        [ -f "$SRC_SKILL/SKILL.md" ] && cp "$SRC_SKILL/SKILL.md" "$DST_SKILL/SKILL.md"
        [ -f "$SRC_SKILL/references/epistemic-ledger.md" ] && \
            cp "$SRC_SKILL/references/epistemic-ledger.md" "$DST_SKILL/references/epistemic-ledger.md"
    fi
done

# Stamp fixed mtimes for deterministic cpio archives across rebuilds
find "$STAGE_DIR" -exec touch -t 202601010000 {} +

CPIO_OUT="$DIST_DIR/initramfs.cpio"
ZST_OUT="$DIST_DIR/initramfs.cpio.zst"
MANIFEST_OUT="$DIST_DIR/netboot-manifest.json"

(
    cd "$STAGE_DIR"
    find . | LC_ALL=C sort | cpio -o --format newc > "$CPIO_OUT" 2>/dev/null
)

zstd -19 -f -q "$CPIO_OUT" -o "$ZST_OUT"

python3 - "$NULLCLAW_BIN" "$CPIO_OUT" "$ZST_OUT" "$MANIFEST_OUT" <<'PY'
import hashlib
import json
import math
import sys
from pathlib import Path

nullclaw_path, cpio_path, zst_path, manifest_path = [Path(p) for p in sys.argv[1:5]]

def sha256_file(p: Path) -> str:
    h = hashlib.sha256()
    with p.open("rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()

raw_bytes = cpio_path.stat().st_size
packed_bytes = zst_path.stat().st_size
g = math.gcd(raw_bytes, packed_bytes) if packed_bytes > 0 else 1
p = raw_bytes // g if packed_bytes > 0 else 0
q = packed_bytes // g if packed_bytes > 0 else 1
bonobo_trit = 0 if (raw_bytes == 0 or packed_bytes == 0 or raw_bytes == packed_bytes) else (
    1 if raw_bytes * 2 >= packed_bytes * 3 else -1
)

manifest = {
    "spec": "ayeos-rpi-netboot/0.3.0",
    "toolchain": "zero-gcc (zig-0.17.1 + llvm/lld + musl)",
    "upx_in_initramfs": False,
    "artifacts": {
        "nullclaw_musl": {
            "path": str(nullclaw_path),
            "bytes": nullclaw_path.stat().st_size,
            "sha256": sha256_file(nullclaw_path),
        },
        "initramfs_cpio": {
            "path": str(cpio_path),
            "bytes": raw_bytes,
            "sha256": sha256_file(cpio_path),
        },
        "initramfs_cpio_zst": {
            "path": str(zst_path),
            "bytes": packed_bytes,
            "sha256": sha256_file(zst_path),
        },
    },
    "compression_ratio_pq": {"p": p, "q": q},
    "bonobo_resonance_trit": bonobo_trit,
}

manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
print(json.dumps(manifest, indent=2))
PY
