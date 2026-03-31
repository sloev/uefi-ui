#!/usr/bin/env bash
# Build FAT ESP image + ISO wrapping it for UEFI (copy to USB or burn).
# Requires: mtools (mcopy, mmd), dosfstools (mkfs.vfat), xorriso.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

UEFI_TARGET="${UEFI_TARGET:-x86_64-unknown-uefi}"
RUSTUP="${RUSTUP:-rustup}"
CARGO="${CARGO:-cargo}"
# Stay under El Torito 65535 × 512 B when embedding the image as `-e` (avoid xorriso warnings).
SIZE_MB="${ISO_ESP_SIZE_MB:-16}"

"$RUSTUP" target add "$UEFI_TARGET"
"$CARGO" build -p uefi_ui_demo --target "$UEFI_TARGET" --release

REL="$ROOT/target/$UEFI_TARGET/release"
EFI_SRC=""
if [ -f "$REL/uefi_ui_demo.efi" ]; then
  EFI_SRC="$REL/uefi_ui_demo.efi"
elif [ -f "$REL/uefi_ui_demo" ]; then
  EFI_SRC="$REL/uefi_ui_demo"
else
  echo "Missing UEFI binary under $REL" >&2
  exit 1
fi

mkdir -p "$ROOT/target"
ESP_IMG="$ROOT/target/esp_uefi.img"
ISO_OUT="${ISO_OUT:-$ROOT/target/uefi_ui_demo.iso}"
STAGE="$ROOT/target/iso_esp_staging"

rm -rf "$STAGE"
mkdir -p "$STAGE"

rm -f "$ESP_IMG"
dd if=/dev/zero of="$ESP_IMG" bs=1M count="$SIZE_MB" status=none
mkfs.vfat "$ESP_IMG"

export MTOOLS_SKIP_CHECK=1
mmd -i "$ESP_IMG" ::/EFI 2>/dev/null || true
mmd -i "$ESP_IMG" ::/EFI/BOOT 2>/dev/null || true
mcopy -i "$ESP_IMG" -o "$EFI_SRC" ::/EFI/BOOT/BOOTX64.EFI

# Only the FAT image belongs in the ISO tree (do not use `target/` as mkisofs root).
cp "$ESP_IMG" "$STAGE/esp_uefi.img"
(
  cd "$STAGE"
  xorriso -as mkisofs \
    -o "$ISO_OUT" \
    -V UEFI_UI_DEMO \
    -e esp_uefi.img \
    -no-emul-boot \
    .
)

echo "Wrote $ISO_OUT (ESP image: $ESP_IMG)"
echo "USB: write the ISO with dd, xorriso, or a GUI tool; or dd the FAT image $ESP_IMG directly to a partition."
