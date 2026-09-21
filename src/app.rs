//! The eframe application: state, control panel and live preview.

use eframe::egui;
use egui::{Color32, FontId, TextFormat};

use crate::color::ColorStyle;
use crate::export::{self, Format};
use crate::fonts;
use crate::i18n::Lang;
use crate::model::{Artwork, Rgb};
use crate::render::{self, charset::Charset, figlet::IconStyle, frame::FrameKind, Mode};
use crate::ui::theme::Palette;

/// Which color style the UI is currently editing. The concrete `ColorStyle` is
/// rebuilt from these fields on every change, so the controls stay simple.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleKind {
    Solid,
    Linear,
    Radial,
    Rainbow,
}

impl StyleKind {
    pub const ALL: [StyleKind; 4] =
        [StyleKind::Solid, StyleKind::Linear, StyleKind::Radial, StyleKind::Rainbow];
}

fn to_color32(c: Rgb) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

fn from_color32(c: Color32) -> Rgb {
    Rgb::new(c.r(), c.g(), c.b())
}

pub struct WartApp {
    // --- inputs -----------------------------------------------------------
    pub text: String,
    pub mode: Mode,
    pub font_name: String,
    pub font_names: Vec<String>,
    /// Filter text for the font dropdown. With ~330 built-in fonts a flat list
    /// is unusable, so the popup is searchable.
    pub font_filter: String,
    pub charset: Charset,
    pub cols: usize,
    pub threshold: f32,
    pub frame_kind: FrameKind,
    pub frame_padding: usize,
    pub icon_style: IconStyle,
    pub icon_query: String,
    pub icon_category: Option<String>,
    /// Filtered icon list, cached against `icon_cache_key`.
    ///
    /// Sorting ~11,000 entries is far too much work to redo every frame, and the
    /// picker needs the full result set (not a page) so it can virtualize over
    /// it.
    pub icon_results: Vec<&'static crate::nerd::Icon>,
    /// `None` until the first refresh. Seeding this with the empty-query key
    /// instead would make the very first call to `refresh_icons` look like a
    /// cache hit and leave the picker permanently empty.
    icon_cache_key: Option<(String, Option<String>)>,
    pub style_kind: StyleKind,
    pub solid: Color32,
    pub grad_from: Color32,
    pub grad_to: Color32,
    pub angle: f32,
    pub format: Format,
    /// Draw a background color behind the preview and in exports.
    pub opaque_background: bool,
    pub background: Color32,
    pub font_size: f32,

    /// The mark drawn beside the title in the top bar.
    pub logo: Option<egui::TextureHandle>,
    pub lang: Lang,
    pub palette: Palette,
    /// The palette currently pushed into the context, so the style is rebuilt
    /// when the choice changes rather than every frame.
    applied_palette: Option<Palette>,

    /// Laid-out preview rows, reused until the artwork or zoom changes.
    ///
    /// Rebuilding these every frame meant re-shaping every text section on
    /// every repaint — expensive enough that dragging a slider stuttered.
    pub galleys: Vec<std::sync::Arc<egui::Galley>>,
    galley_key: Option<u64>,
    /// Completed frames, used to keep the first pass from caching a galley that
    /// was shaped before the bundled fonts were live.
    passes: u64,

    // --- derived state ----------------------------------------------------
    artwork: Artwork,
    /// Bumped whenever an input changes, so the preview only reshapes when needed.
    revision: u64,
    rendered_at: u64,
    pub error: Option<String>,
    pub status: Option<String>,
}

impl WartApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::ui::fonts::install(&cc.egui_ctx);
        // The style itself is pushed on the first frame by `sync_theme`, which
        // needs the palette that is about to be chosen here.
        let palette = crate::ui::theme::system_default(&cc.egui_ctx);

        Self {
            // The interface is written for a Chinese-speaking user; the
            // switcher is one click away for anyone else.
            logo: load_logo(&cc.egui_ctx),
            lang: Lang::Zh,
            palette,
            applied_palette: None,
            galleys: Vec::new(),
            galley_key: None,
            passes: 0,
            text: "WART".to_owned(),
            mode: Mode::Figlet,
            font_name: "Standard".to_owned(),
            font_names: fonts::all_names(),
            font_filter: String::new(),
            charset: Charset::Braille,
            cols: 100,
            threshold: 0.5,
            frame_kind: FrameKind::None,
            frame_padding: 1,
            icon_style: IconStyle::default(),
            icon_query: String::new(),
            icon_category: None,
            icon_results: Vec::new(),
            icon_cache_key: None,
            style_kind: StyleKind::Linear,
            solid: Color32::from_rgb(0xff, 0x88, 0x00),
            grad_from: Color32::from_rgb(0xff, 0x5f, 0x5f),
            grad_to: Color32::from_rgb(0x5f, 0xaf, 0xff),
            angle: 0.0,
            format: Format::Ansi,
            opaque_background: false,
            background: Color32::from_rgb(0x10, 0x10, 0x14),
            font_size: 16.0,
            artwork: Artwork::new(),
            revision: 0,
            // Start stale so the first frame renders.
            rendered_at: u64::MAX,
            error: None,
            status: None,
        }
    }

    /// Call after mutating any input field.
    pub fn touch(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    /// Identity of the current preview layout. Changes exactly when the galleys
    /// need rebuilding: a new artwork, or a different zoom.
    pub fn galley_key(&self) -> u64 {
        self.revision ^ ((self.font_size.to_bits() as u64) << 32)
    }

    pub fn galleys_are_stale(&self) -> bool {
        // `set_fonts` does not take effect until the pass after it is called, so
        // a galley shaped on the very first frame comes out in egui's default
        // face and stays wrong forever once cached. Nothing is cached until one
        // pass has completed; the fonts are installed once, so this settles.
        self.passes > 0 && self.galley_key != Some(self.galley_key())
    }

    /// Rebuild the preview galleys if anything they depend on changed.
    pub fn rebuild_galleys(&mut self, ui: &egui::Ui) {
        // Jobs own their text, so the immutable borrow of the artwork ends
        // before the cache is written.
        let jobs: Vec<_> = self
            .artwork
            .cells
            .iter()
            .map(|row| row_layout_job(row, self.font_size))
            .collect();
        self.galleys = jobs
            .into_iter()
            .map(|job| ui.painter().layout_job(job))
            .collect();
        self.galley_key = Some(self.galley_key());
    }

    /// Push the palette into the context when it changes.
    fn sync_theme(&mut self, ctx: &egui::Context) {
        if self.applied_palette == Some(self.palette) {
            return;
        }
        crate::ui::theme::apply(ctx, self.palette);
        self.applied_palette = Some(self.palette);
        // The preview is drawn with palette colours, so it has to be laid out
        // again... except it is not: the artwork's own colours are unaffected.
        // Only the surrounding chrome changes.
    }

    /// Semantic colours for the active palette.
    pub fn roles(&self) -> crate::ui::theme::Roles {
        self.palette.roles()
    }

    pub fn artwork(&self) -> &Artwork {
        &self.artwork
    }

    /// Recompute the icon picker's result list if the filter changed.
    ///
    /// The full result set is kept, not a page: the picker virtualizes its
    /// drawing, so it needs the whole list to know how far it can scroll.
    pub fn refresh_icons(&mut self) {
        if !icon_cache_stale(
            &self.icon_cache_key,
            &self.icon_query,
            self.icon_category.as_deref(),
        ) {
            return;
        }
        self.icon_cache_key = Some((self.icon_query.clone(), self.icon_category.clone()));
        self.icon_results = crate::nerd::search(
            &self.icon_query,
            self.icon_category.as_deref(),
            crate::nerd::total(),
        );
    }

    pub fn color_style(&self) -> ColorStyle {
        match self.style_kind {
            StyleKind::Solid => ColorStyle::Solid(from_color32(self.solid)),
            StyleKind::Linear => ColorStyle::linear(
                from_color32(self.grad_from),
                from_color32(self.grad_to),
                self.angle,
            ),
            StyleKind::Radial => {
                ColorStyle::radial(&[from_color32(self.grad_from), from_color32(self.grad_to)])
            }
            StyleKind::Rainbow => ColorStyle::rainbow(self.angle),
        }
    }

    fn background_rgb(&self) -> Option<Rgb> {
        self.opaque_background.then(|| from_color32(self.background))
    }

    /// Re-render if any input changed since the last pass.
    ///
    /// Called from the preview, *after* the control panel has run. At the top of
    /// the pass it would be working from last frame's settings, and since egui
    /// only repaints when something asks it to, the preview then sat one
    /// interaction behind until the next mouse move.
    pub fn refresh(&mut self) {
        if self.rendered_at == self.revision {
            return;
        }
        self.rendered_at = self.revision;

        let style = self.color_style();
        let request = render::Request {
            text: &self.text,
            mode: self.mode,
            font_name: &self.font_name,
            charset: self.charset,
            cols: self.cols,
            threshold: self.threshold,
            style: &style,
            frame: render::frame::Options {
                kind: self.frame_kind,
                padding: self.frame_padding,
            },
            icon_style: self.icon_style,
        };

        match render::render(&request) {
            Ok(artwork) => {
                self.artwork = artwork;
                self.error = None;
            }
            Err(e) => {
                self.artwork = Artwork::new();
                self.error = Some(e.to_string());
            }
        }
    }

    /// The artwork as it should be written out.
    ///
    /// Half-block mode bakes its colors into `bg`, so a background is only
    /// filled in where the generator did not already set one. Overwriting would
    /// flatten every half-block cell into a solid square.
    fn prepared_artwork(&self) -> Artwork {
        let mut art = self.artwork.clone();
        if let Some(bg) = self.background_rgb() {
            for row in &mut art.cells {
                for cell in row.iter_mut() {
                    if cell.bg.is_none() && cell.ch != ' ' {
                        cell.bg = Some(bg);
                    }
                }
            }
        }
        art
    }

    /// `logo_source` is left as a bare filename: this artwork is going to a
    /// clipboard or to stdout, and the companion written alongside it lands in
    /// the working directory.
    fn export_options(&self, logo_name: &str) -> export::Options {
        export::Options {
            color: true,
            xterm256: false,
            background: self.background_rgb(),
            logo_name: logo_name.to_owned(),
            logo_source: format!("{logo_name}.txt"),
        }
    }

    pub fn export_bytes(&self) -> anyhow::Result<Vec<u8>> {
        let art = self.prepared_artwork();
        export::export(&art, self.format, &self.export_options(&self.export_stem()))
    }

    /// Filename stem for an export, derived from the input text.
    ///
    /// Shared, so a fastfetch config and the ANSI file it points at come out
    /// with matching names instead of the user having to rename one.
    pub fn export_stem(&self) -> String {
        let stem: String = self
            .text
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(24)
            .collect();
        if stem.is_empty() {
            self.t().default_stem.to_owned()
        } else {
            stem
        }
    }

    pub fn save_to(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        // The stem comes from the file the user actually chose, so a fastfetch
        // config's `source` and the companion written below agree with it —
        // rather than with whatever the input text happened to spell.
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.export_stem());

        let art = self.prepared_artwork();
        let mut opts = self.export_options(&stem);
        // fastfetch resolves a relative source against its working directory,
        // not the config's, so a saved config has to spell the path out.
        opts.logo_source = std::path::absolute(path)
            .map(|p| p.with_file_name(format!("{stem}.txt")))
            .unwrap_or_else(|_| std::path::PathBuf::from(format!("{stem}.txt")))
            .to_string_lossy()
            .into_owned();
        let bytes = export::export(&art, self.format, &opts)?;
        write_file(path, &bytes)?;

        let mut written = vec![path.display().to_string()];
        for (companion, data) in export::companions(&art, self.format, &opts, path) {
            write_file(&companion, &data)?;
            written.push(companion.display().to_string());
        }

        self.status = Some(match written.len() {
            1 => self.lang.wrote_file(&written[0], bytes.len()),
            // Saying only "wrote config.jsonc" would hide the logo that came
            // with it, which the config cannot work without.
            _ => self.lang.wrote_files(&written),
        });
        Ok(())
    }

    /// The active translation table.
    pub fn t(&self) -> &'static crate::i18n::Strings {
        self.lang.strings()
    }
}

impl eframe::App for WartApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.sync_theme(ui.ctx());

        // The top bar takes a fixed strip; the panels that follow divide what is
        // left. The export controls live in a strip of their own at the top of
        // the control column: they act on the result rather than the settings,
        // and as the last section of a scrolling column they were the one thing
        // always out of reach.
        egui::Panel::top("topbar").show(ui, |ui| crate::ui::topbar::panel(self, ui));
        egui::Panel::left("controls")
            .resizable(true)
            .default_size(360.0)
            .max_size(620.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    crate::ui::controls::panel(self, ui);
                });
            });

        egui::CentralPanel::default().show(ui, |ui| {
            crate::ui::preview::panel(self, ui);
        });

        self.passes += 1;
    }
}

fn write_file(path: &std::path::Path, bytes: &[u8]) -> anyhow::Result<()> {
    std::fs::write(path, bytes)
        .map_err(|e| anyhow::anyhow!("failed to write {}: {e}", path.display()))
}

/// Whether the cached icon list no longer matches the current filter.
///
/// Extracted so the "first call must populate" rule is testable. Seeding the
/// cache key with the empty-query key instead of `None` makes the very first
/// call look like a hit, which silently leaves the picker empty on startup.
fn icon_cache_stale(
    cached: &Option<(String, Option<String>)>,
    query: &str,
    category: Option<&str>,
) -> bool {
    let key = (query.to_owned(), category.map(str::to_owned));
    cached.as_ref() != Some(&key)
}

/// Upload the application mark once, for the top bar to draw each frame.
fn load_logo(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let image = image::load_from_memory(crate::assets::APP_ICON_PNG).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let pixels = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    Some(ctx.load_texture("app-logo", pixels, egui::TextureOptions::LINEAR))
}

/// The icon Windows draws in the taskbar and the title bar.
///
/// Separate from the executable's own icon, which is a PE resource linked in at
/// build time and is what Explorer and the Start menu show.
fn window_icon() -> Option<egui::IconData> {
    let image = image::load_from_memory(crate::assets::APP_ICON_PNG).ok()?.to_rgba8();
    let (width, height) = image.dimensions();
    Some(egui::IconData { rgba: image.into_raw(), width, height })
}

/// Start the GUI. Used when `wart` is run with no arguments.
pub fn launch() -> anyhow::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        // Sized to fit a 1080p screen *after* display scaling: at 125% a
        // 1380x900 logical window becomes taller than the screen, which
        // pushes the bottom bar off the display entirely.
        .with_inner_size([1180.0, 760.0])
        .with_min_inner_size([820.0, 520.0])
        .with_title("wart");
    if let Some(icon) = window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "wart",
        options,
        Box::new(|cc| Ok(Box::new(WartApp::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("failed to start the GUI: {e}"))
}

/// Build the colored, monospace layout for one row of the artwork.
pub fn row_layout_job(row: &[crate::model::Cell], font_px: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    // One artwork row must stay one visual row: wrapping would break the grid.
    job.wrap.max_width = f32::INFINITY;
    job.wrap.max_rows = 1;
    job.break_on_newline = false;

    let mut buf = [0u8; 4];
    for cell in row {
        job.append(
            cell.ch.encode_utf8(&mut buf),
            0.0,
            TextFormat {
                font_id: FontId::monospace(font_px),
                color: cell.fg.map_or(Color32::GRAY, to_color32),
                background: cell.bg.map_or(Color32::TRANSPARENT, to_color32),
                ..Default::default()
            },
        );
    }
    job
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_refresh_is_never_a_cache_hit() {
        // Regression: seeding the cache key with ("", None) made the very first
        // call look cached, so the picker came up empty until the user typed.
        assert!(icon_cache_stale(&None, "", None));
    }

    #[test]
    fn an_unchanged_filter_is_a_cache_hit() {
        let cached = Some((String::new(), None));
        assert!(!icon_cache_stale(&cached, "", None));

        let cached = Some(("arrow".to_owned(), Some("fa".to_owned())));
        assert!(!icon_cache_stale(&cached, "arrow", Some("fa")));
    }

    #[test]
    fn changing_either_the_query_or_the_category_invalidates() {
        let cached = Some(("arrow".to_owned(), None));
        assert!(icon_cache_stale(&cached, "arrows", None));
        assert!(icon_cache_stale(&cached, "arrow", Some("fa")));
        assert!(icon_cache_stale(&cached, "", None));
    }

    #[test]
    fn the_application_icon_decodes() {
        // A broken icon would fail silently at runtime — `window_icon` returns
        // `Option` and the callers just skip it — so the asset is checked here.
        assert!(window_icon().is_some(), "the bundled icon should decode");
        let icon = window_icon().expect("icon");
        assert!(icon.width > 0 && icon.height > 0);
        assert_eq!(
            icon.rgba.len(),
            (icon.width * icon.height * 4) as usize,
            "the pixel buffer should be RGBA"
        );
    }

    #[test]
    fn clearing_a_category_invalidates() {
        let cached = Some((String::new(), Some("dev".to_owned())));
        assert!(icon_cache_stale(&cached, "", None));
    }
}
