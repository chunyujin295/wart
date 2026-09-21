//! Exporters. Every format consumes an [`Artwork`] and knows nothing about how
//! it was generated.

pub mod ansi;
pub mod fastfetch;
pub mod html;
pub mod lua;
pub mod plain;
pub mod png;
pub mod svg;

use crate::model::{Artwork, Rgb};
use clap::ValueEnum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// Plain text with embedded SGR escapes — `fastfetch --file`, terminal `cat`.
    Ansi,
    /// The characters alone, with no escape sequences. For anywhere that would
    /// print an escape as literal garbage: a commit message, a README, a chat.
    Plain,
    /// Lua module with highlight groups, for a Neovim dashboard.
    Lua,
    /// Rasterized image of the artwork.
    Png,
    /// A fastfetch configuration that points at an ANSI export.
    Fastfetch,
    /// `<pre>` with inline colors.
    Html,
    /// Vector version of the PNG.
    Svg,
}

impl Format {
    pub const ALL: [Format; 7] = [
        Format::Ansi,
        Format::Plain,
        Format::Lua,
        Format::Fastfetch,
        Format::Png,
        Format::Html,
        Format::Svg,
    ];

    pub fn extension(self) -> &'static str {
        match self {
            // Not `txt`: a file full of escape sequences is not plain text, and
            // the plain format below wants that name.
            Format::Ansi => "ansi",
            Format::Plain => "txt",
            // JSONC, which is what fastfetch's own generated config uses.
            Format::Fastfetch => "jsonc",
            Format::Lua => "lua",
            Format::Png => "png",
            Format::Html => "html",
            Format::Svg => "svg",
        }
    }

}

#[derive(Debug, Clone)]
pub struct Options {
    pub color: bool,
    pub xterm256: bool,
    /// Stem of the ANSI file this configuration should point at, so the two
    /// exports can be told apart without the user having to keep them in sync
    /// by hand. Also names the companion file written beside the config.
    pub logo_name: String,
    /// Exactly what a fastfetch config should write into `logo.source`.
    ///
    /// Not derived from `logo_name` at the point of use, because the right
    /// answer depends on where the files are going: fastfetch resolves a
    /// relative source against its *working directory*, so a saved config wants
    /// an absolute path and a config printed to stdout wants a bare filename.
    pub logo_source: String,
    /// Painted behind the artwork in the image formats, and as a `<span>` /
    /// `<rect>` background in HTML and SVG. Ignored by the text formats, where
    /// the terminal owns the background.
    pub background: Option<Rgb>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            color: true,
            xterm256: false,
            background: None,
            logo_name: "logo".to_owned(),
            logo_source: "logo.txt".to_owned(),
        }
    }
}

/// Filename of the ANSI logo a fastfetch config points at.
///
/// Shared so the name the config writes and the name that actually gets written
/// cannot drift apart — and so the extension matches what the ANSI format would
/// have suggested, rather than being `.txt` in one place and `.ansi` in another.
pub fn logo_file_name(stem: &str) -> String {
    format!("{stem}.{}", Format::Ansi.extension())
}

/// Files an export needs written alongside it, as `(path, bytes)`.
///
/// A fastfetch config is useless without the logo it points at, and asking the
/// user to run a second export and get the two names to match is a footgun. The
/// caller knows where the main file is going, so it can place the companion.
pub fn companions(
    art: &Artwork,
    format: Format,
    opts: &Options,
    path: &std::path::Path,
) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    match format {
        Format::Fastfetch => {
            let sibling = path.with_file_name(logo_file_name(&opts.logo_name));
            let ansi = ansi::export(art, &ansi::Options {
                color: opts.color,
                xterm256: opts.xterm256,
            });
            vec![(sibling, ansi.into_bytes())]
        }
        _ => Vec::new(),
    }
}

/// Returns bytes rather than `String` so binary formats (PNG) can share the
/// same entry point as the text ones.
pub fn export(art: &Artwork, format: Format, opts: &Options) -> anyhow::Result<Vec<u8>> {
    match format {
        Format::Ansi => Ok(ansi::export(art, &ansi::Options {
            color: opts.color,
            xterm256: opts.xterm256,
        })
        .into_bytes()),
        Format::Plain => Ok(plain::export(art).into_bytes()),
        Format::Lua => Ok(lua::export(art).into_bytes()),
        Format::Fastfetch => Ok(fastfetch::export(art, opts).into_bytes()),
        Format::Png => png::export(art, &png::Options {
            background: opts.background,
            ..Default::default()
        }),
        Format::Html => Ok(html::export(art, opts.background).into_bytes()),
        Format::Svg => Ok(svg::export(art, &svg::Options {
            background: opts.background,
            ..Default::default()
        })
        .into_bytes()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_fastfetch_config_has_a_companion() {
        let art = Artwork::from_lines(["ab"]);
        let opts = Options::default();
        let path = std::path::Path::new("/tmp/logo.jsonc");

        for format in Format::ALL {
            let companions = companions(&art, format, &opts, path);
            if format == Format::Fastfetch {
                assert_eq!(companions.len(), 1, "the config needs its logo");
            } else {
                assert!(companions.is_empty(), "{format:?} should stand alone");
            }
        }
    }

    #[test]
    fn the_companion_sits_beside_the_config_and_matches_its_source() {
        let art = Artwork::from_lines(["ab"]);
        let opts = Options {
            logo_name: "wart".to_owned(),
            logo_source: "C:/somewhere/wart.ansi".to_owned(),
            ..Default::default()
        };
        let path = std::path::Path::new("/tmp/wart.jsonc");

        let companions = companions(&art, Format::Fastfetch, &opts, path);
        let (companion, data) = &companions[0];

        // The companion is named from the config it sits beside, with the ANSI
        // format's extension rather than `.txt`.
        assert_eq!(companion, std::path::Path::new("/tmp/wart.ansi"));
        // ...and the config points wherever `logo_source` says, which is an
        // absolute path by the time a file is actually written.
        let config = String::from_utf8(export(&art, Format::Fastfetch, &opts).unwrap()).unwrap();
        assert!(config.contains("\"source\": \"C:/somewhere/wart.ansi\""));
        assert!(!data.is_empty());
    }

    #[test]
    fn the_companion_is_the_ansi_export() {
        let mut art = Artwork::from_lines(["ab"]);
        art.cells[0][0].fg = Some(Rgb::new(255, 0, 0));
        let opts = Options::default();
        let (_, data) = &companions(
            &art,
            Format::Fastfetch,
            &opts,
            std::path::Path::new("/tmp/x.jsonc"),
        )[0];
        let expected = ansi::export(&art, &ansi::Options { color: true, xterm256: false });
        assert_eq!(data, expected.as_bytes());
    }

    #[test]
    fn every_format_has_a_distinct_extension() {
        let mut exts: Vec<_> = Format::ALL.iter().map(|f| f.extension()).collect();
        exts.sort_unstable();
        exts.dedup();
        assert_eq!(exts.len(), Format::ALL.len());
    }
}
