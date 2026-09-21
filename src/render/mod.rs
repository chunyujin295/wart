//! Generation modes. Each takes input text and produces an [`Artwork`].

pub mod block;
pub mod charset;
pub mod figlet;
pub mod frame;
pub mod shape;

use crate::color::{self, ColorStyle};
use crate::model::Artwork;
use charset::Charset;

/// How the input text becomes a grid of cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Classic multi-row FIGlet banners, one glyph per input character.
    /// ASCII only — `.flf` fonts have no glyphs for anything else.
    #[default]
    Figlet,
    /// Rasterize the text with a real font, then map pixels to characters.
    /// This is the mode that can render Nerd Font icons and true gradients.
    Block,
}

impl Mode {
    pub const ALL: [Mode; 2] = [Mode::Figlet, Mode::Block];
}

/// Everything needed to produce a finished, colored artwork.
pub struct Request<'a> {
    pub text: &'a str,
    pub mode: Mode,
    /// FIGlet font name. Ignored in block mode.
    pub font_name: &'a str,
    /// Block mode charset. Ignored in FIGlet mode.
    pub charset: Charset,
    /// Block mode target width in cells.
    pub cols: usize,
    /// Block mode coverage cutoff.
    pub threshold: f32,
    pub style: &'a ColorStyle,
    pub frame: frame::Options,
    /// How FIGlet mode draws characters its font has no glyph for.
    pub icon_style: figlet::IconStyle,
}

/// Render `text` and apply the color style.
///
/// Color is applied here rather than by the caller because the two modes differ:
/// FIGlet and most charsets leave `fg` unset for the shared colorize pass, while
/// half-block mode must sample the gradient itself (it paints two independently
/// colored pixels per cell). Keeping that decision in one place means the CLI and
/// the GUI cannot disagree about it.
pub fn render(req: &Request<'_>) -> anyhow::Result<Artwork> {
    match req.mode {
        Mode::Figlet => {
            let font = crate::fonts::load(req.font_name)?;
            let art = figlet::render(&font, req.text, req.style, req.icon_style).ok_or_else(|| {
                anyhow::anyhow!("nothing to render: input produced an empty result")
            })?;
            // Framed before coloring, so the border picks up the gradient.
            let mut artwork = frame::apply(&art, &req.frame);
            color::colorize(&mut artwork, req.style);
            Ok(artwork)
        }

        Mode::Block => {
            let opts = block::Options {
                charset: req.charset,
                cols: req.cols,
                threshold: req.threshold,
                style: req.style.clone(),
            };
            let art = block::render(req.text, &opts)?;
            let mut artwork = frame::apply(&art, &req.frame);
            if !req.charset.needs_color() {
                color::colorize(&mut artwork, req.style);
            }
            Ok(artwork)
        }
    }
}
