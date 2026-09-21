//! Headless mode: render and export without opening a window, so the tool can
//! be scripted from an editor or shell config.

use anyhow::{Context, Result};
use clap::Parser;
use std::io::Write;
use std::path::PathBuf;

use crate::color::ColorStyle;
use crate::export::{self, Format};
use crate::fonts;
use crate::model::Rgb;
use crate::render::{self, charset::Charset, figlet::IconStyle, frame::FrameKind, Mode};

#[derive(Parser, Debug)]
#[command(
    name = "wart",
    version,
    about = "Turn text into ASCII art with Nerd Font symbols and gradient colors",
    long_about = None,
)]
pub struct Args {
    /// Text to render. Omit to launch the GUI.
    #[arg(short, long)]
    pub text: Option<String>,

    /// FIGlet font name. Case-insensitive, so `standard` finds `Standard`.
    #[arg(short, long, default_value = "Standard")]
    pub font: String,

    /// Generation mode.
    #[arg(short, long, value_enum, default_value_t = ModeArg::Figlet)]
    pub mode: ModeArg,

    /// Block mode character set.
    #[arg(long, value_enum, default_value_t = CharsetArg::Braille)]
    pub charset: CharsetArg,

    /// Block mode target width, in character cells.
    #[arg(long, default_value_t = 100)]
    pub cols: usize,

    /// Block mode coverage cutoff for dot charsets, 0.0 to 1.0. Lower is denser.
    #[arg(long, default_value_t = 0.5)]
    pub threshold: f32,

    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Ansi)]
    pub format: Format,

    /// Write to this path instead of stdout.
    #[arg(short, long, value_name = "PATH")]
    pub out: Option<PathBuf>,

    /// Disable color entirely.
    #[arg(long)]
    pub no_color: bool,

    /// Solid color, e.g. `#ff8800`.
    #[arg(long, value_name = "HEX", conflicts_with_all = ["gradient", "radial", "rainbow"])]
    pub solid: Option<String>,

    /// Linear gradient stops, comma separated: `#ff0000,#0000ff`.
    #[arg(long, value_name = "HEX,..", conflicts_with_all = ["solid", "radial", "rainbow"])]
    pub gradient: Option<String>,

    /// Radial gradient stops, comma separated.
    #[arg(long, value_name = "HEX,..", conflicts_with_all = ["solid", "gradient", "rainbow"])]
    pub radial: Option<String>,

    /// Rainbow hue sweep.
    #[arg(long, conflicts_with_all = ["solid", "gradient", "radial"])]
    pub rainbow: bool,

    /// Gradient angle in degrees: 0 runs left to right, 90 top to bottom.
    #[arg(long, default_value_t = 0.0)]
    pub angle: f32,

    /// Background color for the image and document formats, e.g. `#101014`.
    #[arg(long, value_name = "HEX")]
    pub background: Option<String>,

    /// Draw a decorative border. `powerline` uses Nerd Font caps.
    #[arg(long, value_enum, default_value_t = FrameArg::None)]
    pub frame: FrameArg,

    /// How FIGlet mode draws Nerd Font icons and other characters its font
    /// has no glyph for.
    #[arg(long, value_enum, default_value_t = IconStyleArg::Shape)]
    pub icon_style: IconStyleArg,

    /// Blank cells between the art and the border.
    #[arg(long, default_value_t = 1, value_name = "N")]
    pub frame_padding: usize,

    /// Emit 256-color escapes instead of 24-bit truecolor, for terminals that
    /// cannot do truecolor.
    #[arg(long)]
    pub xterm256: bool,

    /// List available fonts and exit.
    #[arg(long)]
    pub list_fonts: bool,
}

/// `clap::ValueEnum` for [`render::Mode`], kept here so the render module does
/// not have to depend on the argument parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ModeArg {
    Figlet,
    Block,
}

impl From<ModeArg> for Mode {
    fn from(m: ModeArg) -> Self {
        match m {
            ModeArg::Figlet => Mode::Figlet,
            ModeArg::Block => Mode::Block,
        }
    }
}

/// `clap::ValueEnum` mirror of [`charset::Charset`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CharsetArg {
    Ascii,
    Blocks,
    Braille,
    Halfblock,
    /// Quarter blocks: a 2x2 grid of sub-cells per character.
    Quadrant,
    /// Shape-matched printable ASCII.
    Shape,
    /// Edges as `/ \ | _ -`, matching how a FIGlet banner is drawn.
    Lineart,
}

impl From<CharsetArg> for Charset {
    fn from(c: CharsetArg) -> Self {
        match c {
            CharsetArg::Ascii => Charset::AsciiRamp,
            CharsetArg::Blocks => Charset::Blocks,
            CharsetArg::Braille => Charset::Braille,
            CharsetArg::Halfblock => Charset::HalfBlock,
            CharsetArg::Shape => Charset::Shape,
            CharsetArg::Quadrant => Charset::Quadrant,
            CharsetArg::Lineart => Charset::LineArt,
        }
    }
}

/// `clap::ValueEnum` mirror of [`frame::FrameKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum FrameArg {
    None,
    Light,
    Rounded,
    Heavy,
    Double,
    Powerline,
}

impl From<FrameArg> for FrameKind {
    fn from(f: FrameArg) -> Self {
        match f {
            FrameArg::None => FrameKind::None,
            FrameArg::Light => FrameKind::Light,
            FrameArg::Rounded => FrameKind::Rounded,
            FrameArg::Heavy => FrameKind::Heavy,
            FrameArg::Double => FrameKind::Double,
            FrameArg::Powerline => FrameKind::Powerline,
        }
    }
}

/// `clap::ValueEnum` mirror of [`figlet::IconStyle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum IconStyleArg {
    /// Printable ASCII picked by shape: ASCII-art texture, still legible.
    Shape,
    /// Dots: the crispest icon, but a texture unlike the letters.
    Braille,
    /// Shade blocks: a solid silhouette in five steps of darkness.
    Blocks,
    /// Half blocks: two independently coloured pixels per cell. Needs a
    /// terminal that renders background colours.
    Halfblock,
    /// Quarter blocks: four sub-cells per character, no background colour
    /// needed.
    Quadrant,
    /// Strokes matching the banner's own characters, at the cost of legibility.
    Lineart,
}

impl From<IconStyleArg> for IconStyle {
    fn from(i: IconStyleArg) -> Self {
        match i {
            IconStyleArg::Shape => IconStyle::Shape,
            IconStyleArg::Braille => IconStyle::Braille,
            IconStyleArg::Blocks => IconStyle::Blocks,
            IconStyleArg::Halfblock => IconStyle::HalfBlock,
            IconStyleArg::Quadrant => IconStyle::Quadrant,
            IconStyleArg::Lineart => IconStyle::LineArt,
        }
    }
}

/// Parse a comma-separated list of hex colors.
fn parse_colors(s: &str) -> Result<Vec<Rgb>> {
    let colors: Result<Vec<Rgb>> = s
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| Rgb::parse(p).with_context(|| format!("in --gradient/--radial value {s:?}")))
        .collect();
    let colors = colors?;
    anyhow::ensure!(!colors.is_empty(), "no colors given in {s:?}");
    Ok(colors)
}

impl Args {
    /// Resolve the color flags into a single style. Defaults to white, which is
    /// what an uncolored terminal would show anyway.
    fn color_style(&self) -> Result<ColorStyle> {
        if let Some(hex) = &self.solid {
            return Ok(ColorStyle::Solid(Rgb::parse(hex)?));
        }
        if let Some(list) = &self.gradient {
            let colors = parse_colors(list)?;
            return Ok(ColorStyle::linear_multi(&colors, self.angle));
        }
        if let Some(list) = &self.radial {
            let colors = parse_colors(list)?;
            return Ok(ColorStyle::radial(&colors));
        }
        if self.rainbow {
            return Ok(ColorStyle::rainbow(self.angle));
        }
        Ok(ColorStyle::default())
    }
}

fn print_fonts() {
    let builtin: Vec<_> = fonts::builtin_names().collect();
    println!("built-in ({}):", builtin.len());
    for chunk in builtin.chunks(4) {
        println!("  {}", chunk.join("  "));
    }

    let external = fonts::external_fonts();
    if external.is_empty() {
        if let Some(dir) = fonts::external_dir() {
            println!("\nexternal: none — drop .flf files into {}", dir.display());
        }
    } else {
        println!("\nexternal ({}):", external.len());
        for (name, _) in external {
            println!("  {name}");
        }
    }
}

pub fn run() -> Result<()> {
    let args = Args::parse();

    if args.list_fonts {
        print_fonts();
        return Ok(());
    }

    let Some(text) = args.text.as_deref() else {
        // No text means the user wants the interactive app.
        return crate::app::launch();
    };

    let style = args.color_style()?;
    let art = render::render(&render::Request {
        text,
        mode: args.mode.into(),
        font_name: &args.font,
        charset: args.charset.into(),
        cols: args.cols,
        threshold: args.threshold,
        style: &style,
        frame: render::frame::Options {
            kind: args.frame.into(),
            padding: args.frame_padding,
        },
        icon_style: args.icon_style.into(),
    })?;

    let opts = export::Options {
        color: !args.no_color,
        xterm256: args.xterm256,
        // The ANSI export is what the config points at, so it is named after
        // whichever output path was asked for.
        logo_name: args
            .out
            .as_ref()
            .and_then(|p| p.file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "logo".to_owned()),
        // fastfetch resolves a relative source against its working directory,
        // so a config written to a file has to name the logo in full. A config
        // piped to stdout is paired with a logo written to the working
        // directory, where a bare filename is right.
        logo_source: args
            .out
            .as_ref()
            .and_then(|p| p.file_stem().map(|stem| (p, stem.to_string_lossy().into_owned())))
            .and_then(|(p, stem)| {
                let name = export::logo_file_name(&stem);
                std::path::absolute(p).ok().map(|abs| abs.with_file_name(name))
            })
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| export::logo_file_name("logo")),
        background: args
            .background
            .as_deref()
            .map(Rgb::parse)
            .transpose()?,
    };
    let bytes = export::export(&art, args.format, &opts)?;

    match &args.out {
        Some(path) => {
            std::fs::write(path, &bytes)
                .with_context(|| format!("failed to write {}", path.display()))?;
            eprintln!("wrote {} ({} bytes)", path.display(), bytes.len());

            // A fastfetch config is useless without the logo it points at, so
            // the logo comes with it rather than needing a second export.
            for (companion, data) in export::companions(&art, args.format, &opts, path) {
                std::fs::write(&companion, &data).with_context(|| {
                    format!("failed to write {}", companion.display())
                })?;
                eprintln!("wrote {} ({} bytes)", companion.display(), data.len());
            }
        }
        None => {
            // Writing raw bytes, not a `String` through a `LineWriter`, keeps
            // the UTF-8 art and the PNG path on the same code path.
            std::io::stdout()
                .write_all(&bytes)
                .context("failed to write to stdout")?;

            // With the config piped to a file, its `source` is resolved next to
            // wherever that lands, so the logo belongs in the working directory.
            let here = std::path::Path::new("");
            for (companion, data) in export::companions(&art, args.format, &opts, here) {
                std::fs::write(&companion, &data).with_context(|| {
                    format!("failed to write {}", companion.display())
                })?;
                eprintln!("wrote {} ({} bytes)", companion.display(), data.len());
            }
        }
    }

    Ok(())
}
