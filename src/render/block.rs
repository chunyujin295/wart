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
use crate::model::{Artwork, Cell, Rgb};

/// How far apart two halves' sampled colors may be and still be treated as one
/// color.
///
/// A half-block cell carries two colors, which is the whole point of the
/// charset — but that only buys something when the two actually differ. A solid
/// style, and the horizontal gradient most artwork uses, sample the same color
/// for both halves, and `▀` over a background is then strictly worse than one
/// `█`: the stroke reads at half height instead of full, and the cell needs a
/// terminal that draws background colors. The cutoff is about a
/// just-noticeable difference, so anything this close would not have been seen
/// as two colors anyway.
const COLOR_EPSILON: u8 = 8;

/// Reference size the ink is measured at before the rasterization size is
/// solved for.
///
/// The ink box grows linearly with the font size, so measuring once at any size
/// and scaling is exact up to the rounding of that one measurement — the same
/// trade the width-based sizing in [`pick_px`] makes. Larger is more precise and
/// costs nothing but the arithmetic.
const REFERENCE_PX: f32 = 100.0;

/// How much wider than it is tall a figure may be drawn, in character cells.
///
/// The height is what the caller asked for, but the width has to be bounded
/// somewhere. A glyph whose ink is a hairline — an em dash, a rule — has an
/// aspect ratio of tens to one, and fitting its height to ten rows draws it a
/// hundred and fifty columns wide, which is not an emblem but a rule across the
/// terminal. Four columns per row leaves a good margin around the square grid a
/// Nerd Font icon is designed on, so only genuinely thin glyphs are held back,
/// and those are held back by shrinking: an em dash comes out a short dash
/// rather than a long one squeezed.
const MAX_ICON_ASPECT: f32 = 4.0;

/// Whether two sampled colors are close enough to draw as one.
fn same_color(a: Rgb, b: Rgb) -> bool {
    let d = |x: u8, y: u8| x.abs_diff(y);
    d(a.r, b.r).max(d(a.g, b.g)).max(d(a.b, b.b)) <= COLOR_EPSILON
}

pub struct Options {
    pub charset: Charset,
    /// Desired output width in character cells. The rasterization size is
    /// derived from this, so the same text always comes out the same width
    /// regardless of charset.
    pub cols: usize,
    /// Coverage cutoff for the charsets that threshold rather than ramp: a
    /// sub-cell counts as ink when any pixel in it reaches this. See
    /// [`Charset::uses_threshold`].
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

/// Rasterize `text` so that its ink fills a block `rows * cell_h` pixels tall,
/// cropped to the ink's bounding box, or a
/// [`MAX_ICON_ASPECT`] wide as the case may be.
///
/// Sizing off the **ink** rather than off the font's line box is what makes the
/// output the size the caller asked for. A glyph's ink sits well inside the em
/// box it is drawn in — for a Nerd Font icon it is about half of it — so fitting
/// the line box to a target spends half the budget on leading and half on the
/// drawing. At the sizes this is used at, that is the difference between an icon
/// that can be recognised and a speck.
///
/// Cropping to the ink hands the rest of the budget to the drawing and centres
/// it, so the caller's row count is met from the top row to the bottom one.
fn rasterize_fitted(
    font: &Font,
    text: &str,
    rows: usize,
    cell_w: usize,
    cell_h: usize,
) -> anyhow::Result<Bitmap> {
    let target_h = (rows * cell_h).max(1);

    // Measure once at the reference size, then scale: both extents grow
    // linearly with the font size, so one measurement is enough.
    let (ink_w, ink_h) = ink_extent(font, text, REFERENCE_PX);
    anyhow::ensure!(ink_h > 0 && ink_w > 0, "nothing to render: the text has no ink");

    // Height sets the figure's size, and width caps it: whichever the ink runs
    // out of room in first is the one that decides.
    let max_w = (rows as f32 * MAX_ICON_ASPECT * cell_w as f32).round().max(cell_w as f32);
    let px = (REFERENCE_PX * target_h as f32 / ink_h as f32)
        .min(REFERENCE_PX * max_w / ink_w as f32)
        // A rasterization far outside this range is never useful, and a zero
        // would divide by zero downstream.
        .clamp(4.0, 2048.0);
    let bmp = rasterize(font, text, px);

    let (x0, y0, w, h) = ink_bounds(&bmp)
        .ok_or_else(|| anyhow::anyhow!("nothing to render: the text has no ink"))?;
    Ok(crop_centred(&bmp, x0, y0, w, h, target_h))
}

/// Bounding box of the inked pixels, as `(x, y, width, height)`.
///
/// The cutoff is a hair above zero rather than zero: a rasterizer spreads a
/// glyph's edge over a fringe of near-zero coverage, and measuring to the last
/// hint of it would size the drawing a few percent small. This sits below the
/// noise in the result and above a rounding artefact.
fn ink_bounds(bmp: &Bitmap) -> Option<(usize, usize, usize, usize)> {
    const TINY: f32 = 0.02;

    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
    for y in 0..bmp.height {
        for x in 0..bmp.width {
            if bmp.at(x, y) > TINY {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if x0 == usize::MAX {
        return None;
    }
    Some((x0, y0, x1 - x0 + 1, y1 - y0 + 1))
}

/// Width and height, in pixels, of the ink [`rasterize`] would draw for `text`
/// at font size `px`.
///
/// Read from the glyph metrics rather than from a bitmap: measuring by drawing
/// costs a raster of the whole line, and a line of a hundred icons is an
/// ordinary input here, so a measurement that scales with its length would be
/// paying a hundred times over for an answer about one line's height.
///
/// A glyph's box is reported against the baseline and the pen — `ymin` is its
/// bottom edge above the baseline, `height` how far it reaches above that,
/// `xmin` and `width` its horizontal extent from where the pen was — so the
/// ink's extent follows from stepping the pen along each line exactly as
/// [`rasterize`] does. Nothing is drawn, so nothing can be clipped: a glyph
/// whose ink reaches past the em box measures larger here than it draws, which
/// costs the crop below a row it then centres away.
fn ink_extent(font: &Font, text: &str, px: f32) -> (usize, usize) {
    let line_height = font
        .horizontal_line_metrics(px)
        .map_or(px * 1.2, |m| m.new_line_size)
        .round() as i32;

    let mut lines = 0i32;
    let mut widest = 0i32;
    let (mut top, mut bottom) = (i32::MIN, i32::MAX);

    for line in text.split('\n') {
        lines += 1;
        let (mut pen, mut left, mut right) = (0i32, i32::MAX, i32::MIN);
        for ch in line.chars() {
            let m = font.metrics(ch, px);
            if m.height > 0 {
                top = top.max(m.ymin + m.height as i32);
                bottom = bottom.min(m.ymin);
                left = left.min(pen + m.xmin);
                right = right.max(pen + m.xmin + m.width as i32);
            }
            pen += m.advance_width.round() as i32;
        }
        if right != i32::MIN {
            // Negative side bearings are clipped away by the raster, so the
            // line starts at the left edge of the bitmap either way.
            widest = widest.max(right - left.max(0));
        }
    }

    if top == i32::MIN {
        return (0, 0);
    }
    let w = widest.max(0) as usize;
    // The first line's box, plus the leading each following line adds.
    let h = ((top - bottom) + (lines - 1) * line_height).max(0) as usize;
    (w, h)
}

/// Copy the `w` by `h` region at `(x0, y0)` into a `w` by `out_h` bitmap,
/// centred vertically.
///
/// `out_h` is the caller's row budget, spent exactly rather than approximately,
/// so the ink sits in the middle of its block instead of being pinned to the top
/// of a taller one. A region taller than `out_h` is trimmed evenly from both
/// ends: truncating it at the bottom alone is what used to remove the lower half
/// of any figure that overflowed.
fn crop_centred(bmp: &Bitmap, x0: usize, y0: usize, w: usize, h: usize, out_h: usize) -> Bitmap {
    let out_h = out_h.max(1);
    let mut out = Bitmap::new(w, out_h);

    let src_y = y0 + h.saturating_sub(out_h) / 2;
    let dst_y = out_h.saturating_sub(h) / 2;
    for y in 0..h.min(out_h) {
        for x in 0..w {
            out.coverage[(dst_y + y) * w + x] = bmp.at(x0 + x, src_y + y);
        }
    }
    out
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

/// Strongest coverage anywhere in a sub-block of the bitmap.
fn block_peak(bmp: &Bitmap, x0: usize, y0: usize, w: usize, h: usize) -> f32 {
    let mut peak = 0.0f32;
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            peak = peak.max(bmp.at(x, y));
        }
    }
    peak
}

/// Whether a sub-block of the bitmap holds ink.
///
/// The test is on the **strongest** pixel in the block, not on its average. At
/// the sizes these charsets are used at a stroke is routinely thinner than the
/// cell it runs through, and an average then reports the stroke as a fraction of
/// a cell — a light shade character, which at that size reads as empty. A pixel
/// past the cutoff means a stroke comes through here, and a stroke is drawn
/// solid at whatever resolution the charset has.
///
/// Strictly past it, so that a cutoff of zero still means "some ink" rather than
/// "every cell, including the empty ones".
fn has_ink(bmp: &Bitmap, x0: usize, y0: usize, w: usize, h: usize, threshold: f32) -> bool {
    block_peak(bmp, x0, y0, w, h) > threshold
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
                            *dot = has_ink(bmp, x0 + col, y0 + row_idx, 1, 1, opts.threshold);
                        }
                    }
                    Cell::new(charset::braille_cell(bits))
                }

                Charset::HalfBlock => {
                    let half = (cell_h / 2).max(1);
                    let t = opts.threshold;
                    let top = has_ink(bmp, x0, y0, cell_w, half, t);
                    let bottom = has_ink(bmp, x0, y0 + half, cell_w, cell_h - half, t);

                    let u = norm(cx, cols);
                    // The cell covers two source rows, so sample the gradient at
                    // both of them rather than once at the center.
                    let v_top = norm(cy * cell_h, rows * cell_h);
                    let v_bottom = norm(cy * cell_h + half, rows * cell_h);

                    // Half-block always has two colors available, so it is
                    // tempting to color every cell — but an empty cell has no
                    // ink in either half, and painting it turns sparse art into
                    // a solid rectangle. Check for ink first.
                    match (top, bottom) {
                        (false, false) => Cell::new(' '),
                        // Only one half is inked, so the cell is that half block
                        // and the color pass fills in its foreground.
                        (true, false) => Cell::new('▀'),
                        (false, true) => Cell::new('▄'),
                        (true, true) => {
                            let c_top = opts.style.at(u, v_top, aspect);
                            let c_bottom = opts.style.at(u, v_bottom, aspect);

                            // The two halves are only worth splitting when they
                            // carry two different colors. When they do not — a
                            // solid style, or the horizontal gradient most
                            // artwork uses — a stroke through the cell is one
                            // full block rather than two half-height strips.
                            if same_color(c_top, c_bottom) {
                                Cell { ch: '█', fg: Some(c_top), bg: None }
                            } else {
                                // For an upper half block the *foreground* paints
                                // the top half and the background shows through
                                // below. The color pass re-samples both once the
                                // cell's final position is known.
                                Cell { ch: '▀', fg: Some(c_top), bg: Some(c_bottom) }
                            }
                        }
                    }
                }

                Charset::Blocks => {
                    // Solid or nothing. One cell is the smallest thing this
                    // charset can draw, so a stroke through a cell is a full
                    // block: the shade ramp it used to use rendered a
                    // one-pixel line as `░`, which is to say invisible, and
                    // left a ghost row wherever the fringe of a thicker line
                    // landed in a cell of its own.
                    let ch = if has_ink(bmp, x0, y0, cell_w, cell_h, opts.threshold) {
                        '█'
                    } else {
                        ' '
                    };
                    Cell::new(ch)
                }

                Charset::AsciiRamp => {
                    let cov = block_average(bmp, x0, y0, cell_w, cell_h);
                    Cell::new(charset::ramp_char(charset::ASCII_RAMP, cov))
                }

                Charset::Quadrant => {
                    // Four sub-cells per character, each a real 1x2 block of
                    // source pixels. All sixteen inked combinations exist as
                    // quarter-block characters, so the silhouette is exact and
                    // the uninked quadrants stay transparent — no background
                    // color required.
                    let hw = (cell_w / 2).max(1);
                    let hh = (cell_h / 2).max(1);
                    let t = opts.threshold;
                    let mut mask = 0u8;
                    if has_ink(bmp, x0, y0, hw, hh, t) {
                        mask |= 0b0001; // upper left
                    }
                    if has_ink(bmp, x0 + hw, y0, cell_w - hw, hh, t) {
                        mask |= 0b0010; // upper right
                    }
                    if has_ink(bmp, x0, y0 + hh, hw, cell_h - hh, t) {
                        mask |= 0b0100; // lower left
                    }
                    if has_ink(bmp, x0 + hw, y0 + hh, cell_w - hw, cell_h - hh, t) {
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
/// banner, so it is rendered as ASCII art instead — see [`Options`] describing
/// the row count the caller asks for, which FIGlet mode sets from the icon
/// style's scale rather than from the banner's own height.
///
/// The width falls out of the glyph's aspect ratio; the caller gets a
/// rectangular block either way, ready to splice into a banner.
pub fn render_to_rows(text: &str, rows: usize, opts: &Options) -> anyhow::Result<Artwork> {
    if text.trim().is_empty() || rows == 0 {
        anyhow::bail!("nothing to render: input is empty");
    }

    let font = load_font()?;
    let (cell_w, cell_h) = opts.charset.pixels_per_cell();
    let bmp = rasterize_fitted(&font, text, rows, cell_w, cell_h)?;
    if bmp.width == 0 || bmp.height == 0 {
        anyhow::bail!("nothing to render: the text rasterized to an empty image");
    }

    let mut artwork = downsample(&bmp, opts);

    // The caller is splicing this into a fixed-height banner, so the height has
    // to be exact. The crop above spends the budget on the ink, which leaves
    // exactly `rows` cells; this is the belt to that pair of braces, since a
    // short block would shift every following piece on the line.
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

    // --- inline-glyph geometry -------------------------------------------
    //
    // These run through [`render_to_rows`], which FIGlet mode uses to draw the
    // characters its font has no glyph for.

    /// A Nerd Font icon that is three thin horizontal bars (`fa-bars`): the
    /// shape a density ramp renders as nothing at all.
    const BARS: char = '\u{f0c9}';

    fn rows_opts(charset: Charset) -> Options {
        Options { charset, cols: 0, threshold: 0.5, style: ColorStyle::default() }
    }

    /// First and last row of the artwork that holds any ink.
    fn ink_row_span(art: &Artwork) -> (usize, usize) {
        let inked = |y: usize| art.cells[y].iter().any(|c| !c.is_blank());
        let rows: Vec<usize> = (0..art.height()).filter(|&y| inked(y)).collect();
        (rows.first().copied().unwrap_or(0), rows.last().copied().unwrap_or(0))
    }

    #[test]
    fn the_fitted_raster_spends_the_whole_row_budget_on_ink() {
        // The bug this replaced: the font's line box was fitted to the budget
        // instead of the ink, and the ink is about half the line box, so a
        // figure asked for 12 rows came out 12 pixels in a 24-pixel-tall box.
        let font = load_font().expect("font");
        for charset in [Charset::Blocks, Charset::HalfBlock, Charset::Quadrant] {
            let (cell_w, cell_h) = charset.pixels_per_cell();
            let rows = 12;
            let bmp = rasterize_fitted(&font, &BARS.to_string(), rows, cell_w, cell_h)
                .expect("fit");

            assert_eq!(bmp.height, rows * cell_h, "{:?}: wrong block height", charset);

            let (_, y0, _, ink_h) = ink_bounds(&bmp).expect("ink");
            assert!(ink_h >= bmp.height - 2, "{:?}: ink is {ink_h} of {}", charset, bmp.height);
            // Centred rather than pinned to the top: truncating from the bottom
            // is what used to remove the lower half of an overflowing figure.
            let below = bmp.height - (y0 + ink_h);
            assert!(
                y0 <= 1 && below <= 1,
                "{:?}: ink sits {y0} from the top, {below} from the bottom",
                charset
            );
        }
    }

    #[test]
    fn a_fitted_icon_spans_the_rows_it_is_given() {
        let art = render_to_rows(&BARS.to_string(), 10, &rows_opts(Charset::Blocks))
            .expect("render");
        let (first, last) = ink_row_span(&art);
        // The icon is not a solid rectangle, so not every row between the bars
        // is inked; what the fit owes the caller is a figure that reaches the
        // first row and the last one.
        assert!(
            first <= 1 && last >= art.height() - 2,
            "the figure runs from row {first} to row {last} of {}",
            art.height()
        );
        assert!(art.width() >= 8, "a {}-column figure is still a speck", art.width());
    }

    #[test]
    fn the_measured_extent_matches_what_the_raster_draws() {
        // The fit is solved from the metrics, so the two have to agree — this
        // is the assumption the sizing rests on, and a drift here would show up
        // as an icon that never quite fills its block.
        let font = load_font().expect("font");
        for ch in [BARS, '\u{f067}', 'A', 'g', '\u{eb99}'] {
            for px in [20.0f32, 60.0, 140.0] {
                let (w, h) = ink_extent(&font, &ch.to_string(), px);
                let bmp = rasterize(&font, &ch.to_string(), px);
                let (_, _, drawn_w, drawn_h) = ink_bounds(&bmp).expect("ink");
                let (dw, dh) = (w as i32 - drawn_w as i32, h as i32 - drawn_h as i32);
                assert!(
                    dw.abs() <= 2 && dh.abs() <= 2,
                    "{ch:?} at {px}: measured {w}x{h}, drew {drawn_w}x{drawn_h}"
                );
            }
        }
    }

    #[test]
    fn a_hairline_glyph_is_not_magnified_into_a_rule() {
        // An em dash is a couple of pixels tall and a hundred wide. Fitting its
        // height to the block drew it as a rule across the terminal — 150
        // columns of solid block — so the width caps the size and it comes out
        // a short thick dash instead.
        let art = render_to_rows("\u{2014}", 10, &rows_opts(Charset::Blocks)).expect("render");
        assert!(
            art.width() <= 10 * MAX_ICON_ASPECT as usize,
            "an em dash came out {} columns wide",
            art.width()
        );
        assert!(art.width() > 8, "the dash is {} columns, too small to read", art.width());
    }

    #[test]
    fn a_zero_cutoff_still_excludes_the_empty_parts() {
        // `has_ink` compares strictly, so a cutoff of zero means "any ink at
        // all" rather than "every cell, inked or not".
        let empty = Bitmap::new(4, 4);
        assert!(!has_ink(&empty, 0, 0, 4, 4, 0.0));

        let mut faint = Bitmap::new(4, 4);
        faint.coverage[0] = 0.01;
        assert!(has_ink(&faint, 0, 0, 4, 4, 0.0), "a faint pixel is still ink");
        assert!(!has_ink(&faint, 0, 0, 4, 4, 0.02), "and it is below a higher cutoff");
    }

    #[test]
    fn a_thin_bar_is_drawn_as_a_solid_run_not_a_shade() {
        // The complaint this charset was rebuilt around. Three bars a few pixels
        // thick land in cells whose *average* coverage is a quarter to a half,
        // and the old five-step ramp turned that into `░` and `▒` — a line that
        // is there in the code and invisible on the screen.
        let art = render_to_rows(&BARS.to_string(), 10, &rows_opts(Charset::Blocks))
            .expect("render");
        for cell in art.cells.iter().flatten() {
            assert!(matches!(cell.ch, ' ' | '█'), "a shade leaked in: {:?}", cell.ch);
        }
        let longest = art
            .cells
            .iter()
            .map(|r| r.iter().filter(|c| c.ch == '█').count())
            .max()
            .unwrap_or(0);
        assert!(longest >= 8, "no bar was drawn solid; longest run is {longest}");
    }

    #[test]
    fn the_cutoff_controls_how_much_of_a_glyph_is_ink() {
        // Blocks is binary now, so the cutoff is the only thing setting its
        // weight — which is why the UI offers it for this charset.
        let count = |threshold: f32| {
            let o = Options { charset: Charset::Blocks, cols: 40, threshold, ..Default::default() };
            render("A", &o)
                .expect("render")
                .cells
                .iter()
                .flatten()
                .filter(|c| !c.is_blank())
                .count()
        };
        assert!(
            count(0.2) > count(0.8),
            "a lower cutoff must ink more of the glyph: {} vs {}",
            count(0.2),
            count(0.8)
        );
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
    fn block_output_is_only_full_blocks_and_spaces() {
        // Blocks is a silhouette: one cell is its smallest unit, so a cell is
        // either filled or empty and there are no shades in between.
        let art = render("Am", &opts(Charset::Blocks)).expect("render");
        for cell in art.cells.iter().flatten() {
            assert!(matches!(cell.ch, ' ' | '█'), "unexpected character {:?}", cell.ch);
        }
        assert!(art.cells.iter().flatten().any(|c| c.ch == '█'), "nothing was drawn");
    }

    #[test]
    fn half_block_splits_a_cell_only_to_carry_two_colors() {
        // With one color there is nothing for the two halves to say, and a full
        // block says "ink" better: the stroke keeps its height, and the cell no
        // longer depends on the terminal drawing background colors.
        let solid = ColorStyle::Solid(Rgb::new(255, 0, 0));
        let half = Options { charset: Charset::HalfBlock, style: solid, ..Default::default() };
        let art = render("Am", &half).expect("render");
        for cell in art.cells.iter().flatten() {
            assert!(
                cell.fg.is_none() || cell.bg.is_none(),
                "{:?} was split between two colors that are the same",
                cell.ch
            );
        }
    }

    #[test]
    fn half_block_keeps_both_colors_where_the_gradient_crosses_the_cell() {
        // The converse: two colors that genuinely differ are what the charset
        // exists for, so they must survive.
        let vertical = ColorStyle::linear(Rgb::new(255, 0, 0), Rgb::new(0, 0, 255), 90.0);
        let art = render_to_rows(
            "A",
            10,
            &Options { charset: Charset::HalfBlock, cols: 0, threshold: 0.5, style: vertical },
        )
        .expect("render");
        let split = art
            .cells
            .iter()
            .flatten()
            .filter(|c| c.fg.is_some() && c.bg.is_some())
            .count();
        assert!(split > 0, "no cell carried the gradient's two colors");
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

