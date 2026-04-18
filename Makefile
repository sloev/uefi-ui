# uefi_ui — local development
#
# System packages: `make deps` (Debian/Ubuntu; requires sudo), or install ovmf/edk2-ovmf,
# qemu-system-x86, libsdl2-dev via your distro (e.g. pacman -S edk2-ovmf qemu-system-x86 libsdl2).
# For `make iso`: xorriso, mtools, dosfstools (e.g. apt install xorriso mtools dosfstools).
#
# Targets:
#   make sim            — interactive SDL2 window (small Win95-style prototype layout)
#   make linux          — SDL2: full demo (same paint path as uefi_ui_demo / QEMU firmware)
#   make png            — headless PNG export (no SDL2)
#   make qemu           — build UEFI app + run in QEMU with OVMF
#   make iso            — FAT ESP + bootable ISO under target/ (USB / VM)
#   make iso-lotus      — bootable Lotus OS ISO for UEFI
#   make virtualbox     — build uefi_ui_demo ISO + create/start a VirtualBox VM
#   make virtualbox-lotus — build Lotus OS ISO + create/start a VirtualBox VM

.PHONY: help deps sim linux png qemu build-uefi iso iso-lotus virtualbox virtualbox-lotus

CARGO ?= cargo
RUSTUP ?= rustup
QEMU ?= qemu-system-x86_64
# Pointer / keyboard: click the QEMU window so it has focus. Input flags are chosen by
# scripts/qemu-laptop-input.sh (GTK + xhci + host touchpad via evdev when readable, else
# USB tablet). Override the whole bundle with QEMU_INPUT_FLAGS=... or append extras with
# QEMU_EXTRA / QEMU_INPUT_LINUX.
#
# ThinkPad / Linux: ensure /dev/input/event* is readable (e.g. user in "input" group).
# Override evdev: QEMU_INPUT_EVDEV=/dev/input/eventN  |  skip evdev: QEMU_INPUT_USE_TABLET=1
# Debug selection: QEMU_INPUT_DEBUG=1 make qemu
# I2C HID touchpads may need a userspace bridge; see:
#   https://github.com/KBapna/Laptop-Touchpad-Passthrough-For-QEMU/
QEMU_INPUT_FLAGS ?= $(shell bash "$(CURDIR)/scripts/qemu-laptop-input.sh" 2>/dev/null || echo "-device qemu-xhci -device usb-tablet")
QEMU_EXTRA ?=
QEMU_INPUT_LINUX ?=

ESP ?= $(CURDIR)/esp
UEFI_TARGET := x86_64-unknown-uefi
# Writable NVRAM copy for QEMU (do not edit system files under /usr).
OVMF_VARS ?= $(CURDIR)/ovmf_vars.fd

# Use paired CODE+VARS images; distros differ (4M vs legacy names, path).
ifneq ($(wildcard /usr/share/OVMF/OVMF_CODE_4M.fd),)
  OVMF_CODE_SRC := /usr/share/OVMF/OVMF_CODE_4M.fd
  OVMF_VARS_SRC := /usr/share/OVMF/OVMF_VARS_4M.fd
else ifneq ($(wildcard /usr/share/OVMF/OVMF_CODE.fd),)
  OVMF_CODE_SRC := /usr/share/OVMF/OVMF_CODE.fd
  OVMF_VARS_SRC := /usr/share/OVMF/OVMF_VARS.fd
else ifneq ($(wildcard /usr/share/edk2/ovmf/OVMF_CODE.fd),)
  OVMF_CODE_SRC := /usr/share/edk2/ovmf/OVMF_CODE.fd
  OVMF_VARS_SRC := /usr/share/edk2/ovmf/OVMF_VARS.fd
else ifneq ($(wildcard /usr/share/qemu/edk2-x86_64-code.fd),)
  OVMF_CODE_SRC := /usr/share/qemu/edk2-x86_64-code.fd
  OVMF_VARS_SRC := /usr/share/qemu/edk2-x86_64-vars.fd
else
  OVMF_CODE_SRC :=
  OVMF_VARS_SRC :=
endif

OVMF_CODE ?= $(OVMF_CODE_SRC)

help:
	@echo "make deps       Install libsdl2-dev, ovmf, qemu-system-x86 (sudo)"
	@echo "make sim        Live SDL2: small Win95-style prototype (not full demo)"
	@echo "make linux      Live SDL2: full demo (shared paint with uefi_ui_demo / QEMU)"
	@echo "make png        Write target/uefi_ui_prototype.png (no SDL2)"
	@echo "  cargo run -p uefi_ui_prototype -- demo   # same UI as uefi_ui_demo (PNG, no SDL)"
	@echo "make qemu       Build uefi_ui_demo + QEMU with OVMF + FAT ESP (laptop input via scripts/qemu-laptop-input.sh)"
	@echo "make iso        Bootable ISO (target/uefi_ui_demo.iso) — needs xorriso, mtools, dosfstools"
	@echo "make virtualbox Build ISO + VirtualBox VM (VBoxManage) — EFI, USB, usbtablet mouse"
	@echo "  (optional) QEMU_INPUT_FLAGS=... QEMU_EXTRA=... QEMU_INPUT_LINUX=..."
	@echo "  Example: QEMU_EXTRA=-k da   # Danish keyboard layout (host layout differs from guest)"

deps:
	sudo apt-get update && sudo apt-get install -y libsdl2-dev ovmf qemu-system-x86

$(OVMF_VARS):
	@if [ -z "$(OVMF_VARS_SRC)" ]; then \
		echo "No OVMF firmware found under /usr/share/OVMF, /usr/share/edk2/ovmf, or /usr/share/qemu."; \
		echo "Install it (e.g. Debian: apt install ovmf  |  Arch: pacman -S edk2-ovmf) then retry."; \
		exit 1; \
	fi
	cp "$(OVMF_VARS_SRC)" $(OVMF_VARS)

build-uefi:
	$(RUSTUP) target add $(UEFI_TARGET)
	$(CARGO) build -p uefi_ui_test --target $(UEFI_TARGET) --release

qemu: $(OVMF_VARS) build-uefi
	@if [ -z "$(OVMF_CODE)" ]; then \
		echo "OVMF_CODE is empty; install OVMF (see message from ovmf_vars.fd rule)."; \
		exit 1; \
	fi
	mkdir -p $(ESP)/EFI/BOOT
	@rel="$(CURDIR)/target/$(UEFI_TARGET)/release"; \
	if [ -f "$$rel/uefi_ui_demo.efi" ]; then \
		cp "$$rel/uefi_ui_demo.efi" $(ESP)/EFI/BOOT/BOOTX64.EFI; \
	elif [ -f "$$rel/uefi_ui_demo" ]; then \
		cp "$$rel/uefi_ui_demo" $(ESP)/EFI/BOOT/BOOTX64.EFI; \
	else \
		echo "Missing UEFI binary (expected $$rel/uefi_ui_demo.efi — rustc uses .efi for this target)."; \
		exit 1; \
	fi
	$(QEMU) -machine q35 -m 256M -serial stdio $(QEMU_INPUT_FLAGS) $(QEMU_EXTRA) $(QEMU_INPUT_LINUX) \
		-drive if=pflash,format=raw,readonly=on,file=$(OVMF_CODE) \
		-drive if=pflash,format=raw,file=$(OVMF_VARS) \
		-drive file=fat:rw:$(ESP)/,format=raw

build-lotus:
	$(RUSTUP) target add $(UEFI_TARGET)
	$(CARGO) build -p lotus-os --target $(UEFI_TARGET) --release

iso-lotus:
	bash "$(CURDIR)/scripts/build-lotus-iso.sh"

iso:
	bash "$(CURDIR)/scripts/build-efi-iso.sh"

virtualbox:
	bash "$(CURDIR)/scripts/virtualbox-demo.sh"

sim:
	$(CARGO) run -p uefi_ui_prototype --features sdl --release

linux:
	$(CARGO) run -p uefi_ui_prototype --features sdl --release -- demo

png:
	$(CARGO) run -p uefi_ui_prototype --release

virtualbox-lotus: iso-lotus
	@echo "Starting VirtualBox VM with Lotus OS ISO..."
	VBoxManage createvm --name "LotusOS" --ostype "Other" --register 2>/dev/null || true
	VBoxManage modifyvm "LotusOS" --firmware efi --memory 256 --cpus 1 2>/dev/null || true
	VBoxManage modifyvm "LotusOS" --mouse usbtablet 2>/dev/null || true
	VBoxManage storagectl "LotusOS" --name "IDE Controller" --add ide --bootable on 2>/dev/null || true
	VBoxManage storageattach "LotusOS" --storagectl "IDE Controller" --port 0 --device 0 --type dvddrive --medium "$(CURDIR)/target/lotus-os.iso" 2>/dev/null || true
	VBoxManage startvm "LotusOS" --type gui 2>/dev/null || echo "VirtualBox may not be installed or VM already exists"
	echo "If VirtualBox doesn't open automatically, manually create a VM with EFI firmware and attach $(CURDIR)/target/lotus-os.iso"
