# Bundled FIGlet fonts

These 328 `.flf` fonts are vendored from
[patorjk/figlet.js](https://github.com/patorjk/figlet.js) — the engine and font
set behind [patorjk's TAAG](https://patorjk.com/software/taag/), which is what
wart's font list is meant to match.

They are unmodified apart from one transcoding step described below. `build.rs`
scans this directory at compile time and embeds every `.flf` into the binary, so
adding or removing a file here changes the built-in font list.

## Licensing

`figlet.js` is **MIT** (`../licenses/figlet-js-MIT.txt`), and it has
redistributed this font collection under that licence for over a decade.

Be aware that the individual font files are third-party contributions. Each one
carries its author's attribution in its comment block — for example
`Graffiti.flf` credits Leigh Purdie, `Standard.flf` credits Glenn Chappell and
Ian Chai — but most state no licence of their own.

The collection traces back to the FIGlet distribution, which Debian's copyright
file treats as **BSD-3-Clause** across the board (`Files: *`), with a separate
Unicode licence only for the `.flc` codepage files that are not shipped here.
The canonical BSD-3 set from `cmatsuoka/figlet` is preserved in
`../licenses/figlet-BSD3.txt`.

If you need an unambiguously-licensed subset, the 18 fonts in the
`cmatsuoka/figlet` repository are the ones with an explicit BSD-3 grant.

## Transcoding

FIGlet fonts are historically CP437-encoded. Each file here was decoded as UTF-8
and fell back to CP437 only when that failed, then re-encoded as UTF-8. This is
lossless, and it lets the fonts be embedded with `include_str!`, which requires
valid UTF-8.

## Fonts that need special handling

Three files deviate from what the FIGfont spec's happy path assumes. wart
normalises them at load time (`src/fonts.rs::normalize_figfont`) rather than
patching the files, so they stay byte-identical to upstream:

| Font | Issue |
|---|---|
| `Standard.flf` and most others | Declare a non-zero **codetag** count. The parser only accepts the trailing section when it is an exact multiple of `height + 1`, which these are not. |
| `Broadway KB.flf` | Declares **zero** codetags but still trips the same check, because the file's trailing newline is counted as codetag data. |
| `Pyramid.flf` | Declares a **non-ASCII hardblank** (CP437 `0x81`, which decodes to `ü`). The parser slices the header by byte and panics. Substituted with DEL, which is single-byte and never appears in glyph art. |
| `Font Font.flf` | Starts with a **UTF-8 BOM**, which shifts every byte offset on the header line. |
