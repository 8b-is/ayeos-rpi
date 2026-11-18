# ayeos-rpi / burner / Makefile
CARGO ?= cargo

.PHONY: all build clean test run text

all: build

build:
	$(CARGO) build --release

tui:
	$(CARGO) build --release --features tui

test:
	$(CARGO) test

clean:
	$(CARGO) clean

run: build
	$(CARGO) run --release -- --list

text: build
	$(CARGO) run --release -- --text --device /dev/null --kernel dist/kernel8.img --ayeos dist/ayeos.img

install-deps:
	@command -v apt >/dev/null && sudo apt install -y libudev-dev || true
	@command -v brew >/dev/null && brew install libudev || true
