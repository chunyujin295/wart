//! The top bar: what the app is, and the two settings that apply to all of it.
//!
//! Language and palette used to sit inside the control column, where the
//! language switcher was wedged next to the title and the palette did not exist.
//! Both are app-wide rather than per-render, so they belong here.
//!
//! Laid out in plain flow. Both `egui::Sides` and a nested right-to-left layout
//! silently produced no visible widgets inside a `horizontal`; keeping the
//! switchers next to the title is a smaller price than not showing them at all.

use eframe::egui;

use crate::app::WartApp;
use crate::i18n::Lang;
use crate::ui::theme::Palette;

pub fn panel(app: &mut WartApp, ui: &mut egui::Ui) {
    // Everything the closures need is copied out first. `Sides` runs both
    // closures inside one call, so neither may hold a borrow of `app` — and one
    // of them would have to, to record a choice.
    let roles = app.roles();
    let strings = app.t();
    let (mut lang, mut palette) = (app.lang, app.palette);
    // Cloned out for the same reason as the rest: the closure must not hold a
    // borrow of `app`. A `TextureHandle` is an `Arc` inside, so this is cheap.
    let logo = app.logo.clone();

    ui.horizontal(|ui| {
        if let Some(logo) = &logo {
            // Sized to the title's own height so the mark and the word sit on
            // the same optical line.
            let side = crate::ui::theme::UI_TITLE * 1.15;
            ui.add(
                egui::Image::new(egui::load::SizedTexture::new(
                    logo.id(),
                    egui::vec2(side, side),
                ))
                .corner_radius(3.0),
            );
        }
        ui.label(
            egui::RichText::new("wart")
                .size(crate::ui::theme::UI_TITLE)
                .strong()
                .color(roles.heading),
        );
        ui.label(egui::RichText::new(strings.tagline).color(roles.weak));

        ui.add_space(24.0);
        for option in Palette::ALL {
            if ui
                .selectable_label(palette == option, strings.palette(option))
                .clicked()
            {
                palette = option;
            }
        }
        ui.add_space(14.0);
        for option in Lang::ALL {
            if ui
                .selectable_label(lang == option, option.label())
                .clicked()
            {
                lang = option;
            }
        }
    });

    app.lang = lang;
    app.palette = palette;
}
