# Theming Bedrock UI

The default look is the **Bedrock classic** preset: teal desktop, gray panel surfaces, navy title bar,
and 3D bevels. Everything is driven by three structs:

```rust
use uefi_ui::theme::{Theme, ThemeColors, ThemeSpacing};
use uefi_ui::bedrock::BedrockBevel;

let theme = Theme::bedrock_classic();  // the canonical preset
let bevel = BedrockBevel::CLASSIC;     // matches the preset colors
```

---

## The three pieces

### `ThemeColors`

Holds all semantic color slots. The preset values are the Bedrock classic palette (see Design Manual §3).
Swap any field to change one aspect of the look:

```rust
let mut colors = ThemeColors::bedrock_classic();
colors.accent = Rgb888::new(0x80, 0x00, 0x00);  // crimson title bars
let theme = Theme { colors, ..Theme::bedrock_classic() };
```

### `ThemeSpacing`

Controls corner radius and padding density. `ThemeSpacing::bedrock_sharp()` gives the canonical
zero-radius, tight-padding layout. A modern flat theme might increase `control_pad` or add a small
`corner_radius`.

### `BedrockBevel`

The bevel struct holds the four border shades and draws all raised/sunken chrome:

| Method | Use |
|--------|-----|
| `draw_raised` | Buttons, scrollbar thumbs, window frame |
| `draw_raised_soft` | Softer panels, menu popups |
| `draw_sunken` | Input fields, list containers, gallery panels |
| `draw_raised_pressed` | Active/pressed button state |
| `draw_groupbox` | Etched section frame |
| `draw_status_border` | Status-bar cell |
| `draw_title_bar` | Title bar fill + border |
| `draw_title_button` | Min/Max/Close button chrome |

Construct a custom bevel with any four shades:

```rust
let my_bevel = BedrockBevel {
    face:            Rgb888::new(0xd0, 0xd0, 0xd0),
    border_lightest: Rgb888::WHITE,
    border_light:    Rgb888::new(0xe8, 0xe8, 0xe8),
    border_dark:     Rgb888::new(0x70, 0x70, 0x70),
    border_darkest:  Rgb888::new(0x20, 0x20, 0x20),
};
```

---

## Building a custom theme

1. Start from the preset and override what you need:

```rust
let theme = Theme {
    colors: ThemeColors {
        background: Rgb888::new(0x1e, 0x1e, 0x1e),
        surface:    Rgb888::new(0x2d, 0x2d, 0x2d),
        face:       Rgb888::new(0x2d, 0x2d, 0x2d),
        canvas:     Rgb888::new(0x1e, 0x1e, 0x1e),
        text:       Rgb888::new(0xf0, 0xf0, 0xf0),
        accent:     Rgb888::new(0x00, 0x78, 0xd4),
        ..ThemeColors::bedrock_classic()
    },
    spacing: ThemeSpacing::bedrock_sharp(),
    ..Theme::bedrock_classic()
};
```

2. Pair it with a matching `BedrockBevel`, or write your own chrome functions that take `&ThemeColors`
   directly. The `bedrock_controls` module functions all accept `&BedrockBevel` + `&ThemeColors`
   separately, so either can be swapped independently.

---

## Light / Dark toggle

`Theme::toggle_mode` cycles `Light ↔ Dark`, or switches `BedrockClassic → Light` if you are using
the classic preset and want to leave it. App-level menus can swap a full `Theme` value if you need
more than two modes.

---

## `bedrock_controls` — pre-built chrome

`bedrock_controls` provides ready-made drawing functions for every widget type.
All take a `DrawTarget<Color = Rgb888>` so they work on any surface — UEFI framebuffer,
`embedded-graphics-simulator`, or a PNG canvas.

See the [Design Manual](designmanual.md) for visual reference of each control.
