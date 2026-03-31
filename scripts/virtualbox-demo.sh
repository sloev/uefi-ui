#!/usr/bin/env bash
# Boot uefi_ui_demo in VirtualBox (EFI + optical ISO). Requires: VirtualBox, xorriso/mtools for iso.
# Env: VBOX_VM_NAME (default uefi_ui_demo), VBOX_ISO (default target/uefi_ui_demo.iso)
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VM="${VBOX_VM_NAME:-uefi_ui_demo}"
ISO="${VBOX_ISO:-$ROOT/target/uefi_ui_demo.iso}"

if ! command -v VBoxManage >/dev/null 2>&1; then
	echo "VBoxManage not found. Install VirtualBox and ensure VBoxManage is in PATH." >&2
	exit 1
fi

if [ ! -f "$ISO" ]; then
	echo "ISO not found: $ISO — building..."
	bash "$ROOT/scripts/build-efi-iso.sh"
fi

if ! VBoxManage list vms | grep -Fq "\"$VM\""; then
	echo "Creating VM \"$VM\" (EFI, USB HID-friendly pointing device)..."
	VBoxManage createvm --name "$VM" --register --ostype Other_64
	VBoxManage modifyvm "$VM" --memory 256 --vram 32 --cpus 1
	# EFI system partition boot (VBox 6.1+)
	if ! VBoxManage modifyvm "$VM" --firmware efi 2>/dev/null; then
		echo "Note: --firmware efi not supported; enable UEFI manually in VM settings if boot fails." >&2
	fi
	VBoxManage modifyvm "$VM" --graphicscontroller vmsvga
	VBoxManage modifyvm "$VM" --usbohci on --usbehci on --usbxhci on
	# Prefer USB tablet integration (falls back if unknown option)
	if ! VBoxManage modifyvm "$VM" --mouse usbtablet 2>/dev/null; then
		VBoxManage modifyvm "$VM" --mouse usb 2>/dev/null || true
	fi
	VBoxManage modifyvm "$VM" --boot1 dvd --boot2 none --boot3 none --boot4 none
	VBoxManage storagectl "$VM" --name IDE --add ide
	VBoxManage storageattach "$VM" --storagectl IDE --port 0 --device 0 --type dvddrive --medium "$ISO"
else
	echo "VM \"$VM\" exists — attaching ISO and starting..."
	if ! VBoxManage storageattach "$VM" --storagectl IDE --port 0 --device 0 --type dvddrive --medium "$ISO" 2>/dev/null; then
		VBoxManage storagectl "$VM" --name IDE --add ide 2>/dev/null || true
		VBoxManage storageattach "$VM" --storagectl IDE --port 0 --device 0 --type dvddrive --medium "$ISO"
	fi
fi

echo "Starting \"$VM\" (close the window or power off from the guest)..."
echo ""
echo "ThinkPad / laptop touchpad: click inside the VM window to capture the pointer."
echo "If the cursor does not move: VM settings → System → Pointing Device: USB Tablet;"
echo "  or try USB → add USB filter for a physical USB mouse; disable \"Mouse integration\" test."
echo ""
exec VBoxManage startvm "$VM" --type gui
