//! Character sets for block mode.
//!
//! Block mode rasterizes text with a real font and then maps pixels to
//! characters. The charset decides how many source pixels one character cell
//! stands for, which is what determines the effective resolution.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Charset {
    /// Classic density ramp: `" .:-=+*#%@"`.
    #[default]
    AsciiRamp,
    /// Solid blocks `█`: one cell, filled or empty. The chunkiest look, and
    /// the one that survives being drawn small.
    Blocks,
    /// Braille dots: 2x4 pixels per cell, the highest resolution available.
    Braille,
    /// Upper-half block with separate foreground and background colors, so one
    /// cell carries two independently colored pixels.
    HalfBlock,
    /// Quarter blocks: a 2x2 grid of sub-cells per character, with the inked
    /// ones drawn in the foreground color and the rest left transparent.
    ///
    /// Four shape sub-cells sits between half blocks (two) and Braille (eight),
    /// so it resolves more of a silhouette than a half block while keeping the
    /// chunky, solid look.
    Quadrant,
    /// Picks, per cell, the printable ASCII character whose drawn shape most
    /// resembles that cell. See [`super::shape`].
    ///
    /// Unlike the ramps this compares shape rather than darkness, so two cells
    /// with the same amount of ink but different outlines get different
    /// characters.
    Shape,
    /// Edges drawn with `/ \ | _ -`, chosen from the local gradient direction.
    ///
    /// This is the charset that matches how a FIGlet font draws: those same
    /// characters are what the banners are built from, so rasterized glyphs sit
    /// beside them without looking like a different medium.
    LineArt,
}

impl Charset {
    pub const ALL: [Charset; 7] = [
        Charset::AsciiRamp,
        Charset::Blocks,
        Charset::Braille,
        Charset::HalfBlock,
        Charset::Shape,
        Charset::Quadrant,
        Charset::LineArt,
    ];

    /// Stable English identifier, used in test failure messages. The UI labels
    /// come from `i18n` so they can be translated.
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            Charset::AsciiRamp => "ASCII ramp",
            Charset::Blocks => "Blocks",
            Charset::Braille => "Braille",
            Charset::HalfBlock => "Half block",
            Charset::Shape => "Shape matched",
            Charset::Quadrant => "Quarter blocks",
            Charset::LineArt => "Line art",
        }
    }

    /// Source pixels consumed by one character cell as `(width, height)`.
    ///
    /// Note the general case is 1x2, not 1x1: terminal cells are about twice as
    /// tall as they are wide, so sampling a square region would stretch the
    /// result vertically.
    pub fn pixels_per_cell(self) -> (usize, usize) {
        match self {
            // Four by eight source pixels per cell, so the eight-region
            // signature averages real pixels rather than an interpolation.
            Charset::Shape => (4, 8),
            // Same source sampling as Braille, so each of the four
            // sub-cells averages a real 1x2 block.
            Charset::Braille | Charset::Quadrant => (2, 4),
            Charset::AsciiRamp | Charset::Blocks | Charset::HalfBlock | Charset::LineArt => (1, 2),
        }
    }

    /// Whether this charset needs a color to work, rather than just coverage.
    pub fn needs_color(self) -> bool {
        matches!(self, Charset::HalfBlock)
    }

    /// Whether the coverage cutoff (`Options::threshold`) changes this
    /// charset's output.
    ///
    /// These are the charsets that decide each sub-cell with a yes/no question
    /// — is there ink here — rather than shading it with a ramp. It is what the
    /// UI checks to decide whether to offer the cutoff at all.
    pub fn uses_threshold(self) -> bool {
        matches!(
            self,
            Charset::Braille | Charset::Blocks | Charset::HalfBlock | Charset::Quadrant
        )
    }
}

/// Density ramp from lightest to darkest.
pub const ASCII_RAMP: &[char] = &[' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];

/// Pick the ramp character for a coverage value in `0.0..=1.0`.
pub fn ramp_char(ramp: &[char], coverage: f32) -> char {
    if ramp.is_empty() {
        return ' ';
    }
    let idx = (coverage.clamp(0.0, 1.0) * (ramp.len() - 1) as f32).round() as usize;
    ramp[idx.min(ramp.len() - 1)]
}

/// Quarter-block characters, indexed by which of the four sub-cells are inked.
///
/// Bit 0 is upper-left, bit 1 upper-right, bit 2 lower-left, bit 3 lower-right.
/// All sixteen combinations exist in Unicode, so the shape is always exact: the
/// character's own gaps provide the transparency, and no background color is
/// needed.
const QUADRANTS: [char; 16] = [
    ' ',         // 0000
    '\u{2598}',  // 0001 upper left
    '\u{259d}',  // 0010 upper right
    '\u{2580}',  // 0011 upper half
    '\u{2596}',  // 0100 lower left
    '\u{258c}',  // 0101 left half
    '\u{259e}',  // 0110 upper right + lower left
    '\u{259b}',  // 0111 all but lower right
    '\u{2597}',  // 1000 lower right
    '\u{259a}',  // 1001 upper left + lower right
    '\u{2590}',  // 1010 right half
    '\u{259c}',  // 1011 all but lower left
    '\u{2584}',  // 1100 lower half
    '\u{2599}',  // 1101 all but upper right
    '\u{259f}',  // 1110 all but upper left
    '\u{2588}',  // 1111 full block
];

/// Every quarter-block character. Used by the tests to assert that the renderer
/// emits nothing else; the renderer itself goes through [`quadrant_char`].
#[allow(dead_code)]
pub const QUADRANT_CHARS: [char; 16] = QUADRANTS;

/// The quarter-block character for a four-bit ink mask.
pub fn quadrant_char(mask: u8) -> char {
    QUADRANTS[(mask & 0b1111) as usize]
}

/// Unicode Braille Patterns start here; the block is contiguous and fully
/// assigned, so every offset in `0..=0xFF` is a valid character.
pub const BRAILLE_BASE: u32 = 0x2800;

/// Dot position to bit value, indexed `[column][row]` with row 0 at the top.
///
/// The layout is not raster order: dots 7 and 8 are the bottom row but occupy
/// bits 6 and 7, which is the single easiest thing to get wrong here.
const DOT_BITS: [[u32; 4]; 2] = [
    [0x01, 0x02, 0x04, 0x40], // dots 1, 2, 3, 7
    [0x08, 0x10, 0x20, 0x80], // dots 4, 5, 6, 8
];

/// Map a 2x4 boolean block to a Braille character.
pub fn braille_cell(bits: [[bool; 4]; 2]) -> char {
    let mut acc = 0u32;
    for (col, column) in bits.iter().enumerate() {
        for (row, on) in column.iter().enumerate() {
            if *on {
                acc |= DOT_BITS[col][row];
            }
        }
    }
    // Every value in 0x2800..=0x28FF has a character, so this cannot fail.
    char::from_u32(BRAILLE_BASE + acc).expect("braille block is fully assigned")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn braille_all_dots_on_is_the_last_codepoint() {
        assert_eq!(braille_cell([[true; 4]; 2]), '⣿');
        assert_eq!(braille_cell([[true; 4]; 2]) as u32, 0x28FF);
    }

    #[test]
    fn braille_all_dots_off_is_the_blank() {
        assert_eq!(braille_cell([[false; 4]; 2]), '⠀');
        assert_eq!(braille_cell([[false; 4]; 2]) as u32, 0x2800);
    }

    #[test]
    fn braille_dot_one_is_bit_zero() {
        let mut bits = [[false; 4]; 2];
        bits[0][0] = true;
        assert_eq!(braille_cell(bits) as u32, BRAILLE_BASE + 0x01);
    }

    #[test]
    fn braille_bottom_left_is_dot_seven_not_dot_four() {
        // The classic mistake: dot 7 is the 4th dot down the left column, and
        // its bit is 0x40, not 0x04.
        let mut bits = [[false; 4]; 2];
        bits[0][3] = true;
        assert_eq!(braille_cell(bits) as u32, BRAILLE_BASE + 0x40);
    }

    #[test]
    fn braille_bottom_right_is_dot_eight() {
        let mut bits = [[false; 4]; 2];
        bits[1][3] = true;
        assert_eq!(braille_cell(bits) as u32, BRAILLE_BASE + 0x80);
    }

    #[test]
    fn braille_right_column_uses_the_high_nibble() {
        let mut bits = [[false; 4]; 2];
        bits[1][0] = true;
        assert_eq!(braille_cell(bits) as u32, BRAILLE_BASE + 0x08);
    }

    #[test]
    fn braille_never_leaves_the_block() {
        // Exhaustive: every one of the 256 dot combinations must be valid.
        for mask in 0u32..256 {
            let mut bits = [[false; 4]; 2];
            let mut m = mask;
            for col in 0..2 {
                for row in 0..4 {
                    bits[col][row] = m & 1 == 1;
                    m >>= 1;
                }
            }
            let c = braille_cell(bits) as u32;
            assert!((BRAILLE_BASE..=BRAILLE_BASE + 0xFF).contains(&c));
        }
    }

    #[test]
    fn every_quadrant_mask_maps_to_a_distinct_character() {
        // Sixteen inked combinations, sixteen characters: if two masks collide
        // the renderer would silently lose a sub-cell resolution.
        let chars: Vec<char> = (0..16u8).map(quadrant_char).collect();
        let mut sorted = chars.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 16, "duplicate quadrant characters: {chars:?}");
    }

    #[test]
    fn quadrant_masks_match_the_unicode_names() {
        // The bits are easy to permute; check each single-quadrant mask against
        // the character Unicode actually names for it.
        assert_eq!(quadrant_char(0b0000), ' ');
        assert_eq!(quadrant_char(0b0001), '\u{2598}', "upper left");
        assert_eq!(quadrant_char(0b0010), '\u{259d}', "upper right");
        assert_eq!(quadrant_char(0b0100), '\u{2596}', "lower left");
        assert_eq!(quadrant_char(0b1000), '\u{2597}', "lower right");
        // Pairs and triples use the half and three-quarter blocks.
        assert_eq!(quadrant_char(0b0011), '\u{2580}', "both upper");
        assert_eq!(quadrant_char(0b1100), '\u{2584}', "both lower");
        assert_eq!(quadrant_char(0b0101), '\u{258c}', "both left");
        assert_eq!(quadrant_char(0b1010), '\u{2590}', "both right");
        assert_eq!(quadrant_char(0b1111), '\u{2588}', "all four");
    }

    #[test]
    fn quadrant_masks_ignore_bits_above_the_fourth() {
        assert_eq!(quadrant_char(0b1111_1111), quadrant_char(0b1111));
    }

    #[test]
    fn only_the_cutting_charsets_use_the_cutoff() {
        // The UI offers the cutoff for exactly these. A ramp answers with a
        // shade and the matcher with a shape, and a cutoff on either would only
        // quietly change its weight.
        for cs in [Charset::Braille, Charset::Blocks, Charset::HalfBlock, Charset::Quadrant] {
            assert!(cs.uses_threshold(), "{} should offer the cutoff", cs.label());
        }
        for cs in [Charset::AsciiRamp, Charset::Shape, Charset::LineArt] {
            assert!(!cs.uses_threshold(), "{} should not offer the cutoff", cs.label());
        }
    }

    #[test]
    fn quadrant_needs_no_background_color() {
        // A half block encodes one of its two pixels as the background; a
        // quarter block's own gaps are transparent, so it works on a terminal
        // that ignores background colors where half-block does not.
        assert!(!Charset::Quadrant.needs_color());
        assert!(Charset::HalfBlock.needs_color());
    }

    #[test]
    fn ramp_char_covers_both_ends() {
        assert_eq!(ramp_char(ASCII_RAMP, 0.0), ' ');
        assert_eq!(ramp_char(ASCII_RAMP, 1.0), '@');
    }

    #[test]
    fn ramp_char_clamps_out_of_range_input() {
        assert_eq!(ramp_char(ASCII_RAMP, -5.0), ' ');
        assert_eq!(ramp_char(ASCII_RAMP, 5.0), '@');
    }

    #[test]
    fn ramp_char_is_monotonic() {
        // Density must never go down as coverage goes up, or the output looks
        // noisy rather than shaded.
        let mut prev = 0usize;
        for i in 0..=100 {
            let c = ramp_char(ASCII_RAMP, i as f32 / 100.0);
            let idx = ASCII_RAMP.iter().position(|x| *x == c).expect("in ramp");
            assert!(idx >= prev, "density went backwards at coverage {i}");
            prev = idx;
        }
    }

    #[test]
    fn ramp_char_handles_an_empty_ramp() {
        assert_eq!(ramp_char(&[], 0.5), ' ');
    }

    #[test]
    fn braille_is_the_highest_resolution_charset() {
        let braille = Charset::Braille.pixels_per_cell();
        let ascii = Charset::AsciiRamp.pixels_per_cell();
        assert!(braille.0 * braille.1 > ascii.0 * ascii.1);
    }

    #[test]
    fn cells_are_taller_than_they_are_wide() {
        // Matches how a terminal renders, so block mode is not vertically squashed.
        for cs in Charset::ALL {
            let (w, h) = cs.pixels_per_cell();
            assert!(h >= w, "{} has cells wider than tall", cs.label());
        }
    }
}
