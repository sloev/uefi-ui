#!/usr/bin/env bash
# Emit QEMU flags for laptop-friendly input on Linux: host touchpad/trackpoint via evdev when
# readable, else USB tablet. GTK display improves focus/grab; omitted without a GUI session.
#
# ThinkPads often name devices "Synaptics TM…" / "ELAN…" without "touchpad" — we match those.
#
# Env:
#   QEMU_INPUT_USE_TABLET=1     — skip evdev; use virtual USB tablet only (host cursor → guest).
#   QEMU_INPUT_EVDEV=/dev/...   — force this evdev node (must be readable by your user).
#   QEMU_INPUT_GRAB=on|off — append grab= to input-linux (omit by default; unsupported on many builds).
#   QEMU_INPUT_DEBUG=1          — print chosen device to stderr.
set -euo pipefail
shopt -s nullglob

emit_tablet() {
  echo "-device qemu-xhci -device usb-tablet"
}

if [[ "$(uname -s)" != Linux ]]; then
  emit_tablet
  exit 0
fi

if [[ "${QEMU_INPUT_USE_TABLET:-0}" == "1" ]]; then
  emit_tablet
  exit 0
fi

# sysfs name: likely a laptop pointing device (not keyboard / accelerometer / buttons-only).
name_looks_like_pointer() {
  local n_lc="$1"
  [[ "$n_lc" == *keyboard* ]] && return 1
  [[ "$n_lc" == *video* ]] && return 1
  [[ "$n_lc" == *"power button"* ]] && return 1
  [[ "$n_lc" == *"sleep button"* ]] && return 1
  [[ "$n_lc" == *webcam* ]] && return 1
  [[ "$n_lc" == *thinkpad*extra* ]] && return 1
  [[ "$n_lc" == *accelerometer* ]] && return 1
  if [[ "$n_lc" == *touchpad* || "$n_lc" == *trackpad* || "$n_lc" == *clickpad* ]]; then
    return 0
  fi
  if [[ "$n_lc" == *synaptics* || "$n_lc" == *elan* || "$n_lc" == *alps* || "$n_lc" == *glidepoint* ]]; then
    return 0
  fi
  if [[ "$n_lc" == *trackpoint* || "$n_lc" == *"pointing stick"* || "$n_lc" == *dualpoint* ]]; then
    return 0
  fi
  return 1
}

evdev_name() {
  local ev="$1"
  cat "/sys/class/input/$(basename "$ev")/device/name" 2>/dev/null || echo ""
}

find_host_pointer_evdev() {
  local p ev n n_lc

  if [[ -n "${QEMU_INPUT_EVDEV:-}" ]]; then
    if [[ -r "${QEMU_INPUT_EVDEV}" ]]; then
      readlink -f "${QEMU_INPUT_EVDEV}" 2>/dev/null || echo "${QEMU_INPUT_EVDEV}"
      return 0
    fi
    echo "qemu-laptop-input: QEMU_INPUT_EVDEV=${QEMU_INPUT_EVDEV} not readable" >&2
    return 1
  fi

  for p in /dev/input/by-path/*touchpad* /dev/input/by-path/*TouchPad*; do
    [[ -e "$p" ]] || continue
    if [[ -r "$p" ]]; then
      readlink -f "$p"
      return 0
    fi
  done

  # PS/2 touchpad / mouse on many ThinkPads (i8042 serio-1 is often the pad).
  for p in /dev/input/by-path/platform-i8042-serio-1-event-mouse \
           /dev/input/by-path/platform-i8042-serio-1-mouse; do
    [[ -e "$p" ]] || continue
    if [[ -r "$p" ]]; then
      readlink -f "$p"
      return 0
    fi
  done

  for ev in /dev/input/event*; do
    [[ -e "$ev" && -r "$ev" ]] || continue
    n="$(evdev_name "$ev")"
    n_lc="${n,,}"
    if name_looks_like_pointer "$n_lc"; then
      echo "$ev"
      return 0
    fi
  done

  # Last resort: TrackPoint-only (red stick) if nothing else matched.
  for p in /dev/input/by-path/platform-i8042-serio-0-event-mouse \
           /dev/input/by-path/platform-i8042-serio-0-mouse; do
    [[ -e "$p" ]] || continue
    if [[ -r "$p" ]]; then
      readlink -f "$p"
      return 0
    fi
  done

  return 1
}

args=()
if [[ -n "${DISPLAY:-}" || -n "${WAYLAND_DISPLAY:-}" ]]; then
  args+=(-display gtk)
fi

args+=(-device qemu-xhci)

if tp="$(find_host_pointer_evdev)"; then
  [[ "${QEMU_INPUT_DEBUG:-0}" == "1" ]] && echo "qemu-laptop-input: using evdev $(evdev_name "$tp") <$tp>" >&2
  if [[ -n "${QEMU_INPUT_GRAB:-}" ]]; then
    args+=(-object "input-linux,id=hostptr,evdev=${tp},grab=${QEMU_INPUT_GRAB}")
  else
    # Default: no grab= — several QEMU builds error with "Invalid parameter 'grab'".
    args+=(-object "input-linux,id=hostptr,evdev=${tp}")
  fi
else
  [[ "${QEMU_INPUT_DEBUG:-0}" == "1" ]] && echo "qemu-laptop-input: no evdev match — usb-tablet (click QEMU window; use USB mouse if pad fails)" >&2
  args+=(-device usb-tablet)
fi

echo "${args[@]}"
