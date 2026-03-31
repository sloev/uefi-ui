//! **Fast UI iteration on Linux**: writes `target/uefi_ui_prototype.png` (or path argument).
//!
//! - Default: **Bedrock** teal desktop, gray dialog, 3D bevels.
//! - `cargo run -p uefi_ui_prototype -- demo` — **same paint path** as the firmware app ([`uefi_ui_demo::scene::paint_demo_snapshot`]).

use embedded_graphics::pixelcolor::Rgb888;
use embedded_graphics::Pixel;
use embedded_graphics::prelude::*;
use embedded_graphics::mono_font::ascii::FONT_6X10;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::primitives::{Line, PrimitiveStyle, PrimitiveStyleBuilder, Rectangle};
use embedded_graphics::text::{Baseline, Text};
use embedded_graphics_simulator::{OutputSettingsBuilder, SimulatorDisplay};
#[cfg(feature = "sdl")]
use embedded_graphics_simulator::Window;

use std::env;

use uefi_ui::layout::{pad, row_panels, row_panels_fit_start};
use uefi_ui::popover::{center_in_screen, place_below_anchor};
use uefi_ui::bedrock::BedrockBevel;
use uefi_ui::theme::Theme;
use uefi_ui::framebuffer::BgrxFramebuffer;
use uefi_ui::widgets::LineGraph;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let demo_mode = args.iter().any(|s| s == "demo" || s == "--demo");
    let args: Vec<String> = args
        .into_iter()
        .filter(|s| s != "demo" && s != "--demo")
        .collect();
    #[cfg(not(feature = "sdl"))]
    let path = args
        .get(0)
        .cloned()
        .unwrap_or_else(|| "target/uefi_ui_prototype.png".into());

    let mut display = SimulatorDisplay::<Rgb888>::new(Size::new(1024, 768));
    let screen = Rectangle::new(Point::zero(), display.size());

    if demo_mode {
        paint_uefi_demo_like_frame(&mut display);
    } else {
        draw_bedrock_desktop(&mut display, screen);
    }

    let output_settings = OutputSettingsBuilder::new().scale(2).build();

    #[cfg(feature = "sdl")]
    {
        let title = if demo_mode {
            "uefi_ui prototype — UEFI demo (shared paint, close to quit)"
        } else {
            "uefi_ui prototype — Bedrock (close window to quit)"
        };
        Window::new(title, &output_settings).show_static(&display);
        return;
    }

    #[cfg(not(feature = "sdl"))]
    {
        let img = display.to_rgb_output_image(&output_settings);
        img.save_png(&path).expect("save png");
        eprintln!(
            "uefi_ui_prototype: wrote {} ({})",
            path,
            if demo_mode {
                "UEFI demo (shared lib paint)"
            } else {
                "Bedrock"
            }
        );
    }
}

/// Renders [`uefi_ui_demo::scene::paint_demo_snapshot`] into a BGRX buffer, then blits RGB to the simulator (same scene as the `.efi` demo, without UEFI I/O).
fn paint_uefi_demo_like_frame(display: &mut SimulatorDisplay<Rgb888>) {
    let w = display.size().width;
    let h = display.size().height;
    let stride = w as usize * 4;
    let mut buf = vec![0u8; stride * h as usize];
    uefi_ui_demo::scene::paint_demo_snapshot(&mut buf, w, h, stride, None).expect("paint_demo_snapshot");
    let fb = BgrxFramebuffer::new(&mut buf, w, h, stride).expect("framebuffer");
    for y in 0..h {
        for x in 0..w {
            let c = fb.pixel_at(x, y).unwrap_or(Rgb888::BLACK);
            let _ = Pixel(Point::new(x as i32, y as i32), c).draw(display);
        }
    }
}

fn draw_bedrock_desktop<D: DrawTarget<Color = Rgb888> + OriginDimensions>(d: &mut D, screen: Rectangle) {
    let theme = Theme::bedrock_classic();
    let bevel = BedrockBevel::CLASSIC;

    clear(d, theme.colors.background);

    // --- Window chrome (drawn first — bottommost layer) ---
    let dialog_outer = center_in_screen(Size::new(420, 300), screen);
    bevel.draw_raised_soft(d, dialog_outer).ok();
    let dialog = pad(dialog_outer, BedrockBevel::BEVEL_PX);

    const TITLE_H: u32 = 22;
    const MENU_H: u32 = 22;
    let title_bar = Rectangle::new(dialog.top_left, Size::new(dialog.size.width, TITLE_H));
    fill_rect(d, title_bar, theme.colors.accent);
    let title_style = MonoTextStyle::new(&FONT_6X10, theme.colors.caption_on_accent);
    let _ = Text::with_baseline(
        "Bedrock Demo",
        Point::new(title_bar.top_left.x + 8, title_bar.top_left.y + 6),
        title_style,
        Baseline::Top,
    )
    .draw(d);

    // --- Menu bar ---
    let menu_strip = Rectangle::new(
        Point::new(dialog.top_left.x, dialog.top_left.y + TITLE_H as i32),
        Size::new(dialog.size.width, MENU_H),
    );
    fill_rect(d, menu_strip, theme.colors.surface);
    let menu_widths = [40u32, 40, 44, 44];
    let menu_cells = row_panels_fit_start(pad(menu_strip, 2), &menu_widths, 4);
    let menu_labels = ["File", "Edit", "View", "Help"];
    for (i, cell) in menu_cells.iter().enumerate() {
        let sel = i == 0;
        let bg = if sel { theme.colors.accent } else { theme.colors.surface };
        let fg = if sel { theme.colors.caption_on_accent } else { theme.colors.text };
        fill_rect(d, *cell, bg);
        let style = MonoTextStyle::new(&FONT_6X10, fg);
        let _ = Text::with_baseline(
            menu_labels[i],
            Point::new(cell.top_left.x + 4, cell.top_left.y + 5),
            style,
            Baseline::Top,
        )
        .draw(d);
    }

    // --- Body panels ---
    let body = Rectangle::new(
        Point::new(dialog.top_left.x, dialog.top_left.y + TITLE_H as i32 + MENU_H as i32),
        Size::new(dialog.size.width, dialog.size.height.saturating_sub(TITLE_H + MENU_H)),
    );
    let inner = pad(body, theme.spacing.sm);
    let main_split = row_panels(inner, 2, theme.spacing.sm);
    if main_split.len() == 2 {
        // Left: sunken widget-strip placeholder
        bevel.draw_sunken(d, main_split[0]).ok();
        let lbl_style = MonoTextStyle::new(&FONT_6X10, theme.colors.text);
        let _ = Text::with_baseline(
            "Widget strip",
            Point::new(
                main_split[0].top_left.x + BedrockBevel::BEVEL_PX as i32 + 4,
                main_split[0].top_left.y + BedrockBevel::BEVEL_PX as i32 + 4,
            ),
            lbl_style,
            Baseline::Top,
        )
        .draw(d);

        // Right: sunken graph panel
        bevel.draw_sunken(d, main_split[1]).ok();
        let plot_area = pad(main_split[1], 4);
        let mut graph = LineGraph::new(48);
        for x in 0..48 {
            graph.push((x as f32 * 0.15).sin() * 12.0 + 16.0);
        }
        for w in graph.points(plot_area).windows(2) {
            Line::new(w[0], w[1])
                .into_styled(PrimitiveStyle::with_stroke(theme.colors.graph_line, 1))
                .draw(d)
                .ok();
        }
    }

    // --- Menu popup LAST so it renders above body panels ---
    let pop = place_below_anchor(menu_cells[0], Size::new(110, 28), screen);
    bevel.draw_raised_soft(d, pop).ok();
    fill_rect(d, pad(pop, BedrockBevel::BEVEL_PX), theme.colors.canvas);
    let pop_style = MonoTextStyle::new(&FONT_6X10, theme.colors.text);
    let _ = Text::with_baseline(
        "New file…",
        Point::new(pop.top_left.x + 6, pop.top_left.y + 6),
        pop_style,
        Baseline::Top,
    )
    .draw(d);
}

fn clear<D: DrawTarget<Color = Rgb888> + OriginDimensions>(d: &mut D, c: Rgb888) {
    Rectangle::new(Point::zero(), d.size())
        .into_styled(PrimitiveStyleBuilder::new().fill_color(c).build())
        .draw(d)
        .ok();
}

fn fill_rect<D: DrawTarget<Color = Rgb888>>(d: &mut D, r: Rectangle, c: Rgb888) {
    r.into_styled(PrimitiveStyleBuilder::new().fill_color(c).build())
        .draw(d)
        .ok();
}

