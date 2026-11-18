# ayeos-rpi — Makefile
# Builds kernel8.img (bare-metal ARM64), ayeOS payload, burner (host CLI), nullclaw (on-device agent).
#
# Toolchain: aarch64-none-elf-gcc for kernel; cargo for burner/nullclaw.
#   brew install aarch64-none-elf-gcc   (macOS)
#   sudo apt install gcc-aarch64-linux-gnu  (Linux)

SHELL := /bin/bash
PREFIX   ?= aarch64-none-elf-
CC       := $(PREFIX)gcc
AS       := $(PREFIX)as
LD       := $(PREFIX)ld
OBJCOPY  := $(PREFIX)objcopy

KERNEL_DIR   := kernel
AYEOS_DIR    := ayeos
BURNER_DIR   := burner
NULLCLAW_DIR := nullclaw
BUILD_DIR    := build
DIST_DIR     := dist

KERNEL_ELF   := $(BUILD_DIR)/kernel8.elf
KERNEL_IMG   := $(DIST_DIR)/kernel8.img
AYEOS_ELF    := $(BUILD_DIR)/ayeos.elf
AYEOS_IMG    := $(DIST_DIR)/ayeos.img

CFLAGS   := -Wall -Wextra -O2 -ffreestanding -nostdinc -nostdlib -nostartfiles \
            -march=armv8-a -mtune=cortex-a72 -I$(KERNEL_DIR) -I$(AYEOS_DIR)
ASFLAGS  := -march=armv8-a

KERNEL_SRCS := $(KERNEL_DIR)/arch/aarch64/entry.S \
               $(KERNEL_DIR)/main.c \
               $(KERNEL_DIR)/uart.c \
               $(KERNEL_DIR)/timer.c \
               $(KERNEL_DIR)/mm.c
KERNEL_OBJS := $(patsubst $(KERNEL_DIR)/%,$(BUILD_DIR)/kernel/%,$(KERNEL_SRCS:.S=.o:.c=.o))

AYEOS_SRCS := $(AYEOS_DIR)/main.c \
              $(AYEOS_DIR)/init.c \
              $(AYEOS_DIR)/syscalls.c \
              $(AYEOS_DIR)/fs.c \
              $(AYEOS_DIR)/shell.c
AYEOS_OBJS := $(patsubst $(AYEOS_DIR)/%,$(BUILD_DIR)/ayeos/%,$(AYEOS_SRCS:.c=.o))

KERNEL_LINKER := $(KERNEL_DIR)/linker.ld
AYEOS_LINKER  := $(AYEOS_DIR)/linker.ld

.PHONY: all kernel ayeos burner nullclaw clean dist flash run

all: kernel ayeos burner nullclaw dist

kernel: $(KERNEL_IMG)

ayeos: $(AYEOS_IMG)

burner:
	$(MAKE) -C $(BURNER_DIR)

nullclaw:
	$(MAKE) -C $(NULLCLAW_DIR)

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
	$(LD) -T $(KERNEL_LINKER) -o $@ $(KERNEL_OBJS)

$(KERNEL_IMG): $(KERNEL_ELF)
	@mkdir -p $(DIST_DIR)
	$(OBJCOPY) -O binary $< $@

$(AYEOS_ELF): $(AYEOS_OBJS) $(AYEOS_LINKER)
	$(LD) -T $(AYEOS_LINKER) -o $@ $(AYEOS_OBJS)

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
	@echo "Targets: all kernel ayeos burner nullclaw dist clean flash run"
