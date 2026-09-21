//! Neovim Lua export.
//!
//! Dashboard plugins disagree about how much color they accept: alpha.nvim and
//! dashboard-nvim take one highlight group per *line*, while snacks.nvim takes a
//! `virt_text` array of per-*chunk* groups. So this emits all three shapes
//! (`lines`, `virt_text`, `header`) and lets the user pick.
//!
//! Group count is the real cost here. A 60-wide gradient has a distinct color in
//! nearly every cell, so emitting one group per cell would mean thousands of
//! `nvim_set_hl` calls on every reload. Colors are quantized into buckets first,
//! which collapses a smooth ramp to a couple of dozen groups with no visible
//! difference.

use crate::model::{Artwork, Rgb, Segment};
use std::collections::HashMap;

/// Quantization step per channel. 8 gives 32 levels per channel, which is well
/// below the threshold where banding becomes visible on a gradient.
const QUANT_STEP: u8 = 8;

/// Prefix for generated highlight groups, so they cannot collide with a
/// colorscheme's own groups.
const PREFIX: &str = "Wart";

fn quantize(c: Rgb) -> Rgb {
    let q = |v: u8| (v / QUANT_STEP) * QUANT_STEP;
    Rgb::new(q(c.r), q(c.g), q(c.b))
}

/// The single color that best represents a line, used for per-line groups.
/// Averaging the colored cells beats taking the first one: on a gradient the
/// first cell is always the same color, which would flatten every line.
fn average_color(segments: &[Segment]) -> Option<Rgb> {
    let mut r = 0u32;
    let mut g = 0u32;
    let mut b = 0u32;
    let mut n = 0u32;
    for seg in segments {
        if let Some(c) = seg.fg {
            let count = seg.text.chars().filter(|ch| *ch != ' ').count() as u32;
            r += c.r as u32 * count;
            g += c.g as u32 * count;
            b += c.b as u32 * count;
            n += count;
        }
    }
    if n == 0 {
        return None;
    }
    Some(Rgb::new((r / n) as u8, (g / n) as u8, (b / n) as u8))
}

/// Escape a string for a Lua double-quoted literal.
///
/// The ASCII-ramp charsets include a backslash, and control characters would
/// corrupt the file outright, so both need handling.
fn lua_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\u{{{:X}}}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

pub fn export(art: &Artwork) -> String {
    let height = art.height();
    let mut out = String::new();

    out.push_str("-- wart -- generated ASCII logo. Regenerate rather than editing by hand.\n");
    out.push_str("-- Exact colors need truecolor: vim.o.termguicolors = true\n");
    out.push_str("--\n");
    out.push_str("-- dashboard-nvim:\n");
    out.push_str("--   require(\"dashboard\").setup({ config = { header = require(\"wart_logo\").header } })\n");
    out.push_str("-- snacks.nvim:\n");
    out.push_str("--   { section = \"header\", text = require(\"wart_logo\").virt_text }\n");
    out.push_str("-- alpha.nvim:\n");
    out.push_str("--   { type = \"text\", val = require(\"wart_logo\").lines, opts = { hl = \"WartLine1\" } }\n");
    out.push_str("\nlocal M = {}\n\n");

    // --- lines -------------------------------------------------------------
    out.push_str("M.lines = {\n");
    for y in 0..height {
        out.push_str(&format!("  \"{}\",\n", lua_escape(&art.row_text(y))));
    }
    out.push_str("}\n\n");

    // --- highlight groups --------------------------------------------------
    // One map keyed by quantized color keeps per-line and per-segment groups
    // from duplicating each other when they land on the same color.
    let mut groups: HashMap<(u8, u8, u8), String> = HashMap::new();
    let mut declarations = String::new();
    let mut next_id = 0usize;

    let mut intern = |color: Rgb, declarations: &mut String, next_id: &mut usize| -> String {
        let key = {
            let q = quantize(color);
            (q.r, q.g, q.b)
        };
        groups
            .entry(key)
            .or_insert_with(|| {
                let name = format!("{PREFIX}C{}", *next_id);
                *next_id += 1;
                let cterm = color.to_xterm256();
                declarations.push_str(&format!(
                    "vim.api.nvim_set_hl(0, \"{name}\", {{ fg = \"{}\", ctermfg = {cterm} }})\n",
                    color.to_hex()
                ));
                name
            })
            .clone()
    };

    let rows: Vec<Vec<Segment>> = (0..height).map(|y| art.row_segments(y)).collect();

    let mut line_groups = Vec::with_capacity(height);
    for segments in &rows {
        match average_color(segments) {
            Some(c) => line_groups.push(Some(intern(c, &mut declarations, &mut next_id))),
            None => line_groups.push(None),
        }
    }

    let mut virt_rows: Vec<Vec<(String, String)>> = Vec::with_capacity(height);
    for segments in &rows {
        let mut chunks = Vec::new();
        for seg in segments {
            let Some(fg) = seg.fg else {
                continue;
            };
            if seg.text.trim().is_empty() {
                continue;
            }
            let name = intern(fg, &mut declarations, &mut next_id);
            chunks.push((seg.text.clone(), name));
        }
        virt_rows.push(chunks);
    }

    out.push_str("-- Highlight groups, deduplicated by quantized color.\n");
    out.push_str(&declarations);
    out.push('\n');

    // --- per-line groups, for alpha.nvim / dashboard-nvim ------------------
    out.push_str("M.line_hl = {\n");
    for name in &line_groups {
        match name {
            Some(n) => out.push_str(&format!("  \"{n}\",\n")),
            None => out.push_str("  nil,\n"),
        }
    }
    out.push_str("}\n\n");

    // --- virt_text, for snacks.nvim ---------------------------------------
    out.push_str("M.virt_text = {\n");
    for chunks in &virt_rows {
        out.push_str("  {");
        for (text, name) in chunks {
            out.push_str(&format!(" {{ \"{}\", \"{name}\" }},", lua_escape(text)));
        }
        out.push_str(" },\n");
    }
    out.push_str("}\n\n");

    // --- ready-made dashboard-nvim header entry ----------------------------
    out.push_str("M.header = {\n  {\n    type = \"text\",\n    val = M.lines,\n");
    out.push_str("    opts = { position = \"center\" },\n  },\n}\n\n");
    out.push_str("return M\n");

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colored() -> Artwork {
        let mut art = Artwork::from_lines(["ab", "cd"]);
        for row in &mut art.cells {
            for c in row.iter_mut() {
                c.fg = Some(Rgb::new(255, 0, 0));
            }
        }
        art
    }

    #[test]
    fn emits_a_returnable_module() {
        let out = export(&colored());
        assert!(out.starts_with("-- wart"));
        assert!(out.trim_end().ends_with("return M"));
    }

    #[test]
    fn lines_are_lua_string_literals() {
        let out = export(&colored());
        assert!(out.contains("\"ab\","), "got:\n{out}");
        assert!(out.contains("\"cd\","));
    }

    #[test]
    fn identical_colors_share_one_group() {
        // A solid-color artwork must not emit a group per cell.
        let out = export(&colored());
        let count = out.matches("nvim_set_hl").count();
        assert_eq!(count, 1, "expected a single deduplicated group, got {count}\n{out}");
    }

    #[test]
    fn a_gradient_stays_within_a_bounded_group_count() {
        let mut art = Artwork::from_lines(["x".repeat(200).as_str()]);
        crate::color::colorize(&mut art, &crate::color::ColorStyle::linear(
            Rgb::BLACK,
            Rgb::WHITE,
            0.0,
        ));
        let out = export(&art);
        let count = out.matches("nvim_set_hl").count();
        // 256 raw colors on a black-to-white ramp, but quantization buckets them.
        // Anything near one-per-cell would defeat the purpose.
        assert!(count < 80, "quantization failed to collapse the ramp: {count} groups");
        assert!(count > 1, "a gradient should still produce several groups");
    }

    #[test]
    fn escapes_backslash_and_quote() {
        assert_eq!(lua_escape("a\\b"), "a\\\\b");
        assert_eq!(lua_escape("\"x\""), "\\\"x\\\"");
    }

    #[test]
    fn escapes_control_characters() {
        assert_eq!(lua_escape("\u{1}"), "\\u{1}");
        assert_eq!(lua_escape("\n"), "\\n");
    }

    #[test]
    fn uncolored_artwork_emits_no_groups() {
        let out = export(&Artwork::from_lines(["ab"]));
        assert_eq!(out.matches("nvim_set_hl").count(), 0);
        assert!(out.contains("nil,"), "line_hl should have a nil placeholder");
    }
}
