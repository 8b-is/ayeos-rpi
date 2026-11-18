# ayeos-rpi / nullclaw / Makefile
CARGO ?= cargo

.PHONY: all build clean test run

all: build

build:
	$(CARGO) build --release

test:
	$(CARGO) test

clean:
	$(CARGO) clean

run: build
	$(CARGO) run --release

# Cross-compile for Pi (aarch64 unknown linux) — requires rustup target aarch64-unknown-linux-gnu
cross:
	rustup target add aarch64-unknown-linux-gnu 2>/dev/null || true
	$(CARGO) build --release --target aarch64-unknown-linux-gnu

install-on-device: cross
	@echo "Copy target/aarch64-unknown-linux-gnu/release/nullclaw to Pi and run:"
	@echo "  scp target/aarch64-unknown-linux-gnu/release/nullclaw pi@<pi>:/usr/local/bin/nullclaw"
