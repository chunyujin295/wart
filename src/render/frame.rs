//! Decorative borders around an artwork.
//!
//! Applied to the [`Artwork`] *before* the color pass, so the frame takes part
//! in the gradient instead of needing a color of its own — a horizontal gradient
//! then runs from the left border through the art to the right border.
//!
//! Every glyph used here is present in the bundled Nerd Font, so a frame exports
//! cleanly to PNG and to any terminal with a Nerd Font installed.

use crate::model::{Artwork, Cell};

/// Which character set to draw the border with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrameKind {
    #[default]
    None,
    /// `┌ ─ ┐ │ └ ┘` — the plain box.
    Light,
    /// `╭ ─ ╮ │ ╰ ╯` — same but with softened corners.
    Rounded,
    /// `┏ ━ ┓ ┃ ┗ ┛` — a heavier weight for titles.
    Heavy,
    /// `╔ ═ ╗ ║ ╚ ╝` — the classic double rule.
    Double,
    /// Powerline half-circles down each side, forming a rounded band. This is
    /// the style that is uniquely Nerd Font rather than plain Unicode.
    Powerline,
}

struct Glyphs {
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
    horizontal: char,
    /// Left and right edges are separate: a plain box uses the same glyph on
    /// both sides, but the Powerline caps are mirror images of each other.
    left_edge: char,
    right_edge: char,
}

/// Powerline caps live in the private use area, so they only render with a Nerd
/// Font — which is the point, and also why they are opt-in.
const CAP_LEFT: char = '\u{e0b6}';
const CAP_RIGHT: char = '\u{e0b4}';

impl FrameKind {
    pub const ALL: [FrameKind; 6] = [
        FrameKind::None,
        FrameKind::Light,
        FrameKind::Rounded,
        FrameKind::Heavy,
        FrameKind::Double,
        FrameKind::Powerline,
    ];

    /// Stable English name, for test messages. UI labels come from `i18n` so
    /// they can be translated.
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            FrameKind::None => "None",
            FrameKind::Light => "Light",
            FrameKind::Rounded => "Rounded",
            FrameKind::Heavy => "Heavy",
            FrameKind::Double => "Double",
            FrameKind::Powerline => "Powerline",
        }
    }

    fn glyphs(self) -> Option<Glyphs> {
        // Signature: corners, horizontal rule, then the side glyph.
        let plain = |top_left, top_right, bottom_left, bottom_right, horizontal, side| {
            Some(Glyphs {
                top_left,
                top_right,
                bottom_left,
                bottom_right,
                horizontal,
                left_edge: side,
                right_edge: side,
            })
        };
        match self {
            FrameKind::None => None,
            FrameKind::Light => plain('┌', '┐', '└', '┘', '─', '│'),
            FrameKind::Rounded => plain('╭', '╮', '╰', '╯', '─', '│'),
            FrameKind::Heavy => plain('┏', '┓', '┗', '┛', '━', '┃'),
            FrameKind::Double => plain('╔', '╗', '╚', '╝', '═', '║'),
            // Each cap is a filled half circle, so a column of them forms a
            // smoothed edge. They mirror, hence distinct left and right glyphs,
            // and there are no corners or rules: every row is just caps.
            FrameKind::Powerline => Some(Glyphs {
                top_left: CAP_LEFT,
                top_right: CAP_RIGHT,
                bottom_left: CAP_LEFT,
                bottom_right: CAP_RIGHT,
                horizontal: ' ',
                left_edge: CAP_LEFT,
                right_edge: CAP_RIGHT,
            }),
        }
    }

    /// Powerline has no top or bottom rule, only the side caps.
    fn has_rules(self) -> bool {
        !matches!(self, FrameKind::Powerline | FrameKind::None)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Options {
    pub kind: FrameKind,
    /// Blank cells between the art and the border, on all four sides.
    pub padding: usize,
}

/// Padding above this stops being decoration and starts being a lot of nothing.
pub const MAX_PADDING: usize = 4;

/// Wrap `art` in a border, returning a new artwork.
pub fn apply(art: &Artwork, opts: &Options) -> Artwork {
    let Some(glyphs) = opts.kind.glyphs() else {
        return art.clone();
    };
    let width = art.width();
    if art.is_empty() || width == 0 {
        return art.clone();
    }

    let pad = opts.padding.min(MAX_PADDING);
    // Interior width: the art, plus the padding on both sides. The border itself
    // adds one column each side on top of this.
    let inner = width + pad * 2;
    let mut cells: Vec<Vec<Cell>> = Vec::with_capacity(art.height() + pad * 2 + 2);

    let rule_row = |left: char, fill: char, right: char| -> Vec<Cell> {
        let mut row = Vec::with_capacity(inner + 2);
        row.push(Cell::new(left));
        row.extend(std::iter::repeat_n(Cell::new(fill), inner));
        row.push(Cell::new(right));
        row
    };

    let side_row = |body: &[Cell], glyphs: &Glyphs| -> Vec<Cell> {
        let mut row = Vec::with_capacity(inner + 2);
        row.push(Cell::new(glyphs.left_edge));
        row.extend(std::iter::repeat_n(Cell::new(' '), pad));
        row.extend_from_slice(body);
        // Rows are ragged before framing, so pad the short ones out to the
        // widest. Without this the right border would zig-zag.
        row.extend(std::iter::repeat_n(Cell::new(' '), width - body.len()));
        row.extend(std::iter::repeat_n(Cell::new(' '), pad));
        row.push(Cell::new(glyphs.right_edge));
        row
    };

    if opts.kind.has_rules() {
        cells.push(rule_row(glyphs.top_left, glyphs.horizontal, glyphs.top_right));
    }
    for _ in 0..pad {
        cells.push(side_row(&[], &glyphs));
    }
    for row in &art.cells {
        cells.push(side_row(row, &glyphs));
    }
    for _ in 0..pad {
        cells.push(side_row(&[], &glyphs));
    }
    if opts.kind.has_rules() {
        cells.push(rule_row(glyphs.bottom_left, glyphs.horizontal, glyphs.bottom_right));
    }

    Artwork { cells }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn art() -> Artwork {
        Artwork::from_lines(["ab", "c"])
    }

    #[test]
    fn none_returns_the_artwork_untouched() {
        let a = art();
        let out = apply(&a, &Options { kind: FrameKind::None, padding: 2 });
        assert_eq!(out, a);
    }

    #[test]
    fn a_frame_adds_one_column_each_side() {
        let a = art();
        let out = apply(&a, &Options { kind: FrameKind::Light, padding: 0 });
        assert_eq!(out.width(), a.width() + 2);
        assert_eq!(out.height(), a.height() + 2);
    }

    #[test]
    fn padding_grows_both_dimensions() {
        let a = art();
        let out = apply(&a, &Options { kind: FrameKind::Light, padding: 2 });
        assert_eq!(out.width(), a.width() + 2 + 4);
        assert_eq!(out.height(), a.height() + 2 + 4);
    }

    #[test]
    fn padding_is_capped() {
        let a = art();
        let out = apply(&a, &Options { kind: FrameKind::Light, padding: 999 });
        assert_eq!(out.height(), a.height() + 2 + MAX_PADDING * 2);
    }

    #[test]
    fn every_row_is_the_same_width() {
        // The art is ragged before framing; the border must still be a rectangle.
        let out = apply(&art(), &Options { kind: FrameKind::Light, padding: 1 });
        let w = out.width();
        for row in &out.cells {
            assert_eq!(row.len(), w, "ragged row in a framed artwork");
        }
    }

    #[test]
    fn corners_and_edges_are_the_expected_glyphs() {
        let out = apply(&art(), &Options { kind: FrameKind::Double, padding: 0 });
        assert_eq!(out.cells[0][0].ch, '╔');
        assert_eq!(out.cells[0][out.width() - 1].ch, '╗');
        let last = out.height() - 1;
        assert_eq!(out.cells[last][0].ch, '╚');
        assert_eq!(out.cells[last][out.width() - 1].ch, '╝');
        assert_eq!(out.cells[1][0].ch, '║');
    }

    #[test]
    fn powerline_has_caps_but_no_rules() {
        let a = art();
        let out = apply(&a, &Options { kind: FrameKind::Powerline, padding: 0 });
        // No extra rows, since there is no top or bottom rule.
        assert_eq!(out.height(), a.height());
        // The two caps mirror, so the left and right edges must differ.
        assert_eq!(out.cells[0][0].ch, CAP_LEFT);
        assert_eq!(out.cells[0][out.width() - 1].ch, CAP_RIGHT);
    }

    #[test]
    fn powerline_padding_still_adds_rows() {
        let a = art();
        let out = apply(&a, &Options { kind: FrameKind::Powerline, padding: 1 });
        assert_eq!(out.height(), a.height() + 2);
        // Padding rows are capped too, not left as bare spaces.
        assert_eq!(out.cells[0][0].ch, CAP_LEFT);
        assert_eq!(out.cells[out.height() - 1][out.width() - 1].ch, CAP_RIGHT);
    }

    #[test]
    fn the_original_art_survives_inside_the_frame() {
        let a = art();
        let out = apply(&a, &Options { kind: FrameKind::Light, padding: 1 });
        for (y, row) in a.cells.iter().enumerate() {
            for (x, cell) in row.iter().enumerate() {
                // +1 border, +1 padding
                assert_eq!(out.cells[y + 2][x + 2].ch, cell.ch);
            }
        }
    }

    #[test]
    fn empty_artwork_is_returned_as_is() {
        let a = Artwork::new();
        let out = apply(&a, &Options { kind: FrameKind::Light, padding: 1 });
        assert!(out.is_empty());
    }

    #[test]
    fn every_style_produces_a_rectangle() {
        // `None` deliberately returns the artwork as-is, so it inherits the
        // art's ragged right edge and is excluded here.
        for kind in FrameKind::ALL.into_iter().filter(|k| *k != FrameKind::None) {
            let out = apply(&art(), &Options { kind, padding: 1 });
            let w = out.width();
            assert!(w > 0, "{kind:?} produced nothing");
            for row in &out.cells {
                assert_eq!(row.len(), w, "{kind:?} produced a ragged row");
            }
        }
    }

    #[test]
    fn frame_glyphs_are_not_treated_as_blank() {
        // Trimming runs before framing, but a frame cell must never look blank
        // to the colorizer or the border would come out uncolored.
        let out = apply(&art(), &Options { kind: FrameKind::Rounded, padding: 0 });
        assert!(!out.cells[0][0].is_blank());
        assert!(!out.cells[1][0].is_blank());
    }
}
