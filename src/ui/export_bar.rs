//! Choosing a format and getting the artwork out.
//!
//! These actions used to end the scrolling control column, which meant the
//! Save button was the one control never on screen when you wanted it. They now
//! sit at the top of the column, outside the scroll area, so they are always
//! reachable.
//!
//! (A `Panel::bottom` would have been the tidier home, but it rendered nothing
//! at all here while `Panel::top` and `Panel::left` worked, and the cause was
//! not worth more time than the placement is.)

use eframe::egui;

use crate::app::WartApp;
use crate::export::Format;

pub fn panel(app: &mut WartApp, ui: &mut egui::Ui) {
    let roles = app.roles();

    // Laid out in plain flow. Both `Sides` and a nested right-to-left layout
    // failed to produce any visible widget inside a `horizontal` here, so the
    // status simply follows the buttons rather than being pinned to the far end.
    let mut save = false;
    let mut copy = false;
    // Taken before the closures: the left one needs `app` mutably, so the right
    // one cannot hold a borrow of its own.
    let notice: Option<(egui::Color32, String)> = match (&app.error, &app.status) {
        (Some(error), _) => Some((roles.error, error.clone())),
        (None, Some(status)) => Some((roles.notice, status.clone())),
        _ => None,
    };

    let strings = app.t();
    let mut chosen = app.format;

    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("format")
            .selected_text(strings.format(chosen))
            .width(170.0)
            .show_ui(ui, |ui| {
                for format in Format::ALL {
                    if ui
                        .selectable_label(chosen == format, strings.format(format))
                        .clicked()
                    {
                        chosen = format;
                    }
                }
            });

        ui.add_space(6.0);
        save = ui.button(strings.save_as).clicked();
        copy = ui.button(strings.copy).clicked();

        if let Some((color, text)) = &notice {
            ui.add_space(16.0);
            ui.label(egui::RichText::new(text).color(*color));
        }
    });

    app.format = chosen;

    if save {
        save_dialog(app);
    }
    if copy {
        copy_to_clipboard(app);
    }
}

fn default_file_name(app: &WartApp) -> String {
    format!("{}.{}", app.export_stem(), app.format.extension())
}

fn save_dialog(app: &mut WartApp) {
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(default_file_name(app))
        .save_file()
    else {
        return;
    };
    if let Err(e) = app.save_to(&path) {
        app.status = Some(app.lang.error(&e.to_string()));
    }
}

fn copy_to_clipboard(app: &mut WartApp) {
    match app.export_bytes() {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => {
                // A single reused clipboard handle; creating one per copy fails
                // intermittently on Windows.
                match arboard::Clipboard::new().and_then(|mut c| c.set_text(text)) {
                    Ok(()) => app.status = Some(app.lang.copied()),
                    Err(e) => app.status = Some(app.lang.clipboard_error(&e.to_string())),
                }
            }
            Err(_) => app.status = Some(app.lang.copy_text_only()),
        },
        Err(e) => app.status = Some(app.lang.error(&e.to_string())),
    }
}
