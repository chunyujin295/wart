//! Visual styling: two palettes and the shared spacing scale.
//!
//! egui's stock style is tuned for dense desktop tools — 12px text, tight
//! padding, small hit targets. For a tool that is mostly typing and dragging
//! sliders, that reads as cramped and fiddly, so everything is scaled up.
//!
//! Colour is not hardcoded at the point of use. Sections ask the palette for a
//! colour by role (`heading`, `weak`, `warn`), so adding a second palette cannot
//! leave stray hardcoded values behind in one of them.

use eframe::egui;
use egui::{Color32, CornerRadius, FontFamily, FontId, TextStyle};

/// Size of the bundled interface font's design grid.
///
/// It is a pixel font drawn on a 12px grid — 1200 units per em, 100 units to
/// the pixel. Every interface size is a whole multiple of this; anything in
/// between lands the outlines off the grid and comes out blurred.
pub const PIXEL: f32 = 12.0;

/// Body text.
pub const UI_BODY: f32 = PIXEL;
/// Section headings. The font ships one weight per script, so hierarchy has to
/// come from colour and placement rather than from size or boldness.
pub const UI_SECTION: f32 = PIXEL;
/// The application title.
pub const UI_TITLE: f32 = PIXEL * 2.0;

/// The artwork is drawn in the Nerd Font, not the interface font, so its size
/// is unrelated to the pixel grid.
pub const UI_ART: f32 = 16.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Palette {
    #[default]
    Dark,
    Light,
}

impl Palette {
    pub const ALL: [Palette; 2] = [Palette::Dark, Palette::Light];

    pub fn is_dark(self) -> bool {
        matches!(self, Palette::Dark)
    }

    fn colors(self) -> Colors {
        match self {
            Palette::Dark => Colors {
                bg: Color32::from_rgb(0x14, 0x16, 0x1b),
                panel: Color32::from_rgb(0x1b, 0x1e, 0x25),
                widget: Color32::from_rgb(0x26, 0x2a, 0x33),
                hover: Color32::from_rgb(0x33, 0x38, 0x43),
                accent: Color32::from_rgb(0x5f, 0xa8, 0xff),
                // Deliberately not grey-on-grey: the previous palette made
                // section headings and hint text genuinely hard to read.
                text: Color32::from_rgb(0xe6, 0xea, 0xf2),
                weak: Color32::from_rgb(0x9d, 0xa7, 0xb8),
                heading: Color32::from_rgb(0xf5, 0xf7, 0xfb),
                border: Color32::from_rgb(0x33, 0x39, 0x45),
                on_accent: Color32::from_rgb(0x0b, 0x0e, 0x14),
            },
            Palette::Light => Colors {
                bg: Color32::from_rgb(0xf4, 0xf5, 0xf7),
                panel: Color32::from_rgb(0xff, 0xff, 0xff),
                widget: Color32::from_rgb(0xe9, 0xeb, 0xef),
                hover: Color32::from_rgb(0xdc, 0xdf, 0xe5),
                accent: Color32::from_rgb(0x1f, 0x6f, 0xeb),
                text: Color32::from_rgb(0x1f, 0x23, 0x28),
                weak: Color32::from_rgb(0x5a, 0x63, 0x72),
                heading: Color32::from_rgb(0x0d, 0x11, 0x17),
                border: Color32::from_rgb(0xd0, 0xd4, 0xda),
                on_accent: Color32::from_rgb(0xff, 0xff, 0xff),
            },
        }
    }
}

struct Colors {
    bg: Color32,
    panel: Color32,
    widget: Color32,
    hover: Color32,
    accent: Color32,
    text: Color32,
    weak: Color32,
    heading: Color32,
    border: Color32,
    on_accent: Color32,
}

/// Semantic colours for the current palette, so widgets never hardcode a shade.
#[derive(Clone, Copy)]
pub struct Roles {
    pub weak: Color32,
    pub heading: Color32,
    pub notice: Color32,
    pub warn: Color32,
    pub error: Color32,
}

impl Palette {
    pub fn roles(self) -> Roles {
        let c = self.colors();
        let (notice, warn, error) = if self.is_dark() {
            (
                Color32::from_rgb(0x7d, 0xd1, 0x8a),
                Color32::from_rgb(0xe3, 0xb3, 0x4d),
                Color32::from_rgb(0xff, 0x7b, 0x72),
            )
        } else {
            (
                Color32::from_rgb(0x11, 0x6b, 0x2e),
                Color32::from_rgb(0x9a, 0x67, 0x00),
                Color32::from_rgb(0xc0, 0x2b, 0x22),
            )
        };
        Roles { weak: c.weak, heading: c.heading, notice, warn, error }
    }
}

/// The palette the operating system asks for, falling back to dark.
pub fn system_default(ctx: &egui::Context) -> Palette {
    match ctx.system_theme() {
        Some(egui::Theme::Light) => Palette::Light,
        _ => Palette::Dark,
    }
}

pub fn apply(ctx: &egui::Context, palette: Palette) {
    let c = palette.colors();
    let mut style = (*ctx.style_of(egui::Theme::Dark)).clone();

    // --- typography --------------------------------------------------------
    style.text_styles = [
        (TextStyle::Heading, FontId::new(UI_TITLE, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(UI_BODY, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(UI_BODY, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(UI_BODY, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(UI_ART, FontFamily::Monospace)),
    ]
    .into();

    // --- spacing -----------------------------------------------------------
    let s = &mut style.spacing;
    // Padding inside every button; the stock 4x1 is what makes them look small
    // regardless of the font size.
    s.button_padding = egui::vec2(13.0, 8.0);
    s.item_spacing = egui::vec2(10.0, 9.0);
    s.window_margin = egui::Margin::same(14);
    s.indent = 20.0;
    s.slider_width = 150.0;
    s.combo_width = 170.0;
    // Minimum hit target. Raised again after feedback that the six-icon rows
    // were easy to misclick.
    s.interact_size = egui::vec2(46.0, 28.0);
    s.scroll = egui::style::ScrollStyle::solid();
    s.scroll.bar_width = 11.0;
    s.scroll.floating = false;

    // --- visuals -----------------------------------------------------------
    let v = &mut style.visuals;
    v.dark_mode = palette.is_dark();
    v.panel_fill = c.panel;
    v.window_fill = c.panel;
    v.extreme_bg_color = c.bg;
    v.faint_bg_color = c.widget;
    v.override_text_color = Some(c.text);
    // `ui.weak()` goes through this rather than `override_text_color`; left at
    // its default it renders nearly invisible on both palettes.
    v.weak_text_color = Some(c.weak);
    v.weak_text_alpha = 1.0;

    let radius = CornerRadius::same(7);

    v.widgets.noninteractive.bg_fill = c.panel;
    v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, c.border);
    v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, c.weak);
    v.widgets.noninteractive.corner_radius = radius;

    v.widgets.inactive.bg_fill = c.widget;
    v.widgets.inactive.weak_bg_fill = c.widget;
    v.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, c.text);
    v.widgets.inactive.corner_radius = radius;

    v.widgets.hovered.bg_fill = c.hover;
    v.widgets.hovered.weak_bg_fill = c.hover;
    v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, c.accent);
    v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, c.heading);
    v.widgets.hovered.corner_radius = radius;

    // A selected segment (mode, charset, colour style) reads as an accent fill,
    // not just a slightly lighter grey.
    v.widgets.active.bg_fill = c.accent;
    v.widgets.active.weak_bg_fill = c.accent;
    v.widgets.active.bg_stroke = egui::Stroke::NONE;
    v.widgets.active.fg_stroke = egui::Stroke::new(1.0, c.on_accent);
    v.widgets.active.corner_radius = radius;

    v.selection.bg_fill = c.accent.gamma_multiply(0.45);
    v.selection.stroke = egui::Stroke::new(1.0, c.text);
    v.window_corner_radius = CornerRadius::same(10);
    v.window_shadow = egui::epaint::Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: Color32::from_black_alpha(if palette.is_dark() { 120 } else { 40 }),
    };

    // egui keeps a separate style per theme and picks one by the active theme,
    // so both have to be filled in or switching leaves the other at its default.
    let theme = if palette.is_dark() { egui::Theme::Dark } else { egui::Theme::Light };
    ctx.set_theme(theme);
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style_of(egui::Theme::Light, style);
}
