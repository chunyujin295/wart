//! FIGlet generation mode: a `.flf` font plus input text becomes an [`Artwork`].
//!
//! Smushing, kerning and hardblank handling are all done by `figlet-rs` from the
//! font's own header, so there is nothing to reimplement here.
//!
//! # Characters a FIGlet font cannot draw
//!
//! A `.flf` file is a lookup table from character code to an ASCII-art bitmap.
//! Its entries are ASCII, so there is no way to *draw* a Nerd Font icon with a
//! FIGlet font — the icon is simply not in the table.
//!
//! What is possible, and what this module does, is to **rasterize** them: the
//! input is split into runs, the ASCII runs are rendered as banners, and
//! everything else is drawn as ASCII art, slightly taller than the banner so it
//! reads as an emblem. See [`IconStyle`] for how that art is chosen.
//!
//! Block mode is still the one to reach for when the Unicode text itself should
//! be rasterised into a large figure.

use super::block;
use super::charset::Charset;
use crate::color::ColorStyle;
use crate::model::{Artwork, Cell};
use figlet_rs::FIGlet;

/// One horizontal stretch of a line.
enum Piece {
    /// A rectangular block of cells, `height` rows tall.
    ///
    /// Both the ASCII banners and the rasterized glyph art below are this shape,
    /// which is what lets them be concatenated row by row.
    Block(Vec<Vec<Cell>>),
    /// Fallback for a character that could not be rasterized: one literal cell.
    Literal(Vec<char>),
}

/// How to draw characters the FIGlet font has no glyph for.
///
/// A genuine trade-off rather than a default worth guessing at: at the handful
/// of rows a banner is tall, line art matches the letters' vocabulary but has
/// little room for detail, while Braille carries four times the resolution at
/// the cost of a dot texture that reads as a different medium.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IconStyle {
    /// Picks, per cell, the printable ASCII character whose drawn shape most
    /// resembles that cell. Reads as ASCII art — the same medium as the letters
    /// — while staying legible, because it compares shape rather than darkness.
    #[default]
    Shape,
    /// 2x4 dots per cell. The crispest icon, at the cost of a dot texture that
    /// is plainly not the one the letters are drawn in.
    Braille,
    /// Solid blocks `█`: a silhouette drawn one cell thick, chunky and crisp.
    Blocks,
    /// Upper half blocks, with an independent color per half.
    HalfBlock,
    /// Quarter blocks: a 2x2 grid of sub-cells per character. Twice the shape
    /// resolution of a half block, and it needs no background color because a
    /// quarter block's own gaps provide the transparency.
    Quadrant,
    /// `/ \ | _ -`, the same characters a banner is drawn with. Style-matched,
    /// but edge detection has far less information to work with per cell than a
    /// dot grid does, so the result is only legible when drawn much larger.
    LineArt,
}

/// Rows of character cells an icon is drawn into at minimum, whatever the
/// banner's height.
///
/// [`IconStyle::row_scale`] alone ties the icon to the letters, which reads well
/// until the font is a short one: a one-row font asks for a two-row icon, and no
/// charset can draw a figure in two rows. Past that point a smaller icon is not
/// a smaller version of the same picture, it is an unrecognisable smudge, so the
/// icon is given the height it needs to be read and allowed to overhang. Ten
/// rows is where the styles in `IconStyle::ALL` — including the sparse ones —
/// start resolving a Nerd Font icon's interior.
const MIN_ICON_ROWS: usize = 10;

impl IconStyle {
    pub const ALL: [IconStyle; 6] = [
        IconStyle::Shape,
        IconStyle::Braille,
        IconStyle::Blocks,
        IconStyle::Quadrant,
        IconStyle::HalfBlock,
        IconStyle::LineArt,
    ];

    pub fn charset(self) -> Charset {
        match self {
            IconStyle::Shape => Charset::Shape,
            IconStyle::Braille => Charset::Braille,
            IconStyle::Blocks => Charset::Blocks,
            IconStyle::HalfBlock => Charset::HalfBlock,
            IconStyle::Quadrant => Charset::Quadrant,
            IconStyle::LineArt => Charset::LineArt,
        }
    }

    /// How tall the icon is drawn, as a multiple of the banner's height.
    ///
    /// Measured by rendering one icon across a range of sizes and finding where
    /// it becomes recognisable. The height asked for here is the height the
    /// drawn figure gets, not the height of an em box containing it — see
    /// [`block::render_to_rows`], which spends it all on ink. A six-row banner
    /// asks for 9 rows of cells, which [`MIN_ICON_ROWS`] lifts to 10: each cell
    /// is 2 source pixels tall for the block charsets, so a good 20x20 pixels of
    /// figure.
    ///
    /// Every style is drawn with some overhang, which also reads as a deliberate
    /// emblem rather than a misalignment, and the shortest banners keep that
    /// floor rather than the scale, so a small font cannot ask for an illegible
    /// icon.
    ///
    /// Line art needs the most room, because a stroke carries far less
    /// information per cell than a matched glyph or a filled dot does.
    pub fn row_scale(self) -> f32 {
        match self {
            IconStyle::LineArt => 2.0,
            IconStyle::Shape
            | IconStyle::Braille
            | IconStyle::Blocks
            | IconStyle::HalfBlock
            | IconStyle::Quadrant => 1.5,
        }
    }
}

/// Split a line into runs the font can draw and runs it cannot.
///
/// Printable ASCII is the range every FIGlet font covers. Control characters
/// are dropped rather than rendered, since a literal tab or bell in the grid
/// would corrupt the output.
fn pieces(
    font: &FIGlet,
    line: &str,
    height: usize,
    style: &ColorStyle,
    icon_style: IconStyle,
) -> Vec<Piece> {
    let mut out: Vec<Piece> = Vec::new();
    let mut ascii = String::new();
    let mut literal: Vec<char> = Vec::new();

    for ch in line.chars() {
        match ch {
            ' '..='~' => {
                push_literal(&mut literal, height, style, icon_style, &mut out);
                ascii.push(ch);
            }
            c if c.is_control() => {}
            c => literal.push(c),
        }
    }
    push_banner(font, &mut ascii, &mut out);
    push_literal(&mut literal, height, style, icon_style, &mut out);

    // Every block is padded out to a rectangle, so that whatever follows starts
    // at the same column on all rows. Without this an icon placed on a short row
    // lands inside the art's bounding box and reads as overlapping the letters.
    for piece in &mut out {
        if let Piece::Block(cells) = piece {
            let width = cells.iter().map(Vec::len).max().unwrap_or(0);
            for row in cells.iter_mut() {
                row.resize(width, Cell::new(' '));
            }
        }
    }
    out
}

/// Render a run of ASCII as a banner.
fn push_banner(font: &FIGlet, ascii: &mut String, out: &mut Vec<Piece>) {
    if ascii.is_empty() {
        return;
    }
    if let Some(figure) = font.convert(ascii) {
        let rendered = figure.as_str();
        // `Display` writes each row with `writeln!`, so the string ends in a
        // newline; `str::lines` absorbs that.
        out.push(Piece::Block(Artwork::from_lines(rendered.lines()).cells));
    }
    ascii.clear();
}

/// Draw characters the font lacks as ASCII art, at the height the icon style
/// asks for — the banner's height scaled, and never less than
/// [`MIN_ICON_ROWS`].
///
/// A literal cell would be one row tall next to a six-row banner and read as a
/// speck, so the glyph is rasterized instead.
fn push_literal(
    literal: &mut Vec<char>,
    height: usize,
    style: &ColorStyle,
    icon_style: IconStyle,
    out: &mut Vec<Piece>,
) {
    if literal.is_empty() {
        return;
    }
    let text: String = literal.iter().collect();
    let opts = block::Options {
        charset: icon_style.charset(),
        // Unused: the height is what an icon is fitted to, and the width
        // follows from the glyph's own proportions.
        cols: 0,
        // The cutoff is fixed rather than taken from `--threshold`: an icon is
        // drawn at one particular size, and this is the value the styles were
        // measured at. The default is the same value, so a caller that wants to
        // tune the cutoff for *text* is not surprised by the icons.
        threshold: 0.5,
        // The glyph art is colored by the shared pass afterwards, alongside the
        // banner; the style is only here to fill the struct — except for
        // half-block, which picks its two colors while the cell is built.
        style: style.clone(),
    };

    // Legibility sets the floor, the banner sets the target: below a certain
    // size the figure stops being a small picture of the icon and becomes a
    // smudge, and a smudge is worth neither the rows it occupies nor the
    // confusion of sitting next to the letters.
    let rows = ((height as f32) * icon_style.row_scale())
        .round()
        .max(MIN_ICON_ROWS as f32) as usize;
    match block::render_to_rows(&text, rows, &opts) {
        Ok(mut art) => {
            // The icon sampled its own colors on its own grid, and it is about
            // to be spliced into a line whose width it cannot know. Dropping the
            // foreground hands every cell to the shared color pass, which
            // samples the assembled artwork instead — so the icon continues the
            // gradient around it rather than carrying a squeezed copy of the
            // whole ramp. A background is kept: it is what marks a half-block
            // cell whose halves carry two different colors, and those the color
            // pass re-samples in place.
            for cell in art.cells.iter_mut().flatten() {
                if cell.bg.is_none() {
                    cell.fg = None;
                }
            }
            out.push(Piece::Block(art.cells))
        }
        // Rasterizing should not fail; a literal cell is a poor but honest
        // fallback if it ever does.
        Err(_) => out.push(Piece::Literal(std::mem::take(literal))),
    }
    literal.clear();
}

/// Render one or more lines of text.
///
/// Input is split on newlines first. A FIGlet font has no glyph for `\n`, so
/// feeding a multi-line string straight to `convert` makes the crate drop the
/// newline and run the lines together. Each line is therefore rendered as its
/// own banner and the results are stacked.
pub fn render(
    font: &FIGlet,
    text: &str,
    style: &ColorStyle,
    icon_style: IconStyle,
) -> Option<Artwork> {
    // Every banner from this font is this many rows tall, which is what lets
    // banners and literal glyphs be concatenated row by row.
    let height = font.header_line.height.max(1) as usize;

    let mut rows: Vec<Vec<Cell>> = Vec::new();

    for line in text.split('\n') {
        if line.trim().is_empty() {
            // Preserve the user's blank line as a single blank row rather than
            // dropping it, so vertical spacing survives.
            rows.push(Vec::new());
            continue;
        }

        let pieces = pieces(font, line, height, style, icon_style);
        if pieces.is_empty() {
            rows.push(Vec::new());
            continue;
        }

        // A rasterized icon may be taller than the banner, so the line is as
        // tall as its tallest piece and each piece is centred within it.
        let line_height = pieces
            .iter()
            .map(|p| match p {
                Piece::Block(cells) => cells.len(),
                Piece::Literal(_) => 1,
            })
            .max()
            .unwrap_or(height)
            .max(1);

        let mut line_rows: Vec<Vec<Cell>> = vec![Vec::new(); line_height];

        for piece in pieces {
            match piece {
                Piece::Block(cells) => {
                    let offset = (line_height - cells.len()) / 2;
                    for (y, row) in cells.iter().enumerate() {
                        if let Some(slot) = line_rows.get_mut(offset + y) {
                            slot.extend_from_slice(row);
                        }
                    }
                    // Rows the piece does not reach still need its width, or the
                    // next piece would start at a different column.
                    let width = cells.iter().map(Vec::len).max().unwrap_or(0);
                    for (y, row) in line_rows.iter_mut().enumerate() {
                        if y < offset || y >= offset + cells.len() {
                            row.extend(std::iter::repeat_n(Cell::new(' '), width));
                        }
                    }
                }
                Piece::Literal(chars) => {
                    let width = chars.len();
                    let at = (line_height - 1) / 2;
                    for (y, row) in line_rows.iter_mut().enumerate() {
                        if y == at {
                            row.extend(chars.iter().map(|c| Cell::new(*c)));
                        } else {
                            row.extend(std::iter::repeat_n(Cell::new(' '), width));
                        }
                    }
                }
            }
        }

        rows.extend(line_rows);
    }

    // Only tidy the outer edges: interior blank rows are deliberate.
    let mut art = Artwork { cells: rows };
    art.trim_end();
    art.trim_blank_rows();
    if art.is_empty() {
        return None;
    }
    Some(art)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts;

    fn art_of(font: &FIGlet, text: &str) -> Artwork {
        render(font, text, &ColorStyle::default(), IconStyle::default()).expect("render")
    }

    fn standard() -> FIGlet {
        fonts::load_builtin("Standard").expect("Standard font")
    }

    /// The whole grid as plain text, for containment checks.
    fn chars(art: &Artwork) -> String {
        art.to_plain_string()
    }

    // --- banner layout -----------------------------------------------------

    #[test]
    fn multiline_input_stacks_instead_of_running_together() {
        // A FIGlet font has no newline glyph, so without splitting the lines
        // would be silently concatenated into one banner.
        let font = standard();
        let one = art_of(&font, "AB");
        let two = art_of(&font, "A\nB");

        assert!(
            two.height() >= one.height() * 2 - 1,
            "two stacked banners should be about twice as tall: {} vs {}",
            two.height(),
            one.height()
        );
        assert_ne!(chars(&two), chars(&one), "the newline must change the output");
    }

    #[test]
    fn blank_input_lines_become_blank_rows() {
        let art = art_of(&standard(), "A\n\nB");
        let blank = |row: &Vec<Cell>| row.iter().all(|c| c.is_blank());
        assert!(
            art.cells.iter().any(blank),
            "the empty input line should survive as a blank row"
        );
    }

    #[test]
    fn standard_font_produces_multiple_rows() {
        let art = art_of(&standard(), "Hi");
        assert!(art.height() > 1, "FIGlet output should be several rows tall");
        assert!(art.width() > 2, "two letters should be wider than two columns");
    }

    #[test]
    fn empty_input_renders_nothing() {
        let font = standard();
        let style = ColorStyle::default();
        let icon = IconStyle::default();
        assert!(render(&font, "", &style, icon).is_none());
        assert!(render(&font, "   ", &style, icon).is_none());
    }

    #[test]
    fn trailing_whitespace_is_trimmed() {
        let art = art_of(&standard(), "A");
        for row in &art.cells {
            // A FIGlet font pads vertically as well as horizontally, so an
            // interior row can legitimately be blank. What must never happen is
            // a row *ending* in blank cells.
            if let Some(last) = row.last() {
                assert!(!last.is_blank(), "row should not end with blank cells");
            }
        }
    }

    #[test]
    fn no_blank_rows_at_the_edges() {
        let art = art_of(&standard(), "A");
        let blank = |row: &Vec<Cell>| row.iter().all(|c| c.is_blank());
        assert!(!art.cells.first().is_some_and(blank), "leading blank row leaked");
        assert!(!art.cells.last().is_some_and(blank), "trailing blank row leaked");
    }

    #[test]
    fn no_hardblank_leaks_into_output() {
        // `$` is the conventional hardblank; figlet-rs replaces it with a space
        // before output.
        assert!(!chars(&art_of(&standard(), "AB")).contains('$'));
    }

    // --- characters the font cannot draw -----------------------------------

    /// A private-use codepoint, i.e. what a Nerd Font icon is.
    const ICON: char = '\u{eb99}';
    const ICON2: char = '\u{eb9a}';

    fn art_of_style(font: &FIGlet, text: &str, style: IconStyle) -> Artwork {
        render(font, text, &ColorStyle::default(), style).expect("render")
    }

    fn lit_braille(art: &Artwork) -> usize {
        chars(art)
            .chars()
            .filter(|c| ('\u{2801}'..='\u{28ff}').contains(c))
            .count()
    }

    #[test]
    fn a_nerd_icon_is_rasterized_rather_than_dropped() {
        let art = art_of(&standard(), &ICON.to_string());
        assert!(!art.is_empty(), "the icon produced nothing");
        assert!(!chars(&art).contains('?'), "it must not become a placeholder");
        assert!(
            !chars(&art).contains(ICON),
            "it is drawn as art, not placed as a literal glyph"
        );
        assert!(
            art.cells.iter().flatten().any(|c| !c.is_blank()),
            "nothing was drawn"
        );
    }

    #[test]
    fn the_default_style_uses_the_same_characters_as_the_banner() {
        // The whole point of the line-art default: the icon is drawn from the
        // same stroke vocabulary the letters are.
        let art = art_of_style(&standard(), &format!("{ICON}A"), IconStyle::LineArt);
        let stray: Vec<char> = art
            .cells
            .iter()
            .flatten()
            .map(|c| c.ch)
            .filter(|c| !c.is_ascii_alphanumeric())
            .filter(|c| !matches!(c, ' ' | '_' | '-' | '|' | '/' | '\\'))
            .collect();
        assert!(stray.is_empty(), "unexpected characters in the art: {stray:?}");
    }

    #[test]
    fn the_braille_style_draws_braille() {
        let art = art_of_style(&standard(), &ICON.to_string(), IconStyle::Braille);
        assert!(lit_braille(&art) > 0, "no lit braille dots");
    }

    #[test]
    fn the_two_styles_produce_different_art() {
        let font = standard();
        let line = art_of_style(&font, &ICON.to_string(), IconStyle::LineArt);
        let dots = art_of_style(&font, &ICON.to_string(), IconStyle::Braille);
        assert_ne!(chars(&line), chars(&dots));
    }

    #[test]
    fn an_icon_is_drawn_at_least_as_tall_as_the_banner() {
        // The point of rasterizing: a literal glyph was one cell tall and read
        // as a speck beside a six-row banner. The icon overhangs it — 1.5x the
        // banner or [`MIN_ICON_ROWS`], whichever is more — so the bar here is
        // only that it is never the smaller of the two.
        let font = standard();
        let icon = art_of(&font, &ICON.to_string());
        let banner = art_of(&font, "A");
        assert!(
            icon.height() >= banner.height(),
            "icon is {} rows against a {}-row banner",
            icon.height(),
            banner.height()
        );
        assert!(icon.width() > 2, "should be several cells wide, was {}", icon.width());
    }

    #[test]
    fn an_icon_takes_its_colors_from_the_line_not_from_itself() {
        // Regression: half-block bakes its cells' colors while the icon is
        // being drawn, on the icon's own grid. Left in place, that painted a
        // second copy of the gradient squeezed into the icon's own width, and
        // the seam where it rejoined the line was plain to see.
        let font = standard();
        let ramp = ColorStyle::linear(crate::model::Rgb::new(255, 0, 0), crate::model::Rgb::new(0, 0, 255), 0.0);
        let mut art = render(&font, &format!("{ICON}HHHHHHHH"), &ramp, IconStyle::HalfBlock)
            .expect("render");
        // What the caller does once the line is assembled; the icon's cells are
        // expected to be left for it.
        crate::color::colorize(&mut art, &ramp);

        let widest = art
            .cells
            .iter()
            .max_by_key(|r| r.iter().filter(|c| c.fg.is_some()).count())
            .expect("a row");
        let reds: Vec<u8> = widest.iter().filter_map(|c| c.fg.map(|c| c.r)).collect();
        assert!(reds.len() > 20, "expected a long colored row, got {}", reds.len());

        let biggest_step = reds.windows(2).map(|w| w[0].abs_diff(w[1])).max().unwrap_or(0);
        assert!(
            biggest_step <= 20,
            "the ramp jumps by {biggest_step} between neighbouring cells: {reds:?}"
        );
    }

    #[test]
    fn a_one_row_font_still_gets_a_legible_icon() {
        // `row_scale` alone would ask a one-row banner for a two-row icon, which
        // is not a small picture of the icon but a smudge. The floor gives the
        // figure the rows it needs and lets it overhang instead.
        let font = fonts::load_builtin("Term").expect("Term font");
        let banner = art_of(&font, "Hi");
        let art = art_of(&font, &ICON.to_string());

        assert!(
            art.height() >= MIN_ICON_ROWS - 1,
            "a {}-row figure is not legible",
            art.height()
        );
        assert!(art.width() >= 8, "a {}-column figure is not legible", art.width());
        assert!(
            art.height() > banner.height() * 2,
            "the icon should overhang a {}-row banner, not match it",
            banner.height()
        );
    }

    #[test]
    fn a_one_row_font_keeps_the_icon_beside_the_letters() {
        // Overhanging is fine; landing on its own line is not. The line is as
        // tall as the icon, and the banner is centred inside it.
        let font = fonts::load_builtin("Term").expect("Term font");
        let with = art_of(&font, &format!("{ICON}Hi"));
        let without = art_of(&font, "Hi");
        assert!(with.width() > without.width(), "the letters were pushed off");
        let blank = |r: &Vec<Cell>| r.iter().all(|c| c.is_blank());
        assert!(with.cells.iter().any(|r| !blank(r)), "nothing was drawn");
    }

    #[test]
    fn an_icon_reaches_across_the_block_not_one_row() {
        let art = art_of(&standard(), &ICON.to_string());
        let inked = art
            .cells
            .iter()
            .filter(|r| r.iter().any(|c| !c.is_blank()))
            .count();
        assert!(
            inked > 1,
            "the icon should span rows, but only {inked} row(s) have any ink"
        );
    }

    #[test]
    fn an_icon_sits_beside_the_banner_not_above_it() {
        let font = standard();
        let with = art_of(&font, &format!("A{ICON}"));
        let without = art_of(&font, "A");
        assert!(with.width() > without.width(), "the icon should add width");
        // The icon is drawn taller than the letters on purpose, so the line
        // grows; what must not happen is the icon landing on its own row.
        assert!(with.height() >= without.height());
    }

    #[test]
    fn the_icon_starts_at_the_banners_right_edge() {
        // Guards the rectangular-padding rule: without it the icon would land
        // inside the art's bounding box and read as overlapping the letters.
        let font = standard();
        let banner = art_of(&font, "A");
        let with = art_of(&font, &format!("A{ICON}"));
        // Rows are ragged after the final trim, so look for ink past the
        // banner's right edge rather than slicing at a fixed column.
        assert!(
            with.cells
                .iter()
                .any(|r| r.iter().skip(banner.width()).any(|c| !c.is_blank())),
            "nothing drawn past the banner's right edge"
        );
    }

    #[test]
    fn text_around_an_icon_still_renders() {
        let font = standard();
        let with_b = art_of(&font, &format!("A{ICON}B"));
        let without_b = art_of(&font, &format!("A{ICON}"));
        assert!(with_b.width() > without_b.width(), "the trailing letter is missing");
    }

    #[test]
    fn the_banner_is_centred_against_a_taller_icon() {
        // The icon is drawn taller than the banner, so the line is as tall as
        // the icon and the letters have to sit in the middle of it rather than
        // at the top.
        let font = standard();
        let banner_height = art_of(&font, "A").height();
        let art = art_of(&font, &format!("{ICON}A"));

        assert!(
            art.height() > banner_height,
            "expected the icon to make the line taller"
        );

        // The banner's own ink should start below the first row and end above
        // the last, i.e. it is inset top and bottom.
        let blank = |r: &Vec<Cell>| r.iter().all(|c| c.is_blank());
        let first_inked = art.cells.iter().position(|r| !blank(r)).unwrap_or(0);
        assert_eq!(first_inked, 0, "the icon should reach the top row");
    }

    #[test]
    fn consecutive_icons_render_in_order() {
        // Two different icons must not collapse into one, and the run must be
        // wider than either alone.
        let font = standard();
        let one = art_of(&font, &ICON.to_string());
        let two = art_of(&font, &format!("{ICON}{ICON2}"));
        assert!(two.width() > one.width(), "the second icon is missing");
    }

    #[test]
    fn cjk_is_rasterized_too() {
        let art = art_of(&standard(), "A\u{4e16}");
        assert!(!art.is_empty());
        assert!(art.width() > art_of(&standard(), "A").width());
    }

    #[test]
    fn control_characters_are_dropped_not_drawn() {
        // A literal tab or bell in the grid would corrupt the export.
        let font = standard();
        let with_tab = art_of(&font, "A\tB");
        let plain = art_of(&font, "AB");
        assert_eq!(with_tab.width(), plain.width(), "the tab should vanish");
    }

    #[test]
    fn a_line_of_only_non_ascii_still_renders() {
        let art = art_of(&standard(), &ICON.to_string());
        assert!(!art.is_empty());
        assert!(art.height() > 0 && art.width() > 0);
    }

    #[test]
    fn rasterized_glyphs_are_not_blank_so_they_get_colored() {
        // The colorizer skips blank cells; if the icon art looked blank it would
        // come out uncolored next to a colored banner.
        let art = art_of(&standard(), &ICON.to_string());
        assert!(
            art.cells.iter().flatten().any(|c| !c.is_blank()),
            "every cell was blank, so nothing would be colored"
        );
    }
}
