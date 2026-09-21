//! Font registration for the egui context.
//!
//! Two families, two jobs:
//!
//! * **Proportional** — the interface. Leads with the bundled pixel font, which
//!   covers Latin and Simplified Chinese in one face, so labels no longer depend
//!   on what the operating system happens to have installed.
//! * **Monospace** — the artwork and the icon picker. Leads with the Nerd Font,
//!   because private-use icon codepoints and a uniform advance width are the
//!   whole point there.

use eframe::egui;
use std::sync::Arc;

/// Where a CJK face is looked for if the bundled pixel font somehow lacks a
/// glyph. It covers Han, so this is a backstop rather than the usual path.
const CJK_CANDIDATES: &[&str] = &[
    r"C:\Windows\Fonts\Deng.ttf",   // DengXian, the modern default
    r"C:\Windows\Fonts\simhei.ttf", // SimHei, smaller and always present
    r"C:\Windows\Fonts\simsun.ttc", // SimSun, collection
    r"C:\Windows\Fonts\msyh.ttc",   // Microsoft YaHei, collection
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
];

/// Index of the face to use inside a `.ttc` collection.
const COLLECTION_FACE: u32 = 0;

/// Whether a system CJK font is available as a fallback.
pub fn cjk_available() -> bool {
    CJK_CANDIDATES.iter().any(|p| std::path::Path::new(p).is_file())
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    // `from_static` borrows the embedded bytes, so neither font is copied.
    fonts.font_data.insert(
        "pixel".to_owned(),
        Arc::new(egui::FontData::from_static(crate::assets::PIXEL_FONT_OTF)),
    );
    fonts.font_data.insert(
        "nerd".to_owned(),
        Arc::new(egui::FontData::from_static(crate::assets::NERD_FONT_TTF)),
    );

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "pixel".to_owned());

    // The Nerd Font leads the monospace family: that is what makes its icon
    // glyphs resolve and every cell keep the same advance width.
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "nerd".to_owned());
    // The pixel font covers CJK, which the Nerd Font does not.
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .push("pixel".to_owned());

    for path in CJK_CANDIDATES {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let mut data = egui::FontData::from_owned(bytes);
        // `.ttc` files are collections; `index` picks the face inside one, and
        // is simply 0 for a plain `.ttf`.
        data.index = COLLECTION_FACE;
        fonts.font_data.insert("cjk".to_owned(), Arc::new(data));

        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.entry(family).or_default().push("cjk".to_owned());
        }
        break;
    }

    // Last in both families: the interface font covers Latin and Han, but not
    // the Nerd Font's private-use icons.
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .push("nerd".to_owned());

    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_bundled_fonts_are_present_and_parseable() {
        // skrifa handles both outlines; a CFF/OTF face is not a problem here,
        // which is worth pinning down because it once would have been.
        assert!(crate::assets::PIXEL_FONT_OTF.len() > 1024);
        assert!(crate::assets::NERD_FONT_TTF.len() > 1024);
        assert_eq!(&crate::assets::PIXEL_FONT_OTF[..4], b"OTTO");
        assert_eq!(&crate::assets::NERD_FONT_TTF[..4], &[0x00, 0x01, 0x00, 0x00]);
    }

    fn glyphs_of(bytes: &[u8], chars: &[char]) -> Vec<Option<u16>> {
        // fontdue is already a dependency and parses both `glyf` and CFF
        // outlines, so it is the cheap way to ask a font what it contains.
        let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
            .expect("font should parse");
        chars
            .iter()
            .map(|c| {
                let i = font.lookup_glyph_index(*c);
                (i != 0).then_some(i)
            })
            .collect()
    }

    #[test]
    fn the_interface_font_covers_latin_and_han() {
        for ch in ['A', 'z', '0', '中', '文', '导', '出'] {
            assert!(
                glyphs_of(crate::assets::PIXEL_FONT_OTF, &[ch])[0].is_some(),
                "the interface font has no glyph for {ch:?}"
            );
        }
    }

    #[test]
    fn the_interface_font_is_a_pixel_font_of_whole_pixels() {
        // Its design size is 12px: 1200 units per em, 100 units per pixel. That
        // is why the theme only ever asks for multiples of 12.
        let font = fontdue::Font::from_bytes(
            crate::assets::PIXEL_FONT_OTF,
            fontdue::FontSettings::default(),
        )
        .expect("parse");
        let advance = font.metrics('M', 12.0).advance_width;
        assert!(
            (advance - advance.round()).abs() < 0.05,
            "a pixel font should advance by whole pixels at its design size, got {advance}"
        );
    }
}
