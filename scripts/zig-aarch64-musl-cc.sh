#!/bin/sh
args=""
set -- "$@"
out=""
# Strip -Wl,--fix-cortex-a53-843419 which Zig 0.17's zig cc arg parser rejects
exec python3 -c '
import os, sys
argv = [a for a in sys.argv[1:] if a != "-Wl,--fix-cortex-a53-843419"]
os.execvp("zig", ["zig", "cc", "-target", "aarch64-linux-musl", *argv])
' "$@"
