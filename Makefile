# ayeos-rpi — Makefile
# Builds kernel8.img (bare-metal ARM64), ayeOS payload, burner (host CLI),
# nullclaw (on-device / musl mesh agent), and LAN netboot initramfs.
#
# Zero-GCC Toolchain: Zig cc (Clang/LLD) + llvm-objcopy for kernel; cargo + zig cc for burner/nullclaw.

SHELL := /bin/bash
ZIG      ?= zig
CC       := $(ZIG) cc -target aarch64-freestanding-none
AS       := $(ZIG) cc -target aarch64-freestanding-none
LD       := $(ZIG) cc -target aarch64-freestanding-none -nostdlib -Wl,--build-id=none
OBJCOPY  := $(shell command -v /opt/homebrew/opt/llvm/bin/llvm-objcopy || command -v llvm-objcopy || echo llvm-objcopy)

KERNEL_DIR   := kernel
AYEOS_DIR    := ayeos
BURNER_DIR   := burner
NULLCLAW_DIR := nullclaw
NETBOOT_DIR  := netboot
BUILD_DIR    := build
DIST_DIR     := dist

KERNEL_ELF   := $(BUILD_DIR)/kernel8.elf
KERNEL_IMG   := $(DIST_DIR)/kernel8.img
AYEOS_ELF    := $(BUILD_DIR)/ayeos.elf
AYEOS_IMG    := $(DIST_DIR)/ayeos.img

CFLAGS   := -Wall -Wextra -O2 -ffreestanding -nostdlib \
            -mcpu=cortex_a53 -I$(KERNEL_DIR) -I$(AYEOS_DIR)
ASFLAGS  := -mcpu=cortex_a53

KERNEL_SRCS := $(KERNEL_DIR)/arch/aarch64/entry.S \
               $(KERNEL_DIR)/main.c \
               $(KERNEL_DIR)/uart.c \
               $(KERNEL_DIR)/timer.c \
               $(KERNEL_DIR)/mm.c
KERNEL_OBJS_TMP := $(KERNEL_SRCS:.S=.o)
KERNEL_OBJS := $(patsubst $(KERNEL_DIR)/%,$(BUILD_DIR)/kernel/%,$(KERNEL_OBJS_TMP:.c=.o))

AYEOS_SRCS := $(AYEOS_DIR)/main.c \
              $(AYEOS_DIR)/init.c \
              $(AYEOS_DIR)/syscalls.c \
              $(AYEOS_DIR)/fs.c \
              $(AYEOS_DIR)/shell.c
AYEOS_OBJS := $(patsubst $(AYEOS_DIR)/%,$(BUILD_DIR)/ayeos/%,$(AYEOS_SRCS:.c=.o))

KERNEL_LINKER := $(KERNEL_DIR)/linker.ld
AYEOS_LINKER  := $(AYEOS_DIR)/linker.ld

.PHONY: all kernel ayeos burner nullclaw netboot clean dist flash run help

all: kernel ayeos burner nullclaw dist

kernel: $(KERNEL_IMG)

ayeos: $(AYEOS_IMG)

burner:
	$(MAKE) -C $(BURNER_DIR)

nullclaw:
	$(MAKE) -C $(NULLCLAW_DIR)

netboot:
	bash $(NETBOOT_DIR)/pack_initramfs.sh

dist: $(KERNEL_IMG) $(AYEOS_IMG)
	@mkdir -p $(DIST_DIR)
	@cp config.txt $(DIST_DIR)/config.txt 2>/dev/null || true
	@ls -la $(DIST_DIR)

$(BUILD_DIR)/kernel/%.o: $(KERNEL_DIR)/%.c
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -c $< -o $@

$(BUILD_DIR)/kernel/%.o: $(KERNEL_DIR)/%.S
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -c $< -o $@

$(BUILD_DIR)/ayeos/%.o: $(AYEOS_DIR)/%.c
	@mkdir -p $(dir $@)
	$(CC) $(CFLAGS) -c $< -o $@

$(KERNEL_ELF): $(KERNEL_OBJS) $(KERNEL_LINKER)
	$(LD) -Wl,-T,$(KERNEL_LINKER) -o $@ $(KERNEL_OBJS)

$(KERNEL_IMG): $(KERNEL_ELF)
	@mkdir -p $(DIST_DIR)
	$(OBJCOPY) -O binary $< $@

$(AYEOS_ELF): $(AYEOS_OBJS) $(BUILD_DIR)/kernel/uart.o $(AYEOS_LINKER)
	$(LD) -Wl,-T,$(AYEOS_LINKER) -Wl,-e,ayeos_entry -o $@ $(AYEOS_OBJS) $(BUILD_DIR)/kernel/uart.o

$(AYEOS_IMG): $(AYEOS_ELF)
	@mkdir -p $(DIST_DIR)
	$(OBJCOPY) -O binary $< $@

clean:
	rm -rf $(BUILD_DIR) $(DIST_DIR)
	$(MAKE) -C $(BURNER_DIR) clean 2>/dev/null || true
	$(MAKE) -C $(NULLCLAW_DIR) clean 2>/dev/null || true

flash: $(KERNEL_IMG) $(AYEOS_IMG)
	@echo "Flash $(KERNEL_IMG) and $(AYEOS_IMG) to SD card, then insert into Pi 3/4."

run: $(KERNEL_IMG)
	@echo "Boot kernel8.img on Pi 3/4 (QEMU: qemu-system-aarch64 -M raspi3b -kernel $(KERNEL_IMG))"

help:
	@echo "Targets: all kernel ayeos burner nullclaw netboot dist clean flash run"
