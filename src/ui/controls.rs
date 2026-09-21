//! The left-hand control column.
//!
//! Grouped into collapsible sections rather than one long list. At a comfortable
//! reading size the flat version ran past the bottom of a 1080p window, so the
//! colour controls were permanently below the fold — you had to scroll to change
//! a colour and scroll back to see the preview.

use eframe::egui;

use crate::app::{StyleKind, WartApp};
use crate::render::{charset::Charset, figlet::IconStyle, frame::FrameKind, Mode};

pub fn panel(app: &mut WartApp, ui: &mut egui::Ui) {
    let roles = app.roles();

    // Warn when Chinese is selected but the system has no font to draw it with.
    // Deliberately in English: if the CJK font is missing, a Chinese warning
    // would itself render as boxes, which is exactly what it is explaining.
    if app.lang == crate::i18n::Lang::Zh && !crate::ui::fonts::cjk_available() {
        ui.colored_label(
            roles.warn,
            "No CJK font found on this system, so Chinese labels may show as boxes. \
             Install one (e.g. DengXian or SimHei) or switch to English.",
        );
        ui.add_space(6.0);
    }

    // Export first, above the scrolling settings: it acts on the result rather
    // than the settings, and as the last section of a long column it was the one
    // control always out of reach.
    section(app, ui, app.t().export, true, |app, ui| {
        crate::ui::export_bar::panel(app, ui)
    });

    section(app, ui, app.t().text, true, |app, ui| text_section(app, ui));
    section(app, ui, app.t().icons, false, |app, ui| {
        crate::ui::icons::panel(app, ui)
    });
    section(app, ui, app.t().mode, true, |app, ui| render_section(app, ui));
    section(app, ui, app.t().frame, false, |app, ui| frame_section(app, ui));
    section(app, ui, app.t().color, true, |app, ui| color_section(app, ui));
    ui.add_space(10.0);
}

/// A collapsible group with a readable heading.
///
/// The heading colour is set explicitly: left to itself, `CollapsingHeader`
/// picked a shade that vanished against the light palette.
fn section(
    app: &mut WartApp,
    ui: &mut egui::Ui,
    title: &str,
    open: bool,
    contents: impl FnOnce(&mut WartApp, &mut egui::Ui),
) {
    let heading = app.roles().heading;
    let response = egui::CollapsingHeader::new(
        egui::RichText::new(title)
            .strong()
            .size(crate::ui::theme::UI_SECTION)
            .color(heading),
    )
    .default_open(open)
    .show(ui, |ui| contents(app, ui));
    let _ = response;
}

fn text_section(app: &mut WartApp, ui: &mut egui::Ui) {
    let hint = app.t().text_hint;
    let response = ui.add(
        egui::TextEdit::multiline(&mut app.text)
            .desired_rows(3)
            .desired_width(f32::INFINITY)
            .hint_text(hint),
    );
    if response.changed() {
        app.touch();
    }
    ui.weak(app.t().text_note);
}

fn render_section(app: &mut WartApp, ui: &mut egui::Ui) {
    ui.label(app.t().mode);
    ui.horizontal_wrapped(|ui| {
        for mode in Mode::ALL {
            let selected = app.mode == mode;
            if ui
                .selectable_label(selected, app.t().mode(mode))
                .clicked()
                && !selected
            {
                app.mode = mode;
                app.touch();
            }
        }
    });

    ui.add_space(8.0);

    match app.mode {
        Mode::Figlet => font_picker(app, ui),
        Mode::Block => block_options(app, ui),
    }
}

fn font_picker(app: &mut WartApp, ui: &mut egui::Ui) {
    ui.label(app.t().font);
    // The combo borrows `font_names` while the closure runs, so the choice is
    // collected and applied afterwards rather than assigned in place.
    let mut picked: Option<String> = None;

    let combo = egui::ComboBox::from_id_salt("font")
        .selected_text(&app.font_name)
        .width(210.0)
        .height(420.0)
        // A ComboBox closes on *any* click by default, including one inside its
        // own popup — which makes the filter box below unclickable, since
        // clicking it dismisses the popup before it can take focus.
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show_ui(ui, |ui| {
            let hint = app.t().font_search;
            ui.add(
                egui::TextEdit::singleline(&mut app.font_filter)
                    .desired_width(f32::INFINITY)
                    .hint_text(hint),
            );
            ui.separator();

            let needle = app.font_filter.to_ascii_lowercase();
            let mut shown = 0usize;
            for name in &app.font_names {
                if !needle.is_empty() && !name.to_ascii_lowercase().contains(&needle) {
                    continue;
                }
                shown += 1;
                let selected = app.font_name == *name;
                if ui.selectable_label(selected, name).clicked() && !selected {
                    picked = Some(name.clone());
                    // Selection still dismisses the popup; keeping it open is
                    // only wanted for the filter box.
                    ui.close();
                }
            }
            if shown == 0 {
                ui.weak(app.t().font_no_match);
            }
        });

    // Reset the filter whenever the dropdown button is clicked, so reopening it
    // shows the full list rather than silently still filtered by the last
    // search — which reads as "most of my fonts have disappeared".
    if combo.response.clicked() {
        app.font_filter.clear();
    }

    if let Some(name) = picked {
        app.font_name = name;
        app.font_filter.clear();
        app.touch();
    }

    ui.weak(format!("{} {}", app.font_names.len(), app.t().font_note));
    ui.weak(app.t().font_ascii_note);

    ui.add_space(10.0);
    ui.label(app.t().icon_style);
    // Wrapped rather than one long row: six options with wider hit targets do
    // not fit across the panel, and cramming them made them easy to misclick.
    ui.horizontal_wrapped(|ui| {
        for style in IconStyle::ALL {
            let selected = app.icon_style == style;
            if ui
                .selectable_label(selected, app.t().icon_style(style))
                .clicked()
                && !selected
            {
                app.icon_style = style;
                app.touch();
            }
        }
    });
    ui.weak(app.t().icon_style_note);
    if app.icon_style == IconStyle::HalfBlock {
        let warn = app.roles().warn;
        ui.colored_label(warn, app.t().halfblock_note);
    }
}

fn block_options(app: &mut WartApp, ui: &mut egui::Ui) {
    ui.label(app.t().charset);
    ui.horizontal_wrapped(|ui| {
        for charset in Charset::ALL {
            let selected = app.charset == charset;
            if ui
                .selectable_label(selected, app.t().charset(charset))
                .clicked()
                && !selected
            {
                app.charset = charset;
                app.touch();
            }
        }
    });

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(app.t().width);
        let mut cols = app.cols as u32;
        if ui
            .add(egui::Slider::new(&mut cols, 20..=400).suffix(app.t().cells))
            .changed()
        {
            app.cols = cols as usize;
            app.touch();
        }
    });

    // The threshold only means something for charsets that cut rather than ramp.
    let threshold_matters = matches!(app.charset, Charset::Braille | Charset::HalfBlock);
    ui.add_enabled_ui(threshold_matters, |ui| {
        ui.horizontal(|ui| {
            ui.label(app.t().cutoff);
            if ui
                .add(egui::Slider::new(&mut app.threshold, 0.05..=0.95).fixed_decimals(2))
                .changed()
            {
                app.touch();
            }
        });
    });
    ui.weak(app.t().cutoff_note);
}

fn frame_section(app: &mut WartApp, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        for kind in FrameKind::ALL {
            let selected = app.frame_kind == kind;
            if ui
                .selectable_label(selected, app.t().frame_kind(kind))
                .clicked()
                && !selected
            {
                app.frame_kind = kind;
                app.touch();
            }
        }
    });

    // Padding is only meaningful once there is a border to pad against.
    ui.add_enabled_ui(app.frame_kind != FrameKind::None, |ui| {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(app.t().frame_padding);
            let mut pad = app.frame_padding;
            if ui
                .add(egui::Slider::new(&mut pad, 0..=crate::render::frame::MAX_PADDING))
                .changed()
            {
                app.frame_padding = pad;
                app.touch();
            }
        });
    });

    ui.weak(app.t().frame_note);
}

fn color_section(app: &mut WartApp, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        for kind in StyleKind::ALL {
            let selected = app.style_kind == kind;
            if ui
                .selectable_label(selected, app.t().style(kind))
                .clicked()
                && !selected
            {
                app.style_kind = kind;
                app.touch();
            }
        }
    });

    ui.add_space(8.0);
    match app.style_kind {
        StyleKind::Solid => {
            if ui.color_edit_button_srgba(&mut app.solid).changed() {
                app.touch();
            }
        }
        StyleKind::Linear | StyleKind::Radial => {
            ui.horizontal(|ui| {
                ui.label(app.t().from);
                if ui.color_edit_button_srgba(&mut app.grad_from).changed() {
                    app.touch();
                }
                ui.add_space(6.0);
                ui.label(app.t().to);
                if ui.color_edit_button_srgba(&mut app.grad_to).changed() {
                    app.touch();
                }
            });
            if app.style_kind == StyleKind::Linear {
                ui.add_space(6.0);
                angle_slider(app, ui, true);
                ui.weak(app.t().angle_note);
            }
        }
        StyleKind::Rainbow => angle_slider(app, ui, false),
    }
}

/// The angle control, shared by the linear and rainbow styles.
///
/// The suffix is copied out first: `&mut app.angle` and `app.t()` cannot appear
/// in the same expression, and the string is `&'static str`, so copying is free.
fn angle_slider(app: &mut WartApp, ui: &mut egui::Ui, with_label: bool) {
    let label = app.t().angle;
    let suffix = app.t().deg;
    ui.horizontal(|ui| {
        if with_label {
            ui.label(label);
        }
        if ui
            .add(egui::Slider::new(&mut app.angle, 0.0..=360.0).suffix(suffix))
            .changed()
        {
            app.touch();
        }
    });
}
