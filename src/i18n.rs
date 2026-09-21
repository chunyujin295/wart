//! UI translations.
//!
//! Strings live in one struct per language rather than a key/value map, so a
//! missing translation is a compile error instead of a blank label at runtime.
//! Anything with a placeholder gets a function, because the argument order
//! differs between languages often enough that a `format!` template in a struct
//! field would eventually be wrong in one of them.

use crate::app::StyleKind;
use crate::export::Format;
use crate::render::{charset::Charset, figlet::IconStyle, frame::FrameKind, Mode};
use crate::ui::theme::Palette;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    En,
    Zh,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::En, Lang::Zh];

    /// Shown in the language switcher, always in the language's own script.
    pub fn label(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Zh => "中文",
        }
    }

    pub fn strings(self) -> &'static Strings {
        match self {
            Lang::En => &EN,
            Lang::Zh => &ZH,
        }
    }
}

pub struct Strings {
    // Chrome
    pub tagline: &'static str,

    // Text
    pub text: &'static str,
    pub text_hint: &'static str,
    pub text_note: &'static str,

    // Icon picker
    pub icons: &'static str,
    pub icons_search: &'static str,
    pub icons_all: &'static str,
    pub icons_none: &'static str,
    pub icons_figlet_hint: &'static str,

    // Mode and generation
    pub mode: &'static str,
    pub font: &'static str,
    pub font_note: &'static str,
    pub font_ascii_note: &'static str,
    pub font_search: &'static str,
    pub font_no_match: &'static str,
    pub charset: &'static str,
    pub width: &'static str,
    pub cells: &'static str,
    pub cutoff: &'static str,
    pub cutoff_note: &'static str,
    pub halfblock_note: &'static str,
    pub icon_style: &'static str,
    pub icon_style_note: &'static str,
    pub frame: &'static str,
    pub frame_note: &'static str,
    pub frame_padding: &'static str,

    // Color
    pub color: &'static str,
    pub from: &'static str,
    pub to: &'static str,
    pub angle: &'static str,
    pub deg: &'static str,
    pub angle_note: &'static str,

    // Export
    pub export: &'static str,
    pub save_as: &'static str,
    pub copy: &'static str,
    pub default_stem: &'static str,

    // Preview
    pub preview: &'static str,
    pub zoom: &'static str,
    pub background: &'static str,
    pub nothing_to_preview: &'static str,
}

impl Strings {
    pub fn mode(&self, m: Mode) -> &'static str {
        match (self.is_zh(), m) {
            (false, Mode::Figlet) => "FIGlet",
            (false, Mode::Block) => "Block",
            (true, Mode::Figlet) => "FIGlet",
            (true, Mode::Block) => "点阵",
        }
    }

    pub fn charset(&self, c: Charset) -> &'static str {
        match (self.is_zh(), c) {
            (false, Charset::AsciiRamp) => "ASCII ramp",
            (false, Charset::Blocks) => "Blocks",
            (false, Charset::Braille) => "Braille",
            (false, Charset::HalfBlock) => "Half block",
            (true, Charset::AsciiRamp) => "ASCII 渐变",
            (true, Charset::Blocks) => "方块",
            (true, Charset::Braille) => "盲文",
            (true, Charset::HalfBlock) => "半方块",
            (false, Charset::Shape) => "Shape matched",
            (false, Charset::Quadrant) => "Quarter blocks",
            (false, Charset::LineArt) => "Line art",
            (true, Charset::Shape) => "形状匹配",
            (true, Charset::Quadrant) => "四分之一方块",
            (true, Charset::LineArt) => "线画",
        }
    }

    pub fn style(&self, s: StyleKind) -> &'static str {
        match (self.is_zh(), s) {
            (false, StyleKind::Solid) => "Solid",
            (false, StyleKind::Linear) => "Linear",
            (false, StyleKind::Radial) => "Radial",
            (false, StyleKind::Rainbow) => "Rainbow",
            (true, StyleKind::Solid) => "纯色",
            (true, StyleKind::Linear) => "线性",
            (true, StyleKind::Radial) => "径向",
            (true, StyleKind::Rainbow) => "彩虹",
        }
    }

    pub fn format(&self, f: Format) -> &'static str {
        match (self.is_zh(), f) {
            (false, Format::Ansi) => "ANSI text",
            (false, Format::Plain) => "Plain text",
            (false, Format::Lua) => "Neovim Lua",
            (false, Format::Png) => "PNG",
            (false, Format::Fastfetch) => "fastfetch config",
            (false, Format::Html) => "HTML",
            (false, Format::Svg) => "SVG",
            (true, Format::Ansi) => "ANSI 文本",
            (true, Format::Plain) => "纯文本",
            (true, Format::Lua) => "Neovim Lua",
            (true, Format::Png) => "PNG 图片",
            (true, Format::Fastfetch) => "fastfetch 配置",
            (true, Format::Html) => "HTML 网页",
            (true, Format::Svg) => "SVG 矢量",
        }
    }

    pub fn palette(&self, p: Palette) -> &'static str {
        match (self.is_zh(), p) {
            (false, Palette::Dark) => "Dark",
            (false, Palette::Light) => "Light",
            (true, Palette::Dark) => "深色",
            (true, Palette::Light) => "浅色",
        }
    }

    pub fn icon_style(&self, i: IconStyle) -> &'static str {
        match (self.is_zh(), i) {
            (false, IconStyle::Shape) => "Shape matched",
            (false, IconStyle::Blocks) => "Blocks",
            (false, IconStyle::HalfBlock) => "Half block",
            (false, IconStyle::Quadrant) => "Quarter blocks",
            (false, IconStyle::LineArt) => "Line art",
            (false, IconStyle::Braille) => "Braille",
            (true, IconStyle::Shape) => "形状匹配",
            (true, IconStyle::Blocks) => "方块",
            (true, IconStyle::HalfBlock) => "半方块",
            (true, IconStyle::Quadrant) => "四分之一方块",
            (true, IconStyle::LineArt) => "线画",
            (true, IconStyle::Braille) => "盲文",
        }
    }

    pub fn frame_kind(&self, k: FrameKind) -> &'static str {
        match (self.is_zh(), k) {
            (false, FrameKind::None) => "None",
            (false, FrameKind::Light) => "Light",
            (false, FrameKind::Rounded) => "Rounded",
            (false, FrameKind::Heavy) => "Heavy",
            (false, FrameKind::Double) => "Double",
            (false, FrameKind::Powerline) => "Powerline",
            (true, FrameKind::None) => "无",
            (true, FrameKind::Light) => "细线",
            (true, FrameKind::Rounded) => "圆角",
            (true, FrameKind::Heavy) => "粗线",
            (true, FrameKind::Double) => "双线",
            (true, FrameKind::Powerline) => "Powerline",
        }
    }

    /// Identifies the Chinese table, so the label mappings above stay in one
    /// place instead of being duplicated per language.
    fn is_zh(&self) -> bool {
        std::ptr::eq(self, &ZH)
    }
}

// --- dynamic strings -------------------------------------------------------

impl Lang {
    pub fn icons_showing(&self, shown: usize, total: usize) -> String {
        match self {
            Lang::En => format!("{shown} of {total} \u{2014} scroll for more"),
            Lang::Zh => format!("已显示 {shown} / {total} 个 \u{2014} 滚动查看更多"),
        }
    }

    pub fn icons_category(&self, name: &str, count: usize) -> String {
        format!("{name} ({count})")
    }


    pub fn wrote_file(&self, path: &str, bytes: usize) -> String {
        match self {
            Lang::En => format!("Wrote {path} ({bytes} bytes)"),
            Lang::Zh => format!("已写入 {path}（{bytes} 字节）"),
        }
    }

    /// Confirmation for an export that wrote more than one file.
    pub fn wrote_files(&self, paths: &[String]) -> String {
        match self {
            Lang::En => format!("Wrote {}", paths.join(" and ")),
            Lang::Zh => format!("已写入 {}", paths.join(" 和 ")),
        }
    }

    pub fn copied(&self) -> String {
        match self {
            Lang::En => "Copied to clipboard".into(),
            Lang::Zh => "已复制到剪贴板".into(),
        }
    }

    pub fn copy_text_only(&self) -> String {
        match self {
            Lang::En => "Copy supports text formats only".into(),
            Lang::Zh => "仅文本格式支持复制".into(),
        }
    }

    pub fn clipboard_error(&self, e: &str) -> String {
        match self {
            Lang::En => format!("Clipboard error: {e}"),
            Lang::Zh => format!("剪贴板错误：{e}"),
        }
    }

    pub fn error(&self, e: &str) -> String {
        match self {
            Lang::En => format!("Error: {e}"),
            Lang::Zh => format!("错误：{e}"),
        }
    }

}

static EN: Strings = Strings {
    tagline: "Text to ASCII art",

    text: "Text",
    text_hint: "Type here",
    text_note: "Newlines stack as separate banners.",

    icons: "Nerd Font icons",
    icons_search: "search",
    icons_all: "all",
    icons_none: "No matching icons.",
    icons_figlet_hint: "Icons are drawn as art at the letters' height.",

    mode: "Mode",
    font: "Font",
    font_note: "fonts available",
    font_ascii_note: "Icons and other Unicode are drawn as art beside the letters.",
    font_search: "filter",
    font_no_match: "No matching fonts.",
    charset: "Charset",
    width: "Width",
    cells: " cells",
    cutoff: "Cutoff",
    cutoff_note: "Lower cutoff is denser.",
    halfblock_note: "Half block paints two colors per cell; needs a color-capable terminal.",
    icon_style: "Icon style",
    icon_style_note: "How icons and other non-ASCII characters are drawn.",
    frame: "Frame",
    frame_note: "The border takes part in the gradient.",
    frame_padding: "Padding",

    color: "Color",
    from: "From",
    to: "To",
    angle: "Angle",
    deg: " deg",
    angle_note: "0 = left to right, 90 = top to bottom.",

    export: "Export",
    save_as: "Save as...",
    copy: "Copy",
    default_stem: "logo",

    preview: "Preview",
    zoom: "Zoom",
    background: "Background",
    nothing_to_preview: "Nothing to preview.",
};

static ZH: Strings = Strings {
    tagline: "文本转 ASCII 艺术字",

    text: "文本",
    text_hint: "在此输入",
    text_note: "换行会堆叠成多个独立的横幅。",

    icons: "Nerd Font 图标",
    icons_search: "搜索",
    icons_all: "全部",
    icons_none: "没有匹配的图标。",
    icons_figlet_hint: "图标会按大字的行高绘制成点阵。",

    mode: "模式",
    font: "字体",
    font_note: "款字体可用",
    font_ascii_note: "图标和其他 Unicode 字符会绘制成点阵置于大字旁。",
    font_search: "筛选",
    font_no_match: "没有匹配的字体。",
    charset: "字符集",
    width: "宽度",
    cells: " 格",
    cutoff: "阈值",
    cutoff_note: "阈值越低，点阵越密。",
    halfblock_note: "半方块每格绘制两种颜色，需要终端支持背景色。",
    icon_style: "图标样式",
    icon_style_note: "图标及其他非 ASCII 字符的绘制方式。",
    frame: "边框",
    frame_note: "边框会跟随渐变一起着色。",
    frame_padding: "内边距",

    color: "配色",
    from: "起始",
    to: "结束",
    angle: "角度",
    deg: " 度",
    angle_note: "0 度为从左到右，90 度为从上到下。",

    export: "导出",
    save_as: "保存为…",
    copy: "复制",
    default_stem: "logo",

    preview: "预览",
    zoom: "缩放",
    background: "背景",
    nothing_to_preview: "暂无内容可预览。",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_a_label_in_its_own_script() {
        assert_eq!(Lang::En.label(), "English");
        assert_eq!(Lang::Zh.label(), "中文");
    }

    #[test]
    fn no_translation_is_left_blank() {
        for lang in Lang::ALL {
            let s = lang.strings();
            for (name, value) in [
                ("tagline", s.tagline),
                ("text", s.text),
                ("text_hint", s.text_hint),
                ("text_note", s.text_note),
                ("icons", s.icons),
                ("icons_search", s.icons_search),
                ("icons_all", s.icons_all),
                ("icons_none", s.icons_none),
                ("icons_figlet_hint", s.icons_figlet_hint),
                ("mode", s.mode),
                ("font", s.font),
                ("font_note", s.font_note),
                ("font_ascii_note", s.font_ascii_note),
                ("font_search", s.font_search),
                ("font_no_match", s.font_no_match),
                ("charset", s.charset),
                ("width", s.width),
                ("cutoff", s.cutoff),
                ("cutoff_note", s.cutoff_note),
                ("halfblock_note", s.halfblock_note),
                ("icon_style", s.icon_style),
                ("icon_style_note", s.icon_style_note),
                ("frame", s.frame),
                ("frame_note", s.frame_note),
                ("frame_padding", s.frame_padding),
                ("color", s.color),
                ("from", s.from),
                ("to", s.to),
                ("angle", s.angle),
                ("angle_note", s.angle_note),
                ("export", s.export),
                ("save_as", s.save_as),
                ("copy", s.copy),
                ("preview", s.preview),
                ("zoom", s.zoom),
                ("background", s.background),
                ("nothing_to_preview", s.nothing_to_preview),
            ] {
                assert!(!value.trim().is_empty(), "{lang:?}.{name} is blank");
            }
        }
    }

    #[test]
    fn the_chinese_table_is_actually_chinese() {
        // Guards against a copy-paste that leaves English in the ZH table.
        let zh = Lang::Zh.strings();
        let has_han = |s: &str| s.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
        for (name, value) in [
            ("tagline", zh.tagline),
            ("text", zh.text),
            ("mode", zh.mode),
            ("preview", zh.preview),
        ] {
            assert!(has_han(value), "ZH.{name} = {value:?} has no Chinese characters");
        }
    }

    #[test]
    fn the_language_tables_are_distinct() {
        assert!(!std::ptr::eq(Lang::En.strings(), Lang::Zh.strings()));
        assert!(Lang::En.strings().is_zh() == false);
        assert!(Lang::Zh.strings().is_zh());
    }

    #[test]
    fn enum_labels_differ_between_languages() {
        assert_eq!(Lang::En.strings().charset(Charset::Braille), "Braille");
        assert_eq!(Lang::Zh.strings().charset(Charset::Braille), "盲文");
        assert_eq!(Lang::En.strings().style(StyleKind::Rainbow), "Rainbow");
        assert_eq!(Lang::Zh.strings().style(StyleKind::Rainbow), "彩虹");
    }

    #[test]
    fn count_strings_report_both_numbers() {
        assert!(Lang::En.icons_showing(50, 10995).contains("50 of 10995"));
        assert!(Lang::Zh.icons_showing(50, 10995).contains("50 / 10995"));
    }
}
