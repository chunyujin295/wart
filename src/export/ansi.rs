//! ANSI export — a plain text file with embedded SGR escape sequences, ready for
//! `fastfetch --logo` or a terminal `cat`.
//!
//! Only emits an escape sequence when the color actually changes, so a
//! single-color artwork carries one escape per row rather than one per cell.

use crate::model::{Artwork, Rgb};

#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub color: bool,
    /// Use the xterm-256 palette instead of 24-bit truecolor.
    pub xterm256: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { color: true, xterm256: false }
    }
}

fn fg(c: Rgb, xterm256: bool) -> String {
    if xterm256 {
        format!("\x1b[38;5;{}m", c.to_xterm256())
    } else {
        format!("\x1b[38;2;{};{};{}m", c.r, c.g, c.b)
    }
}

fn bg(c: Rgb, xterm256: bool) -> String {
    if xterm256 {
        format!("\x1b[48;5;{}m", c.to_xterm256())
    } else {
        format!("\x1b[48;2;{};{};{}m", c.r, c.g, c.b)
    }
}

const RESET: &str = "\x1b[0m";

pub fn export(art: &Artwork, opts: &Options) -> String {
    let mut out = String::new();

    for (row_idx, row) in art.cells.iter().enumerate() {
        if row_idx > 0 {
            out.push('\n');
        }
        if !opts.color {
            out.extend(row.iter().map(|c| c.ch));
            continue;
        }

        // `None` means "whatever color is currently active", so the first
        // colored cell always differs and forces an escape.
        let mut cur_fg: Option<Rgb> = None;
        let mut cur_bg: Option<Rgb> = None;

        for cell in row {
            if cell.fg != cur_fg {
                match cell.fg {
                    Some(c) => out.push_str(&fg(c, opts.xterm256)),
                    // Falling back to the default foreground is better than
                    // leaving the previous color in place.
                    None => out.push_str("\x1b[39m"),
                }
                cur_fg = cell.fg;
            }
            if cell.bg != cur_bg {
                match cell.bg {
                    Some(c) => out.push_str(&bg(c, opts.xterm256)),
                    None => out.push_str("\x1b[49m"),
                }
                cur_bg = cell.bg;
            }
            out.push(cell.ch);
        }

        if cur_fg.is_some() || cur_bg.is_some() {
            out.push_str(RESET);
        }
    }

    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Cell;

    fn colored_art() -> Artwork {
        let mut a = Artwork::from_lines(["ab", "cd"]);
        a.cells[0][0].fg = Some(Rgb::new(255, 0, 0));
        a.cells[0][1].fg = Some(Rgb::new(255, 0, 0));
        a.cells[1][0].fg = Some(Rgb::new(0, 0, 255));
        a.cells[1][1].fg = Some(Rgb::new(0, 0, 255));
        a
    }

    #[test]
    fn no_color_emits_plain_text() {
        let art = colored_art();
        let out = export(&art, &Options { color: false, xterm256: false });
        assert_eq!(out, "ab\ncd\n");
        assert!(!out.contains('\x1b'));
    }

    #[test]
    fn truecolor_emits_one_escape_per_run() {
        let art = colored_art();
        let out = export(&art, &Options::default());
        // Two cells share each color, so each row costs exactly one escape plus a reset.
        assert_eq!(out.matches("\x1b[38;2;255;0;0m").count(), 1);
        assert_eq!(out.matches("\x1b[38;2;0;0;255m").count(), 1);
        assert_eq!(out.matches(RESET).count(), 2);
        assert!(out.contains("ab") && out.contains("cd"));
    }

    #[test]
    fn xterm256_uses_the_palette_form() {
        let art = colored_art();
        let out = export(&art, &Options { color: true, xterm256: true });
        assert!(out.contains("\x1b[38;5;196m"), "pure red is palette 196");
        assert!(!out.contains("38;2;"));
    }

    #[test]
    fn half_block_cells_emit_background_colors() {
        let mut art = Artwork::new();
        art.cells.push(vec![Cell {
            ch: '▀',
            fg: Some(Rgb::new(255, 0, 0)),
            bg: Some(Rgb::new(0, 0, 255)),
        }]);
        let out = export(&art, &Options::default());
        assert!(out.contains("\x1b[38;2;255;0;0m"));
        assert!(out.contains("\x1b[48;2;0;0;255m"));
    }

    #[test]
    fn every_row_is_reset_before_the_next() {
        // Without the reset, row 2 would inherit row 1's color when the first
        // cell of row 2 is uncolored.
        let art = colored_art();
        let out = export(&art, &Options::default());
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[0].ends_with(RESET));
    }
}
