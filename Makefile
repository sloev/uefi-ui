# uefi_ui — local development
#
# System packages: `make deps` (Debian/Ubuntu; requires sudo), or install ovmf/edk2-ovmf,
# qemu-system-x86, libsdl2-dev via your distro (e.g. pacman -S edk2-ovmf qemu-system-x86 libsdl2).
# For `make iso`: xorriso, mtools, dosfstools (e.g. apt install xorriso mtools dosfstools).
#
# Targets:
#   make sim      — interactive SDL2 window (small Win95-style prototype layout)
#   make linux    — SDL2: full demo (same paint path as uefi_ui_demo / QEMU firmware)
#   make png      — headless PNG export (no SDL2)
#   make qemu     — build UEFI app + run in QEMU with OVMF
#   make iso      — FAT ESP + bootable ISO under target/ (USB / VM)
#   make virtualbox — build ISO + create/start a VirtualBox VM (EFI, USB, tablet mouse)

.PHONY: help deps sim linux png qemu build-uefi iso virtualbox

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

iso-lotus: build-lotus
	@rel="$(CURDIR)/target/$(UEFI_TARGET)/release"; \
	efi="$$rel/lotus-os.efi"; \
	if [ ! -f "$$efi" ]; then efi="$$rel/lotus-os"; fi; \
	if [ ! -f "$$efi" ]; then echo "Missing lotus-os binary at $$efi"; exit 1; fi; \
	esp_img="$(CURDIR)/target/lotus_os_esp.img"; \
	iso_out="$(CURDIR)/target/lotus-os.iso"; \
	stage="$(CURDIR)/target/lotus_os_iso_stage"; \
	rm -rf "$$stage" && mkdir -p "$$stage"; \
	rm -f "$$esp_img"; \
	dd if=/dev/zero of="$$esp_img" bs=1M count=16 status=none; \
	mkfs.vfat "$$esp_img"; \
	MTOOLS_SKIP_CHECK=1 mmd -i "$$esp_img" ::/EFI 2>/dev/null || true; \
	MTOOLS_SKIP_CHECK=1 mmd -i "$$esp_img" ::/EFI/BOOT 2>/dev/null || true; \
	MTOOLS_SKIP_CHECK=1 mcopy -i "$$esp_img" -o "$$efi" ::/EFI/BOOT/BOOTX64.EFI; \
	cp "$$esp_img" "$$stage/esp_uefi.img"; \
	(cd "$$stage" && xorriso -as mkisofs -o "$$iso_out" -V LOTUS_OS -e esp_uefi.img -no-emul-boot .); \
	echo "Wrote $$iso_out"

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
