//! Core data model.
//!
//! Both generation modes (FIGlet and bitmap-block) produce an [`Artwork`], and
//! every exporter consumes one. Keeping this the only seam between the two
//! halves means a new charset never touches an exporter, and a new export
//! format never touches a renderer.

/// A 24-bit color. Not `egui::Color32` because the core must not depend on the
/// GUI toolkit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const WHITE: Self = Self::new(255, 255, 255);
    /// Not referenced by the binary itself, but the natural counterpart to
    /// `WHITE` and what the color tests build ramps from.
    #[allow(dead_code)]
    pub const BLACK: Self = Self::new(0, 0, 0);

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Linear interpolation towards `other`. `t` is clamped to `[0, 1]`.
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Self::new(mix(self.r, other.r), mix(self.g, other.g), mix(self.b, other.b))
    }

    /// Parse `#rrggbb` or `rrggbb`.
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let h = s.strip_prefix('#').unwrap_or(s);
        anyhow::ensure!(h.len() == 6, "expected 6 hex digits, got {h:?}");
        let v = u32::from_str_radix(h, 16)
            .map_err(|e| anyhow::anyhow!("bad hex color {s:?}: {e}"))?;
        Ok(Self::new(
            (v >> 16) as u8,
            (v >> 8) as u8,
            v as u8,
        ))
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Nearest color in the xterm-256 palette, for terminals/plugins that
    /// cannot do truecolor.
    pub fn to_xterm256(self) -> u8 {
        let (r, g, b) = (self.r as i32, self.g as i32, self.b as i32);

        // The 6x6x6 color cube (16..231) and the 24-step grayscale ramp (232..255)
        // are two separate candidates; pick whichever is closer.
        let to6 = |c: i32| -> i32 {
            // 0..255 -> 0..5, rounding to the nearest cube level.
            ((c * 5 + 127) / 255).clamp(0, 5)
        };
        let (cr, cg, cb) = (to6(r), to6(g), to6(b));
        let cube_idx = 16 + 36 * cr + 6 * cg + cb;
        let cube = Rgb::new((cr * 51) as u8, (cg * 51) as u8, (cb * 51) as u8);

        let gray_avg = (r + g + b) / 3;
        let gray_level = ((gray_avg - 3).max(0) / 10).clamp(0, 23);
        let gray_idx = 232 + gray_level;
        let gray_v = (8 + gray_level * 10) as u8;
        let gray = Rgb::new(gray_v, gray_v, gray_v);

        let d = |a: Rgb, b: Rgb| -> i32 {
            let dr = a.r as i32 - b.r as i32;
            let dg = a.g as i32 - b.g as i32;
            let db = a.b as i32 - b.b as i32;
            dr * dr + dg * dg + db * db
        };

        if d(self, cube) <= d(self, gray) {
            cube_idx as u8
        } else {
            gray_idx as u8
        }
    }
}

/// One character cell of the output grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Option<Rgb>,
    /// Only half-block mode uses this: the cell draws an upper half block, so
    /// the lower half of the box takes the background color.
    pub bg: Option<Rgb>,
}

impl Cell {
    pub const fn new(ch: char) -> Self {
        Self { ch, fg: None, bg: None }
    }

    /// Whether this cell paints nothing.
    ///
    /// Braille has its own blank at U+2800 (⠀), which is *not* the ASCII space,
    /// so testing only for `' '` silently defeats every trim in braille and
    /// half-block output.
    pub fn is_blank(&self) -> bool {
        matches!(self.ch, ' ' | '\u{2800}') && self.fg.is_none() && self.bg.is_none()
    }
}

/// A rectangular grid of cells. Every cell of a FIGlet font row is one column,
/// even though the glyphs are not monospace — that is what lets the gradient
/// address cells by coordinate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Artwork {
    pub cells: Vec<Vec<Cell>>,
}

impl Artwork {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build from raw lines, one cell per `char`.
    pub fn from_lines<I, S>(lines: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let cells = lines
            .into_iter()
            .map(|l| l.as_ref().chars().map(Cell::new).collect())
            .collect();
        Self { cells }
    }

    pub fn height(&self) -> usize {
        self.cells.len()
    }

    /// Width of the widest row. Rows are not padded to a common width because
    /// FIGlet output legitimately has ragged right edges.
    pub fn width(&self) -> usize {
        self.cells.iter().map(|r| r.len()).max().unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Strip trailing blank cells from every row. FIGlet fonts are padded to a
    /// fixed width, so without this the exported art carries a large invisible
    /// right margin.
    pub fn trim_end(&mut self) {
        for row in &mut self.cells {
            while row.last().is_some_and(|c| c.is_blank()) {
                row.pop();
            }
        }
    }

    /// Drop leading and trailing all-blank rows.
    pub fn trim_blank_rows(&mut self) {
        let blank = |row: &Vec<Cell>| row.iter().all(|c| c.is_blank());
        while self.cells.first().is_some_and(blank) {
            self.cells.remove(0);
        }
        while self.cells.last().is_some_and(blank) {
            self.cells.pop();
        }
    }

    /// The whole artwork with no color and no escape sequences.
    ///
    /// The plain-text exporter is built on this; the others go through
    /// `row_segments`.
    pub fn to_plain_string(&self) -> String {
        let mut out = String::new();
        for (i, row) in self.cells.iter().enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.extend(row.iter().map(|c| c.ch));
        }
        out
    }

    /// A whole row as plain text, without a trailing newline.
    pub fn row_text(&self, y: usize) -> String {
        self.cells
            .get(y)
            .map(|row| row.iter().map(|c| c.ch).collect())
            .unwrap_or_default()
    }

    /// Group a row into runs of consecutive cells that share a color.
    ///
    /// Every exporter needs this: without it, a solid-color artwork emits one
    /// escape sequence (or one `<span>`, or one highlight group) per cell
    /// instead of one per run. Shared here so the four exporters cannot drift.
    pub fn row_segments(&self, y: usize) -> Vec<Segment> {
        let Some(row) = self.cells.get(y) else {
            return Vec::new();
        };

        let mut segments: Vec<Segment> = Vec::new();
        for cell in row {
            match segments.last_mut() {
                Some(seg) if seg.fg == cell.fg && seg.bg == cell.bg => seg.text.push(cell.ch),
                _ => segments.push(Segment {
                    fg: cell.fg,
                    bg: cell.bg,
                    text: cell.ch.to_string(),
                }),
            }
        }
        segments
    }
}

/// A run of consecutive cells sharing the same colors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub fg: Option<Rgb>,
    pub bg: Option<Rgb>,
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_with_and_without_hash() {
        assert_eq!(Rgb::parse("#ff8800").unwrap(), Rgb::new(255, 136, 0));
        assert_eq!(Rgb::parse("ff8800").unwrap(), Rgb::new(255, 136, 0));
        assert_eq!(Rgb::parse("000000").unwrap(), Rgb::new(0, 0, 0));
        assert!(Rgb::parse("#fff").is_err());
        assert!(Rgb::parse("#gggggg").is_err());
    }

    #[test]
    fn hex_roundtrip() {
        for s in ["#000000", "#ffffff", "#ff8800", "#123456"] {
            assert_eq!(Rgb::parse(s).unwrap().to_hex(), s);
        }
    }

    #[test]
    fn pure_colors_hit_cube_corners() {
        assert_eq!(Rgb::new(0, 0, 0).to_xterm256(), 16);
        assert_eq!(Rgb::new(255, 255, 255).to_xterm256(), 231);
        // Pure red is cube index 16 + 36*5 = 196.
        assert_eq!(Rgb::new(255, 0, 0).to_xterm256(), 196);
    }

    #[test]
    fn mid_gray_prefers_the_grayscale_ramp() {
        let idx = Rgb::new(128, 128, 128).to_xterm256();
        assert!((232..=255).contains(&idx), "got {idx}");
    }

    #[test]
    fn trim_end_removes_trailing_blanks_only() {
        let mut a = Artwork::from_lines(["ab   ", "  c  "]);
        a.trim_end();
        assert_eq!(a.cells[0].len(), 2);
        // Leading blanks survive; only the right edge is trimmed.
        assert_eq!(a.cells[1].len(), 3);
    }

    #[test]
    fn braille_blank_counts_as_blank() {
        // U+2800 is not the ASCII space; treating it as visible leaves a huge
        // invisible margin on every braille export.
        assert!(Cell::new('\u{2800}').is_blank());
        assert!(Cell::new(' ').is_blank());
        assert!(!Cell::new('⠁').is_blank(), "a single lit dot is not blank");
    }

    #[test]
    fn braille_blanks_are_trimmed_vertically() {
        let mut a = Artwork::from_lines(["\u{2800}\u{2800}\u{2800}", "\u{2800}⠿\u{2800}"]);
        a.trim_blank_rows();
        a.trim_end();
        assert_eq!(a.height(), 1, "the all-blank braille row should be dropped");
        // Leading blanks are deliberately kept: in a proportional figure they
        // are part of the shape, not margin, and there is no way to tell the
        // two apart. Only the trailing edge is safe to strip.
        assert_eq!(a.cells[0].len(), 2);
    }

    #[test]
    fn trim_blank_rows_removes_edges_only() {
        let mut a = Artwork::from_lines(["   ", " x ", "   "]);
        a.trim_blank_rows();
        assert_eq!(a.height(), 1);
        assert_eq!(a.to_plain_string(), " x ");
    }
}
