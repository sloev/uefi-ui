//! Global **theme**: light/dark palettes, **Bedrock** preset (default “skin” for
//! this project), font sizes.
//!
//! Use [`crate::bedrock::BedrockBevel`] with [`Theme::bedrock_classic`] for 3D chrome. Custom palettes: see
//! **Theming** in the repo `docs/THEMING.md`. Drawing uses your [`fontdue`](crate::font) handle;
//! sizes here are hints for rasterization.

use embedded_graphics::pixelcolor::Rgb888;

/// Visual style preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Light,
    Dark,
    /// Teal desktop + gray `COLOR_3DFACE` chrome (Bedrock classic style).
    BedrockClassic,
}

/// Semantic palette (RGB888).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeColors {
    pub background: Rgb888,
    /// Panel / button face (`material` in Bedrock).
    pub surface: Rgb888,
    pub surface_alt: Rgb888,
    /// Input field / canvas white background.
    pub canvas: Rgb888,
    /// Menu / list item hover + text selection background.
    pub selection_bg: Rgb888,
    pub text: Rgb888,
    pub text_secondary: Rgb888,
    pub text_disabled: Rgb888,
    /// Text on saturated UI (active title bar, primary buttons).
    pub caption_on_accent: Rgb888,
    /// Active title bar / accent fill.
    pub accent: Rgb888,
    pub accent_hover: Rgb888,
    /// Inactive title bar background.
    pub header_inactive_bg: Rgb888,
    /// Inactive title bar text.
    pub header_inactive_text: Rgb888,
    pub border: Rgb888,
    pub border_focus: Rgb888,
    pub danger: Rgb888,
    pub success: Rgb888,
    pub overlay: Rgb888,
    pub popover_bg: Rgb888,
    /// Tooltip background (cream yellow).
    pub tooltip_bg: Rgb888,
    /// Secondary focus ring color (yellow, used for Select focused state).
    pub focus_secondary: Rgb888,
    pub graph_line: Rgb888,
    pub graph_fill: Rgb888,
    pub progress_track: Rgb888,
    pub progress_fill: Rgb888,
}

/// Font size hints (logical px).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeFonts {
    pub title: f32,
    pub body: f32,
    pub small: f32,
    pub mono: f32,
    pub icon: f32,
}

/// Full theme.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub mode: ThemeMode,
    pub colors: ThemeColors,
    pub fonts: ThemeFonts,
    pub spacing: ThemeSpacing,
}

/// Gaps / radii (px). Retro themes use **0** radius (sharp corners).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeSpacing {
    pub xs: u32,
    pub sm: u32,
    pub md: u32,
    pub lg: u32,
    pub radius_sm: u32,
    pub radius_md: u32,
}

impl ThemeSpacing {
    pub const fn default_theme() -> Self {
        Self {
            xs: 4,
            sm: 8,
            md: 12,
            lg: 20,
            radius_sm: 4,
            radius_md: 8,
        }
    }

    /// Sharp corners, tight padding (Bedrock-era UI density).
    pub const fn bedrock_sharp() -> Self {
        Self {
            xs: 2,
            sm: 4,
            md: 8,
            lg: 12,
            radius_sm: 0,
            radius_md: 0,
        }
    }
}

impl Theme {
    pub fn light() -> Self {
        Self {
            mode: ThemeMode::Light,
            colors: ThemeColors {
                background: Rgb888::new(0xf5, 0xf5, 0xf7),
                surface: Rgb888::new(0xff, 0xff, 0xff),
                surface_alt: Rgb888::new(0xee, 0xee, 0xf0),
                canvas: Rgb888::new(0xff, 0xff, 0xff),
                selection_bg: Rgb888::new(0xb4, 0xd5, 0xfe),
                text: Rgb888::new(0x1a, 0x1a, 0x1e),
                text_secondary: Rgb888::new(0x55, 0x55, 0x5f),
                text_disabled: Rgb888::new(0xa8, 0xa8, 0xb0),
                caption_on_accent: Rgb888::new(0xff, 0xff, 0xff),
                accent: Rgb888::new(0x25, 0x63, 0xeb),
                accent_hover: Rgb888::new(0x1d, 0x4e, 0xd8),
                header_inactive_bg: Rgb888::new(0x9e, 0x9e, 0x9e),
                header_inactive_text: Rgb888::new(0xe0, 0xe0, 0xe0),
                border: Rgb888::new(0xd1, 0xd5, 0xdb),
                border_focus: Rgb888::new(0x25, 0x63, 0xeb),
                danger: Rgb888::new(0xdc, 0x26, 0x26),
                success: Rgb888::new(0x16, 0xa3, 0x4a),
                overlay: Rgb888::new(0x00, 0x00, 0x00),
                popover_bg: Rgb888::new(0xff, 0xff, 0xff),
                tooltip_bg: Rgb888::new(0xff, 0xff, 0xe1),
                focus_secondary: Rgb888::new(0x25, 0x63, 0xeb),
                graph_line: Rgb888::new(0x25, 0x63, 0xeb),
                graph_fill: Rgb888::new(0xbf, 0xd4, 0xfe),
                progress_track: Rgb888::new(0xe5, 0xe7, 0xeb),
                progress_fill: Rgb888::new(0x25, 0x63, 0xeb),
            },
            fonts: ThemeFonts {
                title: 22.0,
                body: 16.0,
                small: 13.0,
                mono: 14.0,
                icon: 18.0,
            },
            spacing: ThemeSpacing::default_theme(),
        }
    }

    pub fn dark() -> Self {
        Self {
            mode: ThemeMode::Dark,
            colors: ThemeColors {
                background: Rgb888::new(0x12, 0x12, 0x16),
                surface: Rgb888::new(0x1e, 0x1e, 0x24),
                surface_alt: Rgb888::new(0x2a, 0x2a, 0x32),
                canvas: Rgb888::new(0x1e, 0x1e, 0x24),
                selection_bg: Rgb888::new(0x3d, 0x5a, 0x8c),
                text: Rgb888::new(0xf4, 0xf4, 0xf8),
                text_secondary: Rgb888::new(0xa1, 0xa1, 0xac),
                text_disabled: Rgb888::new(0x60, 0x60, 0x6c),
                caption_on_accent: Rgb888::new(0xff, 0xff, 0xff),
                accent: Rgb888::new(0x60, 0xa5, 0xfa),
                accent_hover: Rgb888::new(0x7c, 0xb8, 0xfc),
                header_inactive_bg: Rgb888::new(0x3f, 0x3f, 0x4a),
                header_inactive_text: Rgb888::new(0xa1, 0xa1, 0xac),
                border: Rgb888::new(0x3f, 0x3f, 0x4a),
                border_focus: Rgb888::new(0x60, 0xa5, 0xfa),
                danger: Rgb888::new(0xf8, 0x71, 0x71),
                success: Rgb888::new(0x4a, 0xde, 0x80),
                overlay: Rgb888::new(0x00, 0x00, 0x00),
                popover_bg: Rgb888::new(0x27, 0x27, 0x30),
                tooltip_bg: Rgb888::new(0x27, 0x27, 0x30),
                focus_secondary: Rgb888::new(0x60, 0xa5, 0xfa),
                graph_line: Rgb888::new(0x60, 0xa5, 0xfa),
                graph_fill: Rgb888::new(0x37, 0x4f, 0x7a),
                progress_track: Rgb888::new(0x3f, 0x3f, 0x4a),
                progress_fill: Rgb888::new(0x60, 0xa5, 0xfa),
            },
            fonts: ThemeFonts {
                title: 22.0,
                body: 16.0,
                small: 13.0,
                mono: 14.0,
                icon: 18.0,
            },
            spacing: ThemeSpacing::default_theme(),
        }
    }

    /// Bedrock classic style: default theme colors.
    pub fn bedrock_classic() -> Self {
        Self {
            mode: ThemeMode::BedrockClassic,
            colors: ThemeColors {
                // desktopBackground
                background: Rgb888::new(0x00, 0x80, 0x80),
                // material (#c6c6c6)
                surface: Rgb888::new(0xc6, 0xc6, 0xc6),
                // materialDark (#9a9e9c)
                surface_alt: Rgb888::new(0x9a, 0x9e, 0x9c),
                // canvas (#ffffff) — input field interior
                canvas: Rgb888::new(0xff, 0xff, 0xff),
                // hoverBackground (#060084) — menu/list highlight
                selection_bg: Rgb888::new(0x06, 0x00, 0x84),
                // canvasText (#0a0a0a)
                text: Rgb888::new(0x0a, 0x0a, 0x0a),
                text_secondary: Rgb888::new(0x40, 0x40, 0x40),
                // canvasTextDisabled (#848584)
                text_disabled: Rgb888::new(0x84, 0x85, 0x84),
                // headerText (#fefefe)
                caption_on_accent: Rgb888::new(0xfe, 0xfe, 0xfe),
                // headerBackground (#060084)
                accent: Rgb888::new(0x06, 0x00, 0x84),
                accent_hover: Rgb888::new(0x06, 0x00, 0x84),
                // headerNotActiveBackground (#7f787f)
                header_inactive_bg: Rgb888::new(0x7f, 0x78, 0x7f),
                // headerNotActiveText (#c6c6c6)
                header_inactive_text: Rgb888::new(0xc6, 0xc6, 0xc6),
                // borderDark (#848584)
                border: Rgb888::new(0x84, 0x85, 0x84),
                // borderDarkest (#0a0a0a)
                border_focus: Rgb888::new(0x0a, 0x0a, 0x0a),
                danger: Rgb888::new(0xc0, 0x00, 0x00),
                success: Rgb888::new(0x00, 0x80, 0x00),
                overlay: Rgb888::new(0x00, 0x00, 0x00),
                // popover background = material
                popover_bg: Rgb888::new(0xc6, 0xc6, 0xc6),
                // tooltip (#fefbcc)
                tooltip_bg: Rgb888::new(0xfe, 0xfb, 0xcc),
                // focusSecondary (#fefe03)
                focus_secondary: Rgb888::new(0xfe, 0xfe, 0x03),
                // graph uses navy
                graph_line: Rgb888::new(0x06, 0x00, 0x84),
                graph_fill: Rgb888::new(0x80, 0x80, 0xc0),
                // progress track = canvas white, fill = progress blue
                progress_track: Rgb888::new(0xff, 0xff, 0xff),
                // progress (#060084)
                progress_fill: Rgb888::new(0x06, 0x00, 0x84),
            },
            fonts: ThemeFonts {
                title: 18.0,
                body: 13.0,
                small: 11.0,
                mono: 12.0,
                icon: 14.0,
            },
            spacing: ThemeSpacing::bedrock_sharp(),
        }
    }

    pub fn toggle_mode(&mut self) {
        match self.mode {
            ThemeMode::Light => *self = Self::dark(),
            ThemeMode::Dark => *self = Self::light(),
            ThemeMode::BedrockClassic => *self = Self::light(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_light_differ() {
        assert_ne!(Theme::dark().colors.background, Theme::light().colors.background);
    }

    #[test]
    fn bedrock_classic_teal_desktop() {
        let t = Theme::bedrock_classic();
        assert_eq!(t.colors.background, Rgb888::new(0x00, 0x80, 0x80));
    }
}
