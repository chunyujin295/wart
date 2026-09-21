//! Shape-matched character selection.
//!
//! A density ramp asks "how dark is this cell" and answers with a character of
//! matching darkness. That is a lossy question: at the sizes we work at, whole
//! different shapes share a density, so the answer is often wrong and the result
//! reads as mush.
//!
//! This asks the better question — "what does this cell *look like*" — by
//! rasterizing every candidate character and comparing ink distributions. Each
//! cell and each candidate is reduced to the same small signature: the fraction
//! of ink falling in each of an 8-region grid. The character whose signature is
//! nearest wins.
//!
//! The technique follows Alex Harri's "ASCII characters are not pixels"
//! (<https://alexharri.com/blog/ascii-rendering>), simplified: no k-d tree (our
//! candidate set is ~95 glyphs, not thousands), and no contrast enhancement.

use fontdue::Font;
use std::sync::OnceLock;

/// Signature grid. Two columns by four rows, which matches the roughly 1:2
/// aspect of a terminal cell — the same proportions the glyphs are drawn into.
const SIG_W: usize = 2;
const SIG_H: usize = 4;

/// Pixels per signature sample when rasterizing a candidate glyph. Purely an
/// accuracy knob for the glyph side; the comparison happens after downsampling.
const CAND_SCALE: usize = 6;
const CAND_W: usize = SIG_W * CAND_SCALE;
const CAND_H: usize = SIG_H * CAND_SCALE;

/// Size the reference metrics are taken at, before scaling to the canvas.
const REFERENCE_PX: f32 = 100.0;

/// The characters the matcher chooses between: printable ASCII, which is also
/// the vocabulary a FIGlet font draws with.
fn candidate_chars() -> Vec<char> {
    (' '..='~').collect()
}

struct Candidate {
    ch: char,
    signature: Vec<f32>,
}

pub struct Matcher {
    candidates: Vec<Candidate>,
}

impl Matcher {
    fn new(font: &Font) -> Self {
        let candidates = candidate_chars()
            .into_iter()
            .map(|ch| Candidate { ch, signature: glyph_signature(font, ch) })
            .collect();
        Self { candidates }
    }

    /// The character whose ink distribution is closest to `signature`.
    ///
    /// Plain Euclidean distance: the signature is small enough that a linear
    /// scan beats anything cleverer, and the candidate set is fixed.
    pub fn best(&self, signature: &[f32]) -> char {
        let mut best = (' ', f32::INFINITY);
        for candidate in &self.candidates {
            let d: f32 = candidate
                .signature
                .iter()
                .zip(signature)
                .map(|(a, b)| (a - b) * (a - b))
                .sum();
            if d < best.1 {
                best = (candidate.ch, d);
            }
        }
        best.0
    }
}

/// The process-wide matcher. Depends only on the bundled font and the fixed
/// signature geometry, so it is built once and shared.
pub fn matcher() -> Option<&'static Matcher> {
    static MATCHER: OnceLock<Option<Matcher>> = OnceLock::new();
    MATCHER
        .get_or_init(|| {
            let font = fontdue::Font::from_bytes(
                crate::assets::NERD_FONT_TTF,
                fontdue::FontSettings::default(),
            )
            .ok()?;
            Some(Matcher::new(&font))
        })
        .as_ref()
}

/// Rasterize one character into a canvas representing a single terminal cell,
/// then reduce it to a signature.
fn glyph_signature(font: &Font, ch: char) -> Vec<f32> {
    // Scale the font so its line box exactly fills the canvas height, which is
    // the same relationship a cell has to a line of text.
    let Some(probe) = font.horizontal_line_metrics(REFERENCE_PX) else {
        return vec![0.0; SIG_W * SIG_H];
    };
    let line = probe.ascent - probe.descent;
    if line <= 0.0 {
        return vec![0.0; SIG_W * SIG_H];
    }
    let px = REFERENCE_PX * CAND_H as f32 / line;
    let Some(metrics_line) = font.horizontal_line_metrics(px) else {
        return vec![0.0; SIG_W * SIG_H];
    };

    let (m, bitmap) = font.rasterize(ch, px);
    let mut canvas = vec![0.0f32; CAND_W * CAND_H];

    let baseline = metrics_line.ascent.round() as i32;
    // fontdue reports `ymin` as the offset of the bitmap's bottom edge above the
    // baseline, and stores the buffer top row first.
    let top = baseline - m.ymin - m.height as i32;
    // Centre on the advance so narrow and wide glyphs are compared fairly.
    let left = ((CAND_W as f32 - m.advance_width) * 0.5).round() as i32 + m.xmin;

    for gy in 0..m.height {
        for gx in 0..m.width {
            let coverage = bitmap[gy * m.width + gx] as f32 / 255.0;
            if coverage <= 0.0 {
                continue;
            }
            let (cx, cy) = (left + gx as i32, top + gy as i32);
            if cx < 0 || cy < 0 || cx >= CAND_W as i32 || cy >= CAND_H as i32 {
                continue;
            }
            canvas[cy as usize * CAND_W + cx as usize] = coverage;
        }
    }

    resample(&canvas, CAND_W, CAND_H)
}

/// Reduce a `w` by `h` coverage buffer to a `SIG_W` by `SIG_H` signature.
///
/// Also used on the image side, so both vectors describe the same thing:
/// "what fraction of the ink sits in each of these eight regions".
pub fn resample(coverage: &[f32], w: usize, h: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; SIG_W * SIG_H];
    if w == 0 || h == 0 {
        return out;
    }
    for sy in 0..SIG_H {
        // Integer banding, so every source pixel is counted exactly once even
        // when the region does not divide evenly.
        let y0 = sy * h / SIG_H;
        let y1 = ((sy + 1) * h / SIG_H).max(y0 + 1).min(h);
        for sx in 0..SIG_W {
            let x0 = sx * w / SIG_W;
            let x1 = ((sx + 1) * w / SIG_W).max(x0 + 1).min(w);

            let mut sum = 0.0;
            let mut n = 0;
            for y in y0..y1 {
                for x in x0..x1 {
                    sum += coverage[y * w + x];
                    n += 1;
                }
            }
            out[sy * SIG_W + sx] = if n == 0 { 0.0 } else { sum / n as f32 };
        }
    }
    out
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_matcher_builds_from_the_bundled_font() {
        assert!(matcher().is_some(), "matcher failed to build");
    }

    #[test]
    fn every_candidate_has_a_full_signature() {
        let m = matcher().expect("matcher");
        assert!(m.candidates.len() > 90, "expected printable ASCII");
        for c in &m.candidates {
            assert_eq!(c.signature.len(), SIG_W * SIG_H, "{:?} is short", c.ch);
            assert!(
                c.signature.iter().all(|v| (0.0..=1.0).contains(v)),
                "{:?} has out-of-range coverage",
                c.ch
            );
        }
    }

    #[test]
    fn a_blank_cell_picks_a_blank_character() {
        let m = matcher().expect("matcher");
        let blank = vec![0.0; SIG_W * SIG_H];
        assert_eq!(m.best(&blank), ' ', "an empty cell should be empty");
    }

    #[test]
    fn a_filled_cell_picks_a_dense_character() {
        let m = matcher().expect("matcher");
        let full = vec![1.0; SIG_W * SIG_H];
        let best = m.best(&full);
        // Anything visually solid will do; the point is that it is not blank.
        assert_ne!(best, ' ', "a full cell should not be drawn with a space");
    }

    #[test]
    fn the_top_half_and_the_bottom_half_pick_different_characters() {
        // The whole reason for shape matching: two cells with the same amount of
        // ink but different shapes must not come out the same.
        let m = matcher().expect("matcher");
        let mut top = vec![0.0; SIG_W * SIG_H];
        let mut bottom = vec![0.0; SIG_W * SIG_H];
        for y in 0..SIG_H / 2 {
            for x in 0..SIG_W {
                top[y * SIG_W + x] = 1.0;
            }
        }
        for y in SIG_H / 2..SIG_H {
            for x in 0..SIG_W {
                bottom[y * SIG_W + x] = 1.0;
            }
        }
        assert_ne!(m.best(&top), m.best(&bottom));
    }

    #[test]
    fn a_density_ramp_would_confuse_these_but_shape_matching_does_not() {
        // A left-heavy and a right-heavy cell carry identical ink. A ramp has
        // one answer for both; shape matching must distinguish them.
        let m = matcher().expect("matcher");
        let mut left = vec![0.0; SIG_W * SIG_H];
        let mut right = vec![0.0; SIG_W * SIG_H];
        for y in 0..SIG_H {
            left[y * SIG_W] = 1.0;
            right[y * SIG_W + (SIG_W - 1)] = 1.0;
        }
        assert_ne!(m.best(&left), m.best(&right));
    }

    #[test]
    fn resample_averages_the_whole_region() {
        // A uniform buffer must give a uniform signature.
        let buf = vec![0.5f32; 8 * 16];
        let sig = resample(&buf, 8, 16);
        for v in sig {
            assert!((v - 0.5).abs() < 1e-6, "got {v}");
        }
    }

    #[test]
    fn resample_covers_every_source_pixel() {
        // Integer banding must not drop rows or columns when the region does not
        // divide evenly by the signature size.
        let (w, h) = (7, 13);
        let buf = vec![1.0f32; w * h];
        let sig = resample(&buf, w, h);
        for v in sig {
            assert!((v - 1.0).abs() < 1e-6, "a pixel was skipped: {v}");
        }
    }

    #[test]
    fn resample_of_nothing_is_nothing() {
        assert!(resample(&[], 0, 0).iter().all(|v| *v == 0.0));
    }
}
