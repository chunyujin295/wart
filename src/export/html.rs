//! HTML export: a `<pre>` block with inline colors, so the file is
//! self-contained and pastes straight into a page or a README.

use crate::model::{Artwork, Rgb};

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

pub fn export(art: &Artwork, background: Option<Rgb>) -> String {
    let mut out = String::new();
    out.push_str("<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n");
    out.push_str("<title>wart</title>\n<style>\n");
    out.push_str("pre { font-family: ui-monospace, \"JetBrainsMono Nerd Font\", monospace;\n");
    out.push_str("      font-size: 14px; line-height: 1.15; margin: 0; padding: 1rem; }\n");
    out.push_str("</style>\n</head>\n<body>\n<pre>");

    if let Some(bg) = background {
        out.push_str(&format!(
            "<span style=\"background:{}\">",
            bg.to_hex()
        ));
    }

    for y in 0..art.height() {
        if y > 0 {
            out.push('\n');
        }
        for seg in art.row_segments(y) {
            let text = escape(&seg.text);
            match (seg.fg, seg.bg) {
                (None, None) => out.push_str(&text),
                (fg, bg) => {
                    let mut style = String::new();
                    if let Some(fg) = fg {
                        style.push_str(&format!("color:{}", fg.to_hex()));
                    }
                    if let Some(bg) = bg {
                        if !style.is_empty() {
                            style.push(';');
                        }
                        style.push_str(&format!("background:{}", bg.to_hex()));
                    }
                    out.push_str(&format!("<span style=\"{style}\">{text}</span>"));
                }
            }
        }
    }

    if background.is_some() {
        out.push_str("</span>");
    }

    out.push_str("</pre>\n</body>\n</html>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uncolored_text_has_no_span() {
        let out = export(&Artwork::from_lines(["ab"]), None);
        assert!(out.contains("<pre>ab"));
        assert!(!out.contains("<span"));
    }

    #[test]
    fn colors_become_inline_styles() {
        let mut art = Artwork::from_lines(["a"]);
        art.cells[0][0].fg = Some(Rgb::new(255, 136, 0));
        let out = export(&art, None);
        assert!(out.contains("<span style=\"color:#ff8800\">a</span>"), "got:\n{out}");
    }

    #[test]
    fn markup_characters_are_escaped() {
        let out = export(&Artwork::from_lines(["<a>&"]), None);
        assert!(out.contains("&lt;a&gt;&amp;"));
        // The raw sequence must not survive, or it would break the page.
        assert!(!out.contains("<a>"));
    }

    #[test]
    fn background_wraps_the_whole_block() {
        let out = export(&Artwork::from_lines(["a"]), Some(Rgb::BLACK));
        assert!(out.contains("<span style=\"background:#000000\">"));
        assert!(out.contains("</span></pre>"));
    }

    #[test]
    fn adjacent_cells_with_the_same_color_share_a_span() {
        let mut art = Artwork::from_lines(["abc"]);
        for c in art.cells[0].iter_mut() {
            c.fg = Some(Rgb::new(255, 0, 0));
        }
        let out = export(&art, None);
        assert_eq!(out.matches("<span").count(), 1);
        assert!(out.contains(">abc</span>"));
    }
}
