# uefi_ui

A `no_std + alloc` immediate-mode UI library targeting UEFI firmware, with a pixel-perfect
Bedrock-style visual system (3D bevels, teal desktop, navy title bars).

---

## What it is

- **`uefi_ui`** — widget state, layout, keyboard/mouse abstractions, Bedrock chrome helpers.
  Runs on bare UEFI with no operating system underneath.
- **`uefi_ui_demo`** — interactive UEFI application: widget gallery + text editor.
- **`uefi_ui_prototype`** — Linux-hosted simulator for fast visual iteration (PNG output, optional SDL2 window).

---

## Build & Run

### Static screenshots (no dependencies)

```sh
cargo run -p uefi_ui_prototype --bin showcase
# Writes docs/screenshots/*.png and docs/showcase.md
```

### Live SDL2 window (requires `libsdl2-dev`)

```sh
cargo run -p uefi_ui_prototype --features sdl        # widget gallery
cargo run -p uefi_ui_prototype --bin editor --features sdl  # text editor
```

### UEFI firmware image

```sh
make build   # requires nightly + cargo-make + OVMF
make run     # boots in QEMU
```

---

## Documentation

| Document | Contents |
|----------|---------|
| [Design Manual](designmanual.md) | Visual language, color palette, spacing rules, full widget gallery with screenshots |
| [Theming Guide](THEMING.md) | How to use `Theme`, `BedrockBevel`, and `bedrock_controls` to build or customize the look |
| [`../tasks.md`](../tasks.md) | Open design and implementation tasks |
| [`../SPEC.md`](../SPEC.md) | Technical specification and feature contracts |

---

## Screenshots

![bevel styles](screenshots/bevel_styles.png)
![buttons](screenshots/buttons.png)
![editor text](screenshots/editor_text.png)
