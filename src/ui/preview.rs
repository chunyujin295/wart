//! The live preview pane.
//!
//! The grid is laid out with a `LayoutJob` per row: one text section per cell,
//! each carrying its own `TextFormat`. That is what gives per-character colour
//! without leaving egui's text engine.
//!
//! Reshaping is not cheap — a 100x40 artwork is 4,000 text sections — and doing
//! it every repaint is what made dragging a slider stutter. The galleys are
//! therefore cached and rebuilt only when the artwork or the zoom changes.

use eframe::egui;
use egui::Color32;

use crate::app::WartApp;

pub fn panel(app: &mut WartApp, ui: &mut egui::Ui) {
    // After the control panel, so the artwork reflects this frame's settings
    // rather than the previous frame's.
    app.refresh();

    let roles = app.roles();

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(app.t().preview)
                .strong()
                .size(crate::ui::theme::UI_SECTION)
                .color(roles.heading),
        );
        ui.separator();

        ui.label(app.t().zoom);
        let mut size = app.font_size.round();
        if ui
            .add(
                egui::Slider::new(&mut size, 8.0..=40.0)
                    .step_by(1.0)
                    // Integer sizes keep the cell grid on whole pixels; a
                    // fractional size makes columns shimmer as they re-rasterize.
                    .integer(),
            )
            .changed()
        {
            app.font_size = size;
        }

        ui.separator();
        let background = app.t().background;
        if ui
            .checkbox(&mut app.opaque_background, background)
            .changed()
        {
            // Nothing to invalidate: the background is painted outside the galley.
        }
        if app.opaque_background {
            ui.color_edit_button_srgba(&mut app.background);
        }
    });

    ui.separator();

    if app.artwork().is_empty() {
        ui.weak(app.t().nothing_to_preview);
        return;
    }

    // Built here, outside the scroll area: mutating app state from inside its
    // layout closure makes the measured content size jump between frames.
    if app.galleys_are_stale() {
        app.rebuild_galleys(ui);
    }

    let bg = app
        .opaque_background
        .then(|| app.background)
        .unwrap_or(Color32::TRANSPARENT);

    egui::Frame::new()
        .fill(bg)
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    // Every allocated row picks up `item_spacing.y` on top of its
                    // own height, so the theme's comfortable spacing becomes a
                    // gap between every line of art — which no amount of tuning
                    // `line_height` can remove. The preview sets its own rhythm.
                    ui.spacing_mut().item_spacing.y = 0.0;

                    for galley in &app.galleys {
                        let size = galley.size();
                        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                        ui.painter().galley(rect.min, galley.clone(), Color32::WHITE);
                    }
                });
        });
}
