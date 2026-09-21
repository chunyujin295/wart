//! Plain text export: the characters and nothing else.
//!
//! What `--format ansi --no-color` produces, promoted to a format of its own so
//! it cannot pick up colour by accident. It is what you want when the art is
//! going somewhere that would render an escape sequence as literal garbage —
//! a Git commit message, a README, a chat window, a source comment.

use crate::model::Artwork;

pub fn export(art: &Artwork) -> String {
    let mut out = art.to_plain_string();
    // A trailing newline, because the file is line-oriented and most tools
    // treat a missing final newline as a mistake.
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Rgb;

    #[test]
    fn it_keeps_the_characters_and_drops_the_colour() {
        let mut art = Artwork::from_lines(["ab", "cd"]);
        art.cells[0][0].fg = Some(Rgb::new(255, 0, 0));
        art.cells[1][1].bg = Some(Rgb::new(0, 0, 255));

        let out = export(&art);
        assert_eq!(out, "ab\ncd\n");
        assert!(!out.contains('\x1b'), "no escape sequences belong in plain text");
    }

    #[test]
    fn it_ends_with_exactly_one_newline() {
        let out = export(&Artwork::from_lines(["a"]));
        assert!(out.ends_with('\n'));
        assert!(!out.ends_with("\n\n"));
    }

    #[test]
    fn it_matches_the_uncoloured_ansi_export() {
        // The two formats describe the same thing; if they ever disagree, one
        // of them is wrong.
        let mut art = Artwork::from_lines(["ab", "cd"]);
        art.cells[0][1].fg = Some(Rgb::new(1, 2, 3));

        let plain = export(&art);
        let ansi = crate::export::ansi::export(
            &art,
            &crate::export::ansi::Options { color: false, xterm256: false },
        );
        assert_eq!(plain, ansi);
    }

    #[test]
    fn an_empty_artwork_still_produces_a_valid_file() {
        assert_eq!(export(&Artwork::new()), "\n");
    }
}
