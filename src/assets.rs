//! Compile-time embedded assets.
//!
//! `include_bytes!`/`include_str!` rather than a runtime asset directory: the
//! set is fixed and small, it keeps the binary self-contained, and it lets
//! `FontData::from_static` borrow the fonts instead of copying them.

/// JetBrainsMono Nerd Font (Mono variant).
///
/// The `Mono` build matters: its icon glyphs all share the monospace advance
/// width, so a column of icons lines up with a column of letters. The plain
/// `JetBrainsMonoNerdFont-Regular.ttf` gives icons proportional widths and
/// visibly breaks the grid.
///
/// Used for the artwork preview and for rasterizing, where the icon glyphs are
/// the whole point.
pub static NERD_FONT_TTF: &[u8] =
    include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-Regular.ttf");

/// Fusion Pixel Font 12px, Simplified Chinese (OFL-1.1).
///
/// A pixel font, so it is drawn at integer multiples of its 12px design size —
/// see `ui::theme`, which picks sizes to suit.
pub static PIXEL_FONT_OTF: &[u8] =
    include_bytes!("../assets/fonts/fusion-pixel-12px-monospaced-zh_hans.otf");

/// The application icon: the window and taskbar icon, and the mark shown beside
/// the title.
///
/// A 256px copy rather than the 1254px original — it is never drawn larger than
/// a few dozen pixels, and the full-size decode would cost a few megabytes for
/// nothing. The executable's own icon is separate, linked in as a PE resource
/// (see `build.rs`).
pub static APP_ICON_PNG: &[u8] = include_bytes!("../assets/icon/icon.png");

/// Nerd Fonts icon name to codepoint metadata, for the icon picker.
pub static GLYPHNAMES_JSON: &str = include_str!("../assets/glyphnames.json");
