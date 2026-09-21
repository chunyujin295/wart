//! PNG export: draws the artwork cell by cell with the bundled Nerd Font.
//!
//! The whole trick is that every glyph in a row shares one baseline and is only
//! centered horizontally. Centering each glyph vertically inside its own box —
//! the intuitive thing — destroys the picture, because glyphs have wildly
//! different heights (`█` fills the em box, `.` is a few pixels) and the art
//! would dissolve into scattered marks.

use fontdue::{Font, FontSettings};
use image::{ExtendedColorType, ImageEncoder};

use crate::model::{Artwork, Rgb};

pub struct Options {
    /// Rasterization size of the font, in pixels.
    pub font_px: f32,
    /// Cell height divided by cell width. 2.0 matches a terminal.
    pub cell_aspect: f32,
    pub background: Option<Rgb>,
}

impl Default for Options {
    fn default() -> Self {
        Self { font_px: 22.0, cell_aspect: 2.0, background: None }
    }
}

fn load_font() -> anyhow::Result<Font> {
    Font::from_bytes(crate::assets::NERD_FONT_TTF, FontSettings::default())
        .map_err(|e| anyhow::anyhow!("failed to load the bundled font: {e}"))
}

/// Source-over blend of `fg` onto the buffer at `(x, y)` with the given coverage.
fn blend(buf: &mut [u8], width: u32, x: u32, y: u32, fg: Rgb, coverage: f32) {
    let k = coverage.clamp(0.0, 1.0);
    let i = ((y * width + x) * 4) as usize;
    if i + 3 >= buf.len() {
        return;
    }
    for (offset, src) in [(0usize, fg.r), (1, fg.g), (2, fg.b)] {
        let dst = buf[i + offset] as f32;
        buf[i + offset] = (src as f32 * k + dst * (1.0 - k)).round() as u8;
    }
    // Any ink makes the pixel opaque.
    buf[i + 3] = 255;
}

fn fill_rect(buf: &mut [u8], width: u32, height: u32, x0: u32, y0: u32, w: u32, h: u32, c: Rgb) {
    for y in y0..(y0 + h).min(height) {
        for x in x0..(x0 + w).min(width) {
            let i = ((y * width + x) * 4) as usize;
            if i + 3 >= buf.len() {
                continue;
            }
            buf[i] = c.r;
            buf[i + 1] = c.g;
            buf[i + 2] = c.b;
            buf[i + 3] = 255;
        }
    }
}

pub fn export(art: &Artwork, opts: &Options) -> anyhow::Result<Vec<u8>> {
    let font = load_font()?;

    let (cols, rows) = (art.width(), art.height());
    if cols == 0 || rows == 0 {
        anyhow::bail!("nothing to export: the artwork is empty");
    }

    // Derive the cell box from the font itself so the PNG and the terminal
    // preview agree on proportions.
    let cell_w = font.metrics('M', opts.font_px).advance_width.round().max(1.0) as u32;
    let cell_h = (cell_w as f32 * opts.cell_aspect).round().max(1.0) as u32;

    let line = font
        .horizontal_line_metrics(opts.font_px)
        .ok_or_else(|| anyhow::anyhow!("the bundled font has no horizontal metrics"))?;

    // Center the em box vertically, then measure the baseline down from the top.
    let text_height = line.ascent - line.descent;
    let baseline_off = ((cell_h as f32 - text_height) * 0.5 + line.ascent).round() as i32;

    let width = cols as u32 * cell_w;
    let height = rows as u32 * cell_h;

    let mut buf = vec![0u8; (width * height * 4) as usize];
    if let Some(bg) = opts.background {
        for pixel in buf.chunks_exact_mut(4) {
            pixel.copy_from_slice(&[bg.r, bg.g, bg.b, 255]);
        }
    }

    // Artwork uses few distinct characters, so caching rasterizations is a
    // large win on a big grid.
    let mut cache: std::collections::HashMap<char, (fontdue::Metrics, Vec<u8>)> =
        std::collections::HashMap::new();

    for (y, row) in art.cells.iter().enumerate() {
        for (x, cell) in row.iter().enumerate() {
            let x0 = x as u32 * cell_w;
            let y0 = y as u32 * cell_h;

            if let Some(bg) = cell.bg {
                fill_rect(&mut buf, width, height, x0, y0, cell_w, cell_h, bg);
            }

            let Some(fg) = cell.fg else {
                continue;
            };
            if cell.ch == ' ' {
                continue;
            }

            let (metrics, bitmap) = cache
                .entry(cell.ch)
                .or_insert_with(|| font.rasterize(cell.ch, opts.font_px));

            let pen_x = x0 as f32 + (cell_w as f32 - metrics.advance_width) * 0.5;
            let baseline_y = y0 as i32 + baseline_off;

            let glyph_x = pen_x.round() as i32 + metrics.xmin;
            // `ymin` is the offset of the bitmap's *bottom* edge above the
            // baseline, and the buffer is stored top row first.
            let glyph_y = baseline_y - metrics.ymin - metrics.height as i32;

            for gy in 0..metrics.height {
                for gx in 0..metrics.width {
                    let coverage = bitmap[gy * metrics.width + gx] as f32 / 255.0;
                    if coverage <= 0.0 {
                        continue;
                    }
                    let (px, py) = (glyph_x + gx as i32, glyph_y + gy as i32);
                    if px < 0 || py < 0 || px >= width as i32 || py >= height as i32 {
                        continue;
                    }
                    blend(&mut buf, width, px as u32, py as u32, fg, coverage);
                }
            }
        }
    }

    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&buf, width, height, ExtendedColorType::Rgba8)
        .map_err(|e| anyhow::anyhow!("failed to encode PNG: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Cell;

    fn colored_art() -> Artwork {
        let mut art = Artwork::from_lines(["ab"]);
        for c in art.cells[0].iter_mut() {
            c.fg = Some(Rgb::new(255, 255, 255));
        }
        art
    }

    #[test]
    fn produces_a_png_signature() {
        let bytes = export(&colored_art(), &Options::default()).expect("export");
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "not a PNG");
    }

    #[test]
    fn larger_font_makes_a_larger_image() {
        let art = colored_art();
        let small = export(&art, &Options { font_px: 12.0, ..Default::default() }).expect("small");
        let large = export(&art, &Options { font_px: 48.0, ..Default::default() }).expect("large");
        assert!(large.len() > small.len());
    }

    #[test]
    fn empty_artwork_is_an_error() {
        assert!(export(&Artwork::new(), &Options::default()).is_err());
    }

    #[test]
    fn uncolored_cells_still_produce_a_valid_image() {
        // Nothing to draw, but the image must still have the right dimensions
        // rather than collapsing to zero.
        let art = Artwork::from_lines(["  "]);
        let bytes = export(&art, &Options::default()).expect("export");
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[test]
    fn background_produces_a_different_image_than_transparent() {
        let art = colored_art();
        let plain = export(&art, &Options::default()).expect("plain");
        let bg = export(
            &art,
            &Options { background: Some(Rgb::new(0, 0, 0)), ..Default::default() },
        )
        .expect("bg");
        assert_ne!(plain, bg);
    }

    #[test]
    fn half_block_backgrounds_are_painted() {
        let art = Artwork {
            cells: vec![vec![Cell {
                ch: '▀',
                fg: Some(Rgb::new(255, 0, 0)),
                bg: Some(Rgb::new(0, 0, 255)),
            }]],
        };
        let with_bg = export(&art, &Options::default()).expect("export");
        let solid = Artwork {
            cells: vec![vec![Cell { ch: '▀', fg: Some(Rgb::new(255, 0, 0)), bg: None }]],
        };
        let without_bg = export(&solid, &Options::default()).expect("export");
        assert_ne!(with_bg, without_bg, "the background half should change the image");
    }
}
