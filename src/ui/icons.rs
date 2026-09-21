//! Searchable Nerd Font icon picker.
//!
//! The whole set is roughly 11,000 glyphs. Drawing them all would be impossible:
//! the egui font atlas is dropped and re-rasterized once it fills, so rendering
//! a few hundred new glyphs in one frame already causes visible flicker. The
//! grid is therefore virtualized — only the rows actually on screen are drawn —
//! which is what lets the picker expose the full set instead of a tiny page.

use eframe::egui;

use crate::app::WartApp;
use crate::nerd;
use crate::render::Mode;

const COLUMNS: usize = 8;
const BUTTON: f32 = 30.0;
const SPACING: f32 = 4.0;
const ROW_HEIGHT: f32 = BUTTON + SPACING;

/// Height of the scrolling grid.
const VIEWPORT: f32 = 260.0;

/// Amber, for a caution rather than an error.
const HINT: egui::Color32 = egui::Color32::from_rgb(0xd8, 0xa6, 0x40);

pub fn panel(app: &mut WartApp, ui: &mut egui::Ui) {
    app.refresh_icons();

    ui.horizontal(|ui| {
        // Copied out before the mutable borrow of `icon_query` below; the
        // string is `&'static str`, so this costs nothing.
        let hint = app.t().icons_search;
        ui.add(
            egui::TextEdit::singleline(&mut app.icon_query)
                .desired_width(104.0)
                .hint_text(hint),
        );

        let selected = match &app.icon_category {
            Some(c) => app.lang.icons_category(c, category_count(c)),
            None => app.lang.icons_category(app.t().icons_all, nerd::total()),
        };
        egui::ComboBox::from_id_salt("icon_category")
            .selected_text(selected)
            .width(124.0)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(app.icon_category.is_none(), app.t().icons_all)
                    .clicked()
                {
                    app.icon_category = None;
                }
                // Biggest sets first, with counts: the bulk of the icons live in
                // a handful of sets, and without the numbers it looks like the
                // picker only has the one set that sorts first.
                for (cat, count) in nerd::category_counts() {
                    let is_selected = app.icon_category.as_deref() == Some(cat);
                    if ui
                        .selectable_label(is_selected, app.lang.icons_category(cat, count))
                        .clicked()
                        && !is_selected
                    {
                        app.icon_category = Some(cat.to_owned());
                    }
                }
            });
    });

    if app.icon_results.is_empty() {
        ui.weak(app.t().icons_none);
        return;
    }

    if app.mode == Mode::Figlet {
        ui.add_space(2.0);
        ui.colored_label(HINT, app.t().icons_figlet_hint);
    }

    ui.weak(app.lang.icons_showing(app.icon_results.len(), nerd::total()));

    let rows = app.icon_results.len().div_ceil(COLUMNS);
    let mut clicked: Option<char> = None;

    egui::Frame::new()
        .inner_margin(egui::Margin::same(5))
        .show(ui, |ui| {
            // Only the visible row range is visited, so scrolling through
            // thousands of icons costs the same as scrolling through ten.
            egui::ScrollArea::vertical()
                .max_height(VIEWPORT)
                .auto_shrink([false, false])
                .show_rows(ui, ROW_HEIGHT, rows, |ui, range| {
                    for row in range {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = SPACING;
                            for col in 0..COLUMNS {
                                let Some(icon) = app.icon_results.get(row * COLUMNS + col)
                                else {
                                    break;
                                };
                                let glyph = egui::RichText::new(icon.ch).size(BUTTON * 0.62);
                                let response = ui.add(
                                    egui::Button::new(glyph)
                                        .min_size(egui::vec2(BUTTON, BUTTON)),
                                );
                                if response.clicked() {
                                    clicked = Some(icon.ch);
                                }
                                response.on_hover_text(format!(
                                    "{}  U+{:04X}",
                                    icon.name, icon.ch as u32
                                ));
                            }
                        });
                    }
                });
        });

    if let Some(ch) = clicked {
        // Icons render inline in FIGlet mode as real glyphs, so there is no
        // reason to move the user to Block mode behind their back.
        app.text.push(ch);
        app.touch();
    }
}

fn category_count(name: &str) -> usize {
    nerd::category_counts()
        .into_iter()
        .find(|(c, _)| *c == name)
        .map_or(0, |(_, n)| n)
}
