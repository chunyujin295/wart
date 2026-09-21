//! Color styles and the pass that bakes them into an [`Artwork`].
//!
//! Color is materialized into the `Artwork` once, by [`colorize`], rather than
//! recomputed by each exporter. Five exporters each re-deriving a gradient would
//! guarantee five subtly different results; one pass means the ANSI, Lua, HTML,
//! SVG, PNG and GUI outputs are identical by construction.

pub mod gradient;

use crate::model::{Artwork, Rgb};
use gradient::Stop;

/// Terminal cells are roughly twice as tall as they are wide. Any gradient math
/// that mixes the two axes has to account for it, or circles come out as
/// ellipses and "45 degrees" is not 45 degrees.
pub const CELL_ASPECT: f32 = 2.0;

#[derive(Debug, Clone, PartialEq)]
pub enum ColorStyle {
    /// One color for the whole artwork.
    Solid(Rgb),
    /// Gradient along a straight axis.
    Linear { stops: Vec<Stop>, angle_deg: f32 },
    /// Hue sweep along an axis.
    Rainbow {
        cycles: f32,
        angle_deg: f32,
        saturation: f32,
        value: f32,
    },
    /// Gradient radiating from a point. `center` is in normalized coordinates.
    Radial { stops: Vec<Stop>, center: (f32, f32) },
}

impl Default for ColorStyle {
    fn default() -> Self {
        ColorStyle::Solid(Rgb::WHITE)
    }
}

impl ColorStyle {
    /// A linear gradient between two colors.
    pub fn linear(from: Rgb, to: Rgb, angle_deg: f32) -> Self {
        ColorStyle::Linear { stops: gradient::even_stops(&[from, to]), angle_deg }
    }

    /// A linear gradient through several evenly spaced colors.
    pub fn linear_multi(colors: &[Rgb], angle_deg: f32) -> Self {
        ColorStyle::Linear { stops: gradient::even_stops(colors), angle_deg }
    }

    pub fn radial(colors: &[Rgb]) -> Self {
        ColorStyle::Radial { stops: gradient::even_stops(colors), center: (0.5, 0.5) }
    }

    pub fn rainbow(angle_deg: f32) -> Self {
        ColorStyle::Rainbow { cycles: 1.0, angle_deg, saturation: 0.85, value: 1.0 }
    }

    /// The color at normalized grid position `(u, v)`, both in `0..=1`.
    ///
    /// `aspect` is the artwork's visual height divided by its width, in pixels
    /// (see [`CELL_ASPECT`]). It only affects the radial style, where treating
    /// the grid as square would visibly squash the rings.
    pub fn at(&self, u: f32, v: f32, aspect: f32) -> Rgb {
        match self {
            ColorStyle::Solid(c) => *c,

            ColorStyle::Linear { stops, angle_deg } => {
                gradient::sample_stops(stops, gradient::axis_t(*angle_deg, u, v))
            }

            ColorStyle::Rainbow { cycles, angle_deg, saturation, value } => {
                let t = gradient::axis_t(*angle_deg, u, v);
                gradient::hsv_to_rgb(t * cycles * 360.0, *saturation, *value)
            }

            ColorStyle::Radial { stops, center } => {
                let dx = u - center.0;
                let dy = (v - center.1) * aspect;
                let d = (dx * dx + dy * dy).sqrt();
                gradient::sample_stops(stops, d)
            }
        }
    }
}

/// Fill in colors for every cell, using each cell's final position.
///
/// Cells that already carry a single color are left alone, which is what lets a
/// generator pre-color part of the artwork without the rest fighting it.
///
/// The exception is half-block cells, which carry *two* colors — one per half
/// pixel. Those must be sampled here rather than at generation time: a generator
/// only knows its own local coordinates, so an icon drawn on the left of a wide
/// gradient would otherwise be given the entire ramp squeezed into its own
/// width. Re-sampling once the artwork is assembled puts every half-block cell
/// on the same gradient as everything around it.
pub fn colorize(art: &mut Artwork, style: &ColorStyle) {
    let (w, h) = (art.width(), art.height());
    if w == 0 || h == 0 {
        return;
    }

    // Map cell indices onto 0..=1. A single row or column would divide by zero,
    // so the midpoint is the only sensible sample in that case.
    let du = if w > 1 { 1.0 / (w - 1) as f32 } else { 0.0 };
    let dv = if h > 1 { 1.0 / (h - 1) as f32 } else { 0.0 };
    let aspect = (h as f32 * CELL_ASPECT) / w as f32;
    // A half-block cell covers two source rows, so its two colors are sampled
    // half a row apart.
    let half_row = dv * 0.5;

    for y in 0..h {
        for x in 0..w {
            let Some(cell) = art.cells.get_mut(y).and_then(|r| r.get_mut(x)) else {
                continue;
            };
            // Blank cells stay blank: coloring a space would add invisible
            // escape sequences to the export and, in half-block mode, paint
            // background where the user expects transparency.
            if cell.ch == ' ' && cell.bg.is_none() {
                continue;
            }

            let u = x as f32 * du;
            let v = y as f32 * dv;

            if cell.bg.is_some() {
                cell.fg = Some(style.at(u, (v - half_row).max(0.0), aspect));
                cell.bg = Some(style.at(u, (v + half_row).min(1.0), aspect));
                continue;
            }
            if cell.fg.is_some() {
                continue;
            }
            cell.fg = Some(style.at(u, v, aspect));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Rgb = Rgb::new(255, 0, 0);
    const BLUE: Rgb = Rgb::new(0, 0, 255);

    #[test]
    fn solid_is_constant_everywhere() {
        let s = ColorStyle::Solid(RED);
        for (u, v) in [(0.0, 0.0), (1.0, 1.0), (0.5, 0.5)] {
            assert_eq!(s.at(u, v, 2.0), RED);
        }
    }

    #[test]
    fn horizontal_linear_runs_left_to_right() {
        let s = ColorStyle::linear(RED, BLUE, 0.0);
        assert_eq!(s.at(0.0, 0.5, 2.0), RED);
        assert_eq!(s.at(1.0, 0.5, 2.0), BLUE);
        assert_eq!(s.at(0.0, 0.0, 2.0), RED, "v must not matter at 0 degrees");
        assert_eq!(s.at(0.0, 1.0, 2.0), RED);
    }

    #[test]
    fn vertical_linear_runs_top_to_bottom() {
        let s = ColorStyle::linear(RED, BLUE, 90.0);
        assert_eq!(s.at(0.5, 0.0, 2.0), RED);
        assert_eq!(s.at(0.5, 1.0, 2.0), BLUE);
    }

    #[test]
    fn colorize_skips_blank_cells() {
        let mut art = Artwork::from_lines(["a b"]);
        colorize(&mut art, &ColorStyle::Solid(RED));
        assert!(art.cells[0][0].fg.is_some(), "letter should be colored");
        assert!(art.cells[0][1].fg.is_none(), "space should stay uncolored");
        assert!(art.cells[0][2].fg.is_some());
    }

    #[test]
    fn colorize_respects_existing_colors() {
        let mut art = Artwork::from_lines(["a"]);
        art.cells[0][0].fg = Some(BLUE);
        colorize(&mut art, &ColorStyle::Solid(RED));
        assert_eq!(art.cells[0][0].fg, Some(BLUE), "generator color must win");
    }

    #[test]
    fn colorize_handles_a_single_cell_without_dividing_by_zero() {
        let mut art = Artwork::from_lines(["x"]);
        colorize(&mut art, &ColorStyle::linear(RED, BLUE, 0.0));
        assert_eq!(art.cells[0][0].fg, Some(RED));
    }

    #[test]
    fn colorize_handles_empty_artwork() {
        let mut art = Artwork::new();
        colorize(&mut art, &ColorStyle::Solid(RED));
        assert!(art.is_empty());
    }

    #[test]
    fn radial_is_symmetric_about_its_center() {
        let s = ColorStyle::radial(&[Rgb::BLACK, Rgb::WHITE]);
        // With aspect 1.0 the four edge midpoints sit at equal distance.
        let left = s.at(0.0, 0.5, 1.0);
        let right = s.at(1.0, 0.5, 1.0);
        let top = s.at(0.5, 0.0, 1.0);
        let bottom = s.at(0.5, 1.0, 1.0);
        assert_eq!(left, right);
        assert_eq!(top, bottom);
        assert_eq!(left, top, "aspect 1.0 should give a circle, not an ellipse");
    }

    #[test]
    fn radial_center_is_the_first_stop() {
        let s = ColorStyle::radial(&[RED, BLUE]);
        assert_eq!(s.at(0.5, 0.5, 2.0), RED);
    }

    #[test]
    fn rainbow_sweeps_the_hue() {
        // Full saturation so the hue sweep is checkable against pure colors.
        let s = ColorStyle::Rainbow {
            cycles: 1.0,
            angle_deg: 0.0,
            saturation: 1.0,
            value: 1.0,
        };
        assert_eq!(s.at(0.0, 0.5, 2.0), Rgb::new(255, 0, 0), "t=0 is red");
        assert_eq!(s.at(0.5, 0.5, 2.0), Rgb::new(0, 255, 255), "t=0.5 is cyan");
        assert_eq!(s.at(1.0, 0.5, 2.0), Rgb::new(255, 0, 0), "a full sweep returns to red");
    }

    #[test]
    fn rainbow_desaturates_without_losing_hue() {
        let s = ColorStyle::rainbow(0.0);
        let c = s.at(0.0, 0.5, 2.0);
        assert_eq!(c.r, 255, "red channel stays dominant");
        assert!(c.g > 0 && c.b > 0, "0.85 saturation lifts the other channels");
    }
}
