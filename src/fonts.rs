//! FIGlet font registry: the fonts compiled into the binary, plus any `.flf`
//! the user drops into the external font directory.

use anyhow::{Context, Result};
use figlet_rs::FIGlet;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

/// Number of characters in a classic FIGfont body.
const CLASSIC_CHARS: usize = 102;

/// Rewrite a FIGfont into the shape `figlet-rs` expects.
///
/// Real-world font files trip three separate assumptions in the parser:
///
/// * The codetag extension appends extra character sets after the classic
///   102-glyph body, and the parser only accepts the result when the trailing
///   data is an exact multiple of `height + 1` — which rejects almost every
///   font that uses the extension, including figlet.js's `Standard.flf`.
/// * That check runs even when the header declares *zero* codetags, so a font
///   whose file merely ends in a newline trips it too (`Broadway KB`).
/// * The header is sliced by byte, so a non-ASCII hardblank panics outright
///   (`Pyramid`) and a UTF-8 BOM shifts every offset (`Font Font`).
///
/// Truncating to `1 + comments + 102 * height` lines handles the first two: the
/// parser sees an empty codetag section and stops early. The dropped glyphs are
/// alternate character sets we never render, so output is unaffected.
///
/// Done at load time rather than at vendor time so the shipped `.flf` files stay
/// byte-identical to upstream.
fn normalize_figfont(content: &str) -> Cow<'_, str> {
    // A leading BOM shifts every byte offset on the header line.
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);

    let Some(header) = content.lines().next() else {
        return Cow::Borrowed(content);
    };
    if !header.starts_with("flf2a") {
        return Cow::Borrowed(content);
    }

    // signature height baseline maxlen oldlayout commentlines [printdir fulllayout codetagcount]
    let fields: Vec<&str> = header.split_whitespace().collect();
    let number = |i: usize| fields.get(i).and_then(|s| s.parse::<i64>().ok());
    let (Some(height), Some(comment_lines)) = (number(1), number(5)) else {
        return Cow::Borrowed(content);
    };
    if height <= 0 {
        return Cow::Borrowed(content);
    }

    // The signature's last character is the hardblank. `Pyramid` declares the
    // CP437 byte 0x81, which decodes to `ü`; DEL is single-byte ASCII and can
    // never appear in glyph art, so it substitutes without colliding with the
    // `$` that font also uses as ordinary art.
    let hardblank = fields[0].chars().last().filter(|c| !c.is_ascii());

    let body_end = 1 + comment_lines.max(0) as usize + CLASSIC_CHARS * height as usize;
    let lines: Vec<&str> = content.split('\n').collect();

    if hardblank.is_none() && lines.len() <= body_end {
        return Cow::Borrowed(content);
    }

    let mut out = String::with_capacity(content.len() + 1);
    // Drop the codetag count when one is declared, so the parser is not told to
    // expect a section that was just removed.
    if number(8).is_some_and(|c| c > 0) {
        out.push_str(&fields[..8].join(" "));
    } else {
        out.push_str(header);
    }
    for line in lines.iter().take(body_end).skip(1) {
        out.push('\n');
        out.push_str(line);
    }
    out.push('\n');

    match hardblank {
        Some(hb) => Cow::Owned(out.replace(hb, "\u{7f}")),
        None => Cow::Owned(out),
    }
}

include!(concat!(env!("OUT_DIR"), "/fonts_generated.rs"));

/// Names of the fonts compiled into the binary, sorted.
pub fn builtin_names() -> impl Iterator<Item = &'static str> {
    BUILTIN_FONTS.iter().map(|(name, _)| *name)
}

/// Look up a font by name, ignoring case.
///
/// The upstream font set uses display-style names (`ANSI Shadow`, `Standard`),
/// but those are awkward to type on a command line, so `--font standard` has to
/// resolve to `Standard`.
fn find_builtin(name: &str) -> Option<&'static (&'static str, &'static str)> {
    BUILTIN_FONTS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
}

pub fn is_builtin(name: &str) -> bool {
    find_builtin(name).is_some()
}

/// Parse font source text into a renderer, working around the codetag quirk.
fn parse(canonical: &str, content: &str) -> Result<FIGlet> {
    FIGlet::from_content(&normalize_figfont(content))
        .map_err(|e| anyhow::anyhow!("failed to parse font {canonical:?}: {e}"))
}

pub fn load_builtin(name: &str) -> Result<FIGlet> {
    let (canonical, content) = find_builtin(name)
        .with_context(|| format!("no built-in font named {name:?}"))?;
    parse(canonical, content)
}

/// Where user-supplied `.flf` files live: `figlet/` next to the executable.
pub fn external_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.parent()?.join("figlet"))
}

/// `.flf` files found in the external directory, sorted by name.
pub fn external_fonts() -> Vec<(String, PathBuf)> {
    let Some(dir) = external_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut found: Vec<(String, PathBuf)> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "flf"))
        .filter_map(|p| {
            let stem = p.file_stem()?.to_string_lossy().into_owned();
            Some((stem, p))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

pub fn load_file(path: &Path) -> Result<FIGlet> {
    // Deliberately not `read_to_string` + `from_content`: `.flf` files in the
    // wild are frequently CP437 or Latin-1, which is not valid UTF-8. This
    // loader detects the encoding for us.
    let path_str = path
        .to_str()
        .with_context(|| format!("font path is not valid UTF-8: {}", path.display()))?;
    FIGlet::from_file(path_str)
        .map_err(|e| anyhow::anyhow!("failed to parse font file {}: {e}", path.display()))
}

/// Resolve a font by name: built-in first, then the external directory.
pub fn load(name: &str) -> Result<FIGlet> {
    if is_builtin(name) {
        return load_builtin(name);
    }
    // Case-insensitive on the external set too, matching the built-in behaviour.
    if let Some((_, path)) = external_fonts()
        .into_iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
    {
        return load_file(&path);
    }
    anyhow::bail!(
        "unknown font {name:?}. Run `wart --list-fonts` to see the built-in set, \
         or drop a .flf into {}",
        external_dir()
            .map(|d| d.display().to_string())
            .unwrap_or_else(|| "<exe dir>/figlet".into())
    )
}

/// All selectable font names: built-ins plus external files.
pub fn all_names() -> Vec<String> {
    let mut names: Vec<String> = builtin_names().map(str::to_owned).collect();
    for (name, _) in external_fonts() {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_table_is_populated_and_sorted() {
        let names: Vec<_> = builtin_names().collect();
        assert!(!names.is_empty(), "build.rs found no .flf files");
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn every_builtin_font_parses() {
        for name in builtin_names() {
            load_builtin(name)
                .unwrap_or_else(|e| panic!("built-in font {name:?} failed to parse: {e}"));
        }
    }

    #[test]
    fn unknown_font_is_an_error() {
        assert!(load("definitely-not-a-real-font-xyz").is_err());
    }
}
