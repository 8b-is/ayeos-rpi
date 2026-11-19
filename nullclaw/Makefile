# ayeos-rpi / nullclaw / Makefile
CARGO ?= cargo
export CARGO_TARGET_DIR := $(CURDIR)/target
ARCH_MUSL := aarch64-unknown-linux-musl
ARCH_GNU := aarch64-unknown-linux-gnu

.PHONY: all build clean test run cross cross-musl build-wild pack-upx-standalone install-on-device

all: build

build:
	$(CARGO) build --release

test:
	$(CARGO) test

clean:
	$(CARGO) clean

run: build
	$(CARGO) run --release

# Zero-GCC static musl cross-compile for Pi 3B / Pi 4B RAM initramfs (uses Zig cc/ar + LLD)
cross-musl:
	rustup target add $(ARCH_MUSL) 2>/dev/null || true
	CC_aarch64_unknown_linux_musl="$(CURDIR)/../scripts/zig-aarch64-musl-cc.sh" \
	AR_aarch64_unknown_linux_musl="$(CURDIR)/../scripts/zig-aarch64-musl-ar.sh" \
	CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER="$(CURDIR)/../scripts/zig-aarch64-musl-cc.sh" \
	RUSTFLAGS="-C link-self-contained=no" \
	$(CARGO) build --release --target $(ARCH_MUSL)

# Legacy gnu target (prefer cross-musl for stateless initramfs)
cross:
	rustup target add $(ARCH_GNU) 2>/dev/null || true
	$(CARGO) build --release --target $(ARCH_GNU)

# Native Linux Pi 4B coordinator fast incremental build using Wild linker
build-wild:
	RUSTFLAGS="-C link-arg=--ld-path=wild" $(CARGO) build --release

# Optional standalone SCP binary packer (NEVER use inside initramfs — preserves W^X and avoids double compression)
pack-upx-standalone: cross-musl
	upx --best --lzma target/$(ARCH_MUSL)/release/nullclaw -o target/$(ARCH_MUSL)/release/nullclaw.upx

install-on-device: cross-musl
	@echo "Copy static musl binary to Pi and run:"
	@echo "  scp target/$(ARCH_MUSL)/release/nullclaw pi@<pi>:/usr/local/bin/nullclaw"
