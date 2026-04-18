#!/usr/bin/env bash
# Build FAT ESP image + ISO wrapping it for Lotus OS UEFI boot.
# Requires: mtools (mcopy, mmd), dosfstools (mkfs.vfat), xorriso.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

UEFI_TARGET="${UEFI_TARGET:-x86_64-unknown-uefi}"
RUSTUP="${RUSTUP:-rustup}"
CARGO="${CARGO:-cargo}"
SIZE_MB="${ISO_ESP_SIZE_MB:-16}"

"$RUSTUP" target add "$UEFI_TARGET"
"$CARGO" build -p lotus-os --target "$UEFI_TARGET" --release

REL="$ROOT/target/$UEFI_TARGET/release"
EFI_SRC=""
if [ -f "$REL/lotus-os.efi" ]; then
  EFI_SRC="$REL/lotus-os.efi"
elif [ -f "$REL/lotus-os" ]; then
  EFI_SRC="$REL/lotus-os"
else
  echo "Missing UEFI binary under $REL" >&2
  exit 1
fi

mkdir -p "$ROOT/target"
ESP_IMG="$ROOT/target/lotus_os_esp.img"
ISO_OUT="${ISO_OUT:-$ROOT/target/lotus-os.iso}"
STAGE="$ROOT/target/lotus_os_iso_stage"

rm -rf "$STAGE"
mkdir -p "$STAGE"

rm -f "$ESP_IMG"
dd if=/dev/zero of="$ESP_IMG" bs=1M count="$SIZE_MB" status=none
mkfs.vfat "$ESP_IMG"

export MTOOLS_SKIP_CHECK=1
mmd -i "$ESP_IMG" ::/EFI 2>/dev/null || true
mmd -i "$ESP_IMG" ::/EFI/BOOT 2>/dev/null || true
mcopy -i "$ESP_IMG" -o "$EFI_SRC" ::/EFI/BOOT/BOOTX64.EFI

# Copy FAT image for El Torito boot and also extract EFI to ISO filesystem
cp "$ESP_IMG" "$STAGE/esp_uefi.img"
# Also copy EFI binary directly to stage for ISO filesystem (some UEFI firmwares look here)
mkdir -p "$STAGE/EFI/BOOT"
copied=0
if [ -f "$REL/lotus-os.efi" ]; then
  cp "$REL/lotus-os.efi" "$STAGE/EFI/BOOT/BOOTX64.EFI"
  copied=1
elif [ -f "$REL/lotus-os" ]; then
  cp "$REL/lotus-os" "$STAGE/EFI/BOOT/BOOTX64.EFI"
  copied=1
fi

(
  cd "$STAGE"
  xorriso -as mkisofs \
    -o "$ISO_OUT" \
    -V LOTUS_OS \
    -e esp_uefi.img \
    -no-emul-boot \
    -eltorito-alt-boot \
    -b esp_uefi.img \
    -no-emul-boot \
    .
)

echo "Wrote $ISO_OUT (ESP image: $ESP_IMG)"
echo "VirtualBox: Create VM with EFI firmware, attach as optical drive"
