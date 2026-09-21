//! SVG export: vector output, so the logo stays sharp at any size.
//!
//! Each row is one `<text>` element with a `<tspan>` per color run. The font is
//! declared monospace, which is what keeps the columns aligned — the glyphs
//! themselves are never positioned individually.

use crate::model::{Artwork, Rgb};

pub struct Options {
    pub font_size: f32,
    /// Width of one cell. Defaults to a typical monospace advance (0.6em).
    pub cell_width: f32,
    /// Height of one row. Terminal cells are about twice as tall as wide.
    pub cell_height: f32,
    pub background: Option<Rgb>,
    /// Font stack. The Nerd Font matters when the art contains icon glyphs.
    pub font_family: String,
}

impl Default for Options {
    fn default() -> Self {
        let font_size = 14.0;
        Self {
            font_size,
            cell_width: font_size * 0.6,
            cell_height: font_size * 1.2,
            background: None,
            font_family: "'JetBrainsMono Nerd Font', ui-monospace, monospace".into(),
        }
    }
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

pub fn export(art: &Artwork, opts: &Options) -> String {
    let (w, h) = (art.width(), art.height());
    let width = (w as f32 * opts.cell_width).max(1.0);
    let height = (h as f32 * opts.cell_height).max(1.0);

    let mut out = String::new();
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" \
         viewBox=\"0 0 {width:.0} {height:.0}\">\n"
    ));

    if let Some(bg) = opts.background {
        out.push_str(&format!(
            "  <rect width=\"{width:.0}\" height=\"{height:.0}\" fill=\"{}\"/>\n",
            bg.to_hex()
        ));
    }

    out.push_str(&format!(
        "  <g font-family=\"{}\" font-size=\"{}\" xml:space=\"preserve\">\n",
        opts.font_family, opts.font_size
    ));

    // The first baseline sits one ascent below the top of the first row. Using
    // 0.8em is the usual approximation and lines up with how terminals render.
    let ascent = opts.font_size * 0.8;
    for y in 0..h {
        let baseline = y as f32 * opts.cell_height + ascent;
        let segments = art.row_segments(y);
        if segments.iter().all(|s| s.text.trim().is_empty() && s.fg.is_none()) {
            continue;
        }
        out.push_str(&format!("    <text x=\"0\" y=\"{baseline:.2}\">"));
        for seg in segments {
            let text = escape(&seg.text);
            match seg.fg {
                Some(fg) => out.push_str(&format!(
                    "<tspan fill=\"{}\">{text}</tspan>",
                    fg.to_hex()
                )),
                None => out.push_str(&format!("<tspan>{text}</tspan>")),
            }
        }
        out.push_str("</text>\n");
    }

    out.push_str("  </g>\n</svg>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_a_well_formed_svg_root() {
        let out = export(&Artwork::from_lines(["ab"]), &Options::default());
        assert!(out.starts_with("<svg xmlns="));
        assert!(out.trim_end().ends_with("</svg>"));
    }

    #[test]
    fn size_follows_the_grid() {
        let art = Artwork::from_lines(["abc", "def"]);
        let opts = Options { cell_width: 10.0, cell_height: 20.0, ..Default::default() };
        let out = export(&art, &opts);
        assert!(out.contains("width=\"30\""), "3 columns * 10px\n{out}");
        assert!(out.contains("height=\"40\""), "2 rows * 20px\n{out}");
    }

    #[test]
    fn colors_become_tspan_fills() {
        let mut art = Artwork::from_lines(["a"]);
        art.cells[0][0].fg = Some(Rgb::new(255, 136, 0));
        let out = export(&art, &Options::default());
        assert!(out.contains("<tspan fill=\"#ff8800\">a</tspan>"), "got:\n{out}");
    }

    #[test]
    fn markup_characters_are_escaped() {
        let out = export(&Artwork::from_lines(["<&>"]), &Options::default());
        assert!(out.contains("&lt;&amp;&gt;"));
        assert!(!out.contains("<&>"));
    }

    #[test]
    fn blank_rows_are_skipped() {
        let art = Artwork::from_lines(["a", "   ", "b"]);
        let out = export(&art, &Options::default());
        assert_eq!(out.matches("<text").count(), 2, "blank middle row should be dropped");
    }

    #[test]
    fn background_adds_a_rect() {
        let opts = Options { background: Some(Rgb::BLACK), ..Default::default() };
        let out = export(&Artwork::from_lines(["a"]), &opts);
        assert!(out.contains("<rect"));
        assert!(out.contains("fill=\"#000000\""));
    }
}
