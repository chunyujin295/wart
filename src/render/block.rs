//! Block mode: rasterize text with a real font, then map pixels to characters.
//!
//! This is the mode that makes Nerd Font icons and smooth gradients possible —
//! FIGlet fonts only carry ASCII glyphs. It works by rendering the text into a
//! coverage bitmap and then averaging each charset-defined block of pixels down
//! into one character cell.

use fontdue::{Font, FontSettings};

use super::charset::{self, Charset};
use super::shape;
use crate::color::ColorStyle;
use crate::model::{Artwork, Cell};

/// Coverage below which the two halves of a half-block cell are treated as one
/// solid color. Without it, a flat region flickers between `█` and `▀`.
const SOLID_EPSILON: f32 = 0.08;

/// Coverage below which a half of a cell counts as empty rather than inked.
///
/// Half-block has a color for both halves, so without this the renderer paints
/// every cell and a sparse glyph comes out as a solid rectangle.
const INK_THRESHOLD: f32 = 0.12;

pub struct Options {
    pub charset: Charset,
    /// Desired output width in character cells. The rasterization size is
    /// derived from this, so the same text always comes out the same width
    /// regardless of charset.
    pub cols: usize,
    /// Coverage cutoff for charsets that threshold rather than ramp.
    pub threshold: f32,
    /// Half-block samples the gradient per half-pixel, so it needs the style
    /// here rather than from the shared colorize pass.
    pub style: ColorStyle,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            charset: Charset::Braille,
            cols: 100,
            threshold: 0.5,
            style: ColorStyle::default(),
        }
    }
}

struct Bitmap {
    width: usize,
    height: usize,
    /// Coverage in `0.0..=1.0`, row-major.
    coverage: Vec<f32>,
}

impl Bitmap {
    fn new(width: usize, height: usize) -> Self {
        Self { width, height, coverage: vec![0.0; width * height] }
    }

    fn at(&self, x: usize, y: usize) -> f32 {
        if x >= self.width || y >= self.height {
            return 0.0;
        }
        self.coverage[y * self.width + x]
    }
}

fn load_font() -> anyhow::Result<Font> {
    Font::from_bytes(crate::assets::NERD_FONT_TTF, FontSettings::default())
        .map_err(|e| anyhow::anyhow!("failed to load the bundled font: {e}"))
}

/// Longest line, in characters.
fn longest_line(text: &str) -> usize {
    text.split('\n').map(|l| l.chars().count()).max().unwrap_or(0)
}

/// Choose a rasterization size that makes the output about `rows` cells tall.
///
/// Mirrors [`pick_px`] but keys off height, for the inline-glyph case where the
/// banner's height is fixed and the width is free.
fn pick_px_for_rows(font: &Font, rows: usize, pixels_per_cell: usize) -> f32 {
    const REFERENCE: f32 = 100.0;
    let line = font.horizontal_line_metrics(REFERENCE);
    let line_height = line.map_or(REFERENCE * 1.2, |m| m.ascent - m.descent);

    if line_height <= 0.0 {
        return 32.0;
    }
    let target_px_height = (rows * pixels_per_cell) as f32;
    (target_px_height * REFERENCE / line_height).clamp(4.0, 512.0)
}

/// Choose a rasterization size that makes the output about `cols` cells wide.
///
/// Measuring the advance at a reference size and scaling linearly avoids
/// needing the metrics at the yet-unknown final size.
fn pick_px(font: &Font, text: &str, cols: usize, pixels_per_cell: usize) -> f32 {
    const REFERENCE: f32 = 100.0;
    let advance = font.metrics('M', REFERENCE).advance_width;
    let chars = longest_line(text).max(1);

    if advance <= 0.0 {
        return 32.0;
    }
    let target_px_width = (cols * pixels_per_cell) as f32;
    let px = target_px_width * REFERENCE / (chars as f32 * advance);
    // A tiny or enormous rasterization is never useful, and a zero would divide
    // by zero downstream.
    px.clamp(4.0, 512.0)
}

/// Draw `text` into a coverage bitmap at `px` size.
fn rasterize(font: &Font, text: &str, px: f32) -> Bitmap {
    let line_metrics = font.horizontal_line_metrics(px);
    let (ascent, line_height) = match line_metrics {
        Some(m) => (m.ascent, m.new_line_size),
        // No vertical metrics is unusual but not fatal; fall back to the em box.
        None => (px * 0.8, px * 1.2),
    };

    let lines: Vec<&str> = text.split('\n').collect();

    // Measure first so the buffer can be allocated exactly once.
    let mut max_advance = 0.0f32;
    for line in &lines {
        let mut w = 0.0f32;
        for ch in line.chars() {
            w += font.metrics(ch, px).advance_width;
        }
        max_advance = max_advance.max(w);
    }

    let width = (max_advance.ceil() as usize + 2).max(1);
    let height = ((ascent - descent_of(font, px)) + line_height * (lines.len().saturating_sub(1) as f32))
        .ceil() as usize
        + 2;
    let height = height.max(1);

    let mut bmp = Bitmap::new(width, height);

    for (row, line) in lines.iter().enumerate() {
        let baseline = ascent + row as f32 * line_height;
        let mut pen_x = 0.0f32;

        for ch in line.chars() {
            let (metrics, bitmap) = font.rasterize(ch, px);

            // fontdue reports `ymin` as the offset of the bitmap's bottom edge
            // above the baseline, and the buffer is stored top row first, so the
            // top edge in image coordinates is baseline - ymin - height.
            let glyph_left = pen_x.round() as i32 + metrics.xmin;
            let glyph_top = baseline.round() as i32 - metrics.ymin - metrics.height as i32;

            for gy in 0..metrics.height {
                for gx in 0..metrics.width {
                    let cov = bitmap[gy * metrics.width + gx] as f32 / 255.0;
                    if cov <= 0.0 {
                        continue;
                    }
                    let (px_x, px_y) = (glyph_left + gx as i32, glyph_top + gy as i32);
                    if px_x < 0 || px_y < 0 {
                        continue;
                    }
                    let (px_x, px_y) = (px_x as usize, px_y as usize);
                    if px_x >= bmp.width || px_y >= bmp.height {
                        continue;
                    }
                    let slot = &mut bmp.coverage[px_y * bmp.width + px_x];
                    // Glyphs can overlap after kerning; keep the strongest
                    // coverage rather than summing, which would blow out.
                    *slot = slot.max(cov);
                }
            }

            pen_x += metrics.advance_width;
        }
    }

    bmp
}

fn descent_of(font: &Font, px: f32) -> f32 {
    font.horizontal_line_metrics(px).map_or(px * 0.2, |m| m.descent)
}

/// Average coverage over a sub-block of the bitmap.
fn block_average(bmp: &Bitmap, x0: usize, y0: usize, w: usize, h: usize) -> f32 {
    let mut sum = 0.0;
    let mut n = 0;
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            sum += bmp.at(x, y);
            n += 1;
        }
    }
    if n == 0 {
        0.0
    } else {
        sum / n as f32
    }
}

/// Normalised position of a cell within the grid, in `0.0..=1.0`.
fn norm(index: usize, total: usize) -> f32 {
    if total > 1 {
        index as f32 / (total - 1) as f32
    } else {
        0.5
    }
}

/// Stroke strength below which a cell is not considered part of an edge.
///
/// Coverage runs 0..=1 and neighbours are one cell apart, so this is roughly
/// "the coverage changes by a fifth across the cell".
const EDGE_THRESHOLD: f32 = 0.28;

/// Pick the stroke character for one cell from the local gradient direction.
///
/// A FIGlet banner is drawn with `/ \ | _ -`, so using the same vocabulary here
/// is what makes a rasterized glyph look like it belongs beside the letters
/// instead of like a pasted-in dot matrix.
///
/// The gradient points towards increasing coverage; the stroke runs
/// perpendicular to it. `y` grows downward.
fn line_art_char(bmp: &Bitmap, x0: usize, y0: usize, cell_w: usize, cell_h: usize) -> char {
    let avg = |x: usize, y: usize| block_average(bmp, x, y, cell_w, cell_h);
    let left = avg(x0.saturating_sub(cell_w), y0);
    let right = avg(x0 + cell_w, y0);
    let up = avg(x0, y0.saturating_sub(cell_h));
    let down = avg(x0, y0 + cell_h);

    let gx = right - left;
    let gy = down - up;
    let (ax, ay) = (gx.abs(), gy.abs());

    if (ax * ax + ay * ay).sqrt() < EDGE_THRESHOLD {
        return ' ';
    }

    // Axis-aligned strokes get the flat characters; anything in between is a
    // diagonal. The 2:1 ratio keeps a slightly tilted edge from flipping
    // between `|` and `/` from cell to cell.
    if ay * 2.0 < ax {
        '|'
    } else if ax * 2.0 < ay {
        // Coverage rising downwards means this is the top of the shape, and
        // FIGlet fonts draw tops with `_` and bottoms with `-`.
        if gy > 0.0 {
            '_'
        } else {
            '-'
        }
    } else if gx * gy > 0.0 {
        // Rising to the right and downwards, so the stroke runs the other way.
        '/'
    } else {
        '\\'
    }
}

/// Map a coverage bitmap down to character cells.
fn downsample(bmp: &Bitmap, opts: &Options) -> Artwork {
    let (cell_w, cell_h) = opts.charset.pixels_per_cell();
    let cols = bmp.width.div_ceil(cell_w);
    let rows = bmp.height.div_ceil(cell_h);

    let aspect = (rows as f32 * crate::color::CELL_ASPECT) / cols.max(1) as f32;
    let mut artwork = Artwork::new();

    for cy in 0..rows {
        let mut row = Vec::with_capacity(cols);
        for cx in 0..cols {
            let x0 = cx * cell_w;
            let y0 = cy * cell_h;

            let cell = match opts.charset {
                Charset::Braille => {
                    let mut bits = [[false; 4]; 2];
                    for (col, column) in bits.iter_mut().enumerate() {
                        for (row_idx, dot) in column.iter_mut().enumerate() {
                            *dot = bmp.at(x0 + col, y0 + row_idx) > opts.threshold;
                        }
                    }
                    Cell::new(charset::braille_cell(bits))
                }

                Charset::HalfBlock => {
                    let half = (cell_h / 2).max(1);
                    let top = block_average(bmp, x0, y0, cell_w, half);
                    let bottom = block_average(bmp, x0, y0 + half, cell_w, cell_h - half);

                    let u = norm(cx, cols);
                    // The cell covers two source rows, so sample the gradient at
                    // both of them rather than once at the center.
                    let v_top = norm(cy * cell_h, rows * cell_h);
                    let v_bottom = norm(cy * cell_h + half, rows * cell_h);

                    // Half-block always has two colors available, so it is
                    // tempting to color every cell — but an empty cell has no
                    // ink in either half, and painting it turns sparse art into
                    // a solid rectangle. Check for ink first.
                    match (top >= INK_THRESHOLD, bottom >= INK_THRESHOLD) {
                        (false, false) => Cell::new(' '),
                        // Only one half is inked, so the cell is that half block
                        // and the color pass fills in its foreground.
                        (true, false) => Cell::new('▀'),
                        (false, true) => Cell::new('▄'),
                        (true, true) if (top - bottom).abs() < SOLID_EPSILON => {
                            // One flat color: a full block reads more solidly
                            // than a half block split between two near-identical
                            // colors.
                            let c = opts.style.at(u, (v_top + v_bottom) * 0.5, aspect);
                            Cell { ch: '█', fg: Some(c), bg: None }
                        }
                        (true, true) => {
                            // For an upper half block the *foreground* paints the
                            // top half and the background shows through below.
                            // The color pass re-samples both once the cell's
                            // final position is known.
                            Cell {
                                ch: '▀',
                                fg: Some(opts.style.at(u, v_top, aspect)),
                                bg: Some(opts.style.at(u, v_bottom, aspect)),
                            }
                        }
                    }
                }

                Charset::AsciiRamp | Charset::Blocks => {
                    let ramp = if opts.charset == Charset::Blocks {
                        charset::BLOCK_RAMP
                    } else {
                        charset::ASCII_RAMP
                    };
                    let cov = block_average(bmp, x0, y0, cell_w, cell_h);
                    Cell::new(charset::ramp_char(ramp, cov))
                }

                Charset::Quadrant => {
                    // Four sub-cells per character, each a real 1x2 block of
                    // source pixels. All sixteen inked combinations exist as
                    // quarter-block characters, so the silhouette is exact and
                    // the uninked quadrants stay transparent — no background
                    // color required.
                    let hw = (cell_w / 2).max(1);
                    let hh = (cell_h / 2).max(1);
                    let mut mask = 0u8;
                    if block_average(bmp, x0, y0, hw, hh) >= INK_THRESHOLD {
                        mask |= 0b0001; // upper left
                    }
                    if block_average(bmp, x0 + hw, y0, cell_w - hw, hh) >= INK_THRESHOLD {
                        mask |= 0b0010; // upper right
                    }
                    if block_average(bmp, x0, y0 + hh, hw, cell_h - hh) >= INK_THRESHOLD {
                        mask |= 0b0100; // lower left
                    }
                    if block_average(bmp, x0 + hw, y0 + hh, cell_w - hw, cell_h - hh)
                        >= INK_THRESHOLD
                    {
                        mask |= 0b1000; // lower right
                    }
                    Cell::new(charset::quadrant_char(mask))
                }

                Charset::Shape => {
                    // Hand the matcher the cell's actual pixels, resampled to
                    // the same eight regions the candidate glyphs were reduced
                    // to, then take whichever character sits closest.
                    let mut region = Vec::with_capacity(cell_w * cell_h);
                    for y in y0..y0 + cell_h {
                        for x in x0..x0 + cell_w {
                            region.push(bmp.at(x, y));
                        }
                    }
                    let signature = shape::resample(&region, cell_w, cell_h);
                    let ch = shape::matcher().map_or(' ', |m| m.best(&signature));
                    Cell::new(ch)
                }

                Charset::LineArt => Cell::new(line_art_char(bmp, x0, y0, cell_w, cell_h)),
            };

            row.push(cell);
        }
        artwork.cells.push(row);
    }

    artwork
}

pub fn render(text: &str, opts: &Options) -> anyhow::Result<Artwork> {
    if text.trim().is_empty() {
        anyhow::bail!("nothing to render: input is empty");
    }

    let font = load_font()?;
    let (cell_w, _) = opts.charset.pixels_per_cell();
    let px = pick_px(&font, text, opts.cols, cell_w);
    let bmp = rasterize(&font, text, px);
    if bmp.width == 0 || bmp.height == 0 {
        anyhow::bail!("nothing to render: the text rasterized to an empty image");
    }

    let mut artwork = downsample(&bmp, opts);
    artwork.trim_end();
    artwork.trim_blank_rows();
    if artwork.is_empty() {
        anyhow::bail!("nothing to render: the text produced no visible pixels");
    }
    Ok(artwork)
}

/// Rasterize `text` into an artwork that is exactly `rows` cells tall.
///
/// Used by FIGlet mode to draw characters its `.flf` font has no glyph for. A
/// literal glyph would occupy a single cell and look tiny beside a six-row
/// banner, so it is rendered as ASCII art at the banner's own height instead.
///
/// The width falls out of the glyph's aspect ratio; the caller gets a
/// rectangular block either way, ready to splice into a banner.
pub fn render_to_rows(text: &str, rows: usize, opts: &Options) -> anyhow::Result<Artwork> {
    if text.trim().is_empty() || rows == 0 {
        anyhow::bail!("nothing to render: input is empty");
    }

    let font = load_font()?;
    let (_, cell_h) = opts.charset.pixels_per_cell();
    let px = pick_px_for_rows(&font, rows, cell_h);
    let bmp = rasterize(&font, text, px);
    if bmp.width == 0 || bmp.height == 0 {
        anyhow::bail!("nothing to render: the text rasterized to an empty image");
    }

    let mut artwork = downsample(&bmp, opts);

    // The caller is splicing this into a fixed-height banner, so the height has
    // to be exact rather than whatever the rounding produced.
    artwork.cells.truncate(rows);
    while artwork.cells.len() < rows {
        artwork.cells.push(Vec::new());
    }

    // Pad out to a rectangle so the following piece starts at the same column on
    // every row.
    let width = artwork.width();
    for row in &mut artwork.cells {
        row.resize(width, Cell::new(' '));
    }
    artwork.trim_end();
    Ok(artwork)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Rgb;

    fn opts(charset: Charset) -> Options {
        Options { charset, cols: 60, ..Default::default() }
    }

    #[test]
    fn block_mode_renders_something_for_ascii() {
        let art = render("A", &opts(Charset::AsciiRamp)).expect("render");
        assert!(art.height() > 0);
        assert!(art.width() > 0);
    }

    #[test]
    fn every_charset_renders() {
        for charset in Charset::ALL {
            let art = render("Hi", &opts(charset))
                .unwrap_or_else(|e| panic!("{} failed: {e}", charset.label()));
            assert!(!art.is_empty(), "{} produced nothing", charset.label());
        }
    }

    #[test]
    fn braille_output_is_only_braille() {
        let art = render("A", &opts(Charset::Braille)).expect("render");
        for cell in art.cells.iter().flatten() {
            let c = cell.ch as u32;
            assert!(
                (0x2800..=0x28FF).contains(&c),
                "unexpected character {:?}",
                cell.ch
            );
        }
    }

    #[test]
    fn quadrant_output_is_only_quadrant_characters() {
        let art = render("A", &opts(Charset::Quadrant)).expect("render");
        for cell in art.cells.iter().flatten() {
            assert!(
                charset::QUADRANT_CHARS.contains(&cell.ch),
                "unexpected character {:?}",
                cell.ch
            );
        }
    }

    #[test]
    fn quadrant_reaches_more_detail_than_a_half_block() {
        // The point of the charset: four sub-cells per character against two,
        // so the same glyph resolves to more distinct block characters.
        let quad = render("A", &opts(Charset::Quadrant)).expect("render");
        let half = render("A", &opts(Charset::HalfBlock)).expect("render");
        let distinct = |a: &Artwork| {
            let mut s: Vec<char> = a.cells.iter().flatten().map(|c| c.ch).collect();
            s.sort_unstable();
            s.dedup();
            s.len()
        };
        assert!(
            distinct(&quad) > distinct(&half),
            "quadrant {} vs half block {}",
            distinct(&quad),
            distinct(&half)
        );
    }

    #[test]
    fn quadrant_does_not_paint_empty_cells() {
        let art = render("A", &opts(Charset::Quadrant)).expect("render");
        assert!(
            art.cells.iter().flatten().any(|c| c.is_blank()),
            "a glyph should leave some cells empty"
        );
    }

    #[test]
    fn half_block_output_is_only_block_characters() {
        let art = render("A", &opts(Charset::HalfBlock)).expect("render");
        for cell in art.cells.iter().flatten() {
            assert!(
                matches!(cell.ch, ' ' | '▀' | '▄' | '█'),
                "unexpected character {:?}",
                cell.ch
            );
        }
    }

    #[test]
    fn half_block_does_not_paint_empty_cells() {
        // Regression: every cell used to be colored, because half-block always
        // has two colors available. A sparse glyph then came out as a solid
        // rectangle with the shape buried inside it.
        let art = render("A", &opts(Charset::HalfBlock)).expect("render");
        let blank = art
            .cells
            .iter()
            .flatten()
            .filter(|c| c.ch == ' ')
            .count();
        assert!(
            blank > art.height() * art.width() / 4,
            "expected plenty of empty cells, got {blank} of {}",
            art.height() * art.width()
        );
    }

    #[test]
    fn half_block_colors_only_the_inked_halves() {
        let art = render("A", &opts(Charset::HalfBlock)).expect("render");
        for cell in art.cells.iter().flatten() {
            if cell.ch == ' ' {
                assert!(cell.fg.is_none() && cell.bg.is_none(), "an empty cell was painted");
            }
            if cell.ch == '█' {
                assert!(cell.fg.is_some(), "a solid block needs a color");
                assert!(cell.bg.is_none(), "a solid block should not need a background");
            }
            if cell.ch == '▀' && cell.bg.is_some() {
                // Two inked halves; the color pass fills both in later.
                assert!(cell.fg.is_some());
            }
        }
    }

    #[test]
    fn half_block_draws_the_lower_half_when_only_that_half_is_inked() {
        // A glyph whose ink sits low in the cell must not be silently dropped.
        let art = render("_", &opts(Charset::HalfBlock)).expect("render");
        let has_lower = art.cells.iter().flatten().any(|c| c.ch == '▄');
        let has_upper = art.cells.iter().flatten().any(|c| c.ch == '▀');
        assert!(has_lower || has_upper, "an underscore produced no blocks");
    }

    #[test]
    fn wider_output_request_produces_more_columns() {
        let narrow = render("Hello", &Options { cols: 40, ..Default::default() }).expect("narrow");
        let wide = render("Hello", &Options { cols: 120, ..Default::default() }).expect("wide");
        assert!(
            wide.width() > narrow.width(),
            "{} should be wider than {}",
            wide.width(),
            narrow.width()
        );
    }

    #[test]
    fn braille_resolves_more_detail_than_the_ascii_ramp() {
        let braille = render("A", &opts(Charset::Braille)).expect("braille");
        let ascii = render("A", &opts(Charset::AsciiRamp)).expect("ascii");
        // Same requested width, but braille packs 2x4 pixels per cell.
        assert!(
            braille.height() >= ascii.height(),
            "braille {} rows vs ascii {} rows",
            braille.height(),
            ascii.height()
        );
    }

    #[test]
    fn empty_input_is_an_error_not_a_panic() {
        assert!(render("", &opts(Charset::Braille)).is_err());
        assert!(render("   ", &opts(Charset::Braille)).is_err());
    }

    #[test]
    fn unicode_and_icons_do_not_panic() {
        // The whole point of block mode: arbitrary Unicode survives.
        let art = render("世界 ", &opts(Charset::Braille)).expect("render");
        assert!(!art.is_empty());
    }

    #[test]
    fn multiline_input_is_taller_than_single_line() {
        let one = render("A", &opts(Charset::Braille)).expect("one");
        let two = render("A\nB", &opts(Charset::Braille)).expect("two");
        assert!(two.height() > one.height());
    }

    #[test]
    fn threshold_changes_braille_density() {
        let sparse = render("A", &Options { charset: Charset::Braille, cols: 60, threshold: 0.9, style: ColorStyle::default() }).expect("sparse");
        let dense = render("A", &Options { charset: Charset::Braille, cols: 60, threshold: 0.1, style: ColorStyle::default() }).expect("dense");

        // Count inked cells rather than braille characters specifically: a
        // fully lit cell is drawn as a block, which still counts as ink.
        let inked = |a: &Artwork| a.cells.iter().flatten().filter(|c| !c.is_blank()).count();
        assert!(
            inked(&dense) > inked(&sparse),
            "a lower threshold should light more of the glyph: {} vs {}",
            inked(&dense),
            inked(&sparse)
        );
    }

    #[test]
    fn half_block_colors_follow_the_style() {
        // Half-block needs the style at generation time because it has to decide
        // *per half pixel*; a cell whose two halves differ carries two colors.
        let red = ColorStyle::Solid(Rgb::new(255, 0, 0));
        let blue = ColorStyle::Solid(Rgb::new(0, 0, 255));

        let a = render("A", &Options { charset: Charset::HalfBlock, style: red, ..Default::default() })
            .expect("red");
        let b = render("A", &Options { charset: Charset::HalfBlock, style: blue, ..Default::default() })
            .expect("blue");

        assert_ne!(
            a.cells.iter().flatten().map(|c| c.fg).collect::<Vec<_>>(),
            b.cells.iter().flatten().map(|c| c.fg).collect::<Vec<_>>(),
        );
        // Cells it did not color are the single-half ones, which the shared
        // colorize pass fills in once the artwork's final layout is known.
        for cell in a.cells.iter().flatten() {
            if let Some(fg) = cell.fg {
                assert_eq!(fg, Rgb::new(255, 0, 0), "{:?} took the wrong color", cell.ch);
            }
        }
    }
}
