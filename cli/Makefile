# ayeos-rpi / cli / Makefile
CARGO ?= cargo
export CARGO_TARGET_DIR := $(CURDIR)/target

.PHONY: all build test clippy install clean

all: build

build:
	$(CARGO) build --release

test:
	$(CARGO) test

clippy:
	$(CARGO) clippy --all-targets -- -D warnings

install: build
	cp $(CURDIR)/target/release/ayeos $(HOME)/.cargo/bin/ayeos

clean:
	$(CARGO) clean
