mod tui;

use clap::{Parser, ValueEnum};
use lazarobox_ascii::converter::{Glyph, Options, Preset, convert, convert_canvas};
use lazarobox_ascii::to_ansi_shaded;
use std::fs;

/// Conversor de imágenes a arte de texto (ASCII / Braille / bloques).
#[derive(Parser)]
#[command(name = "lazarobox-ascii")]
struct Cli {
    /// Imagen de entrada (png, jpg, webp, ...). Si se omite, abre la TUI.
    input: Option<String>,

    /// Preset por caso de uso. Fija cols/glyph/threshold; los flags de abajo lo pisan.
    #[arg(long, value_enum)]
    preset: Option<PresetArg>,

    /// Ancho de salida en columnas. El alto se calcula solo.
    #[arg(long)]
    cols: Option<u16>,

    /// Estrategia de glifo.
    #[arg(long, value_enum)]
    glyph: Option<GlyphArg>,

    /// Umbral 0-255 para Braille/Blocks (fondo vacío por debajo).
    #[arg(long)]
    threshold: Option<u8>,

    /// Invierte claro/oscuro.
    #[arg(long)]
    invert: bool,

    /// Dithering (sombreado por densidad de puntos) en Braille/Blocks.
    #[arg(long)]
    dither: bool,

    /// Color de acento; si se indica, exporta ANSI con sombreado por celda.
    #[arg(long, value_enum)]
    color: Option<ColorArg>,

    /// Archivo de salida. Si se omite, imprime por stdout.
    #[arg(long, short)]
    out: Option<String>,

    /// Abre el editor interactivo (TUI) sobre la imagen.
    #[arg(long)]
    tui: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum ColorArg {
    Cyan,
    Green,
    Magenta,
    Yellow,
    Orange,
    White,
}

impl ColorArg {
    fn rgb(self) -> (u8, u8, u8) {
        match self {
            ColorArg::Cyan => (0, 229, 255),
            ColorArg::Green => (0, 255, 128),
            ColorArg::Magenta => (255, 0, 200),
            ColorArg::Yellow => (255, 214, 0),
            ColorArg::Orange => (255, 120, 0),
            ColorArg::White => (235, 235, 235),
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum GlyphArg {
    Ascii,
    Braille,
    Blocks,
}

impl From<GlyphArg> for Glyph {
    fn from(g: GlyphArg) -> Self {
        match g {
            GlyphArg::Ascii => Glyph::Ascii,
            GlyphArg::Braille => Glyph::Braille,
            GlyphArg::Blocks => Glyph::Blocks,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum PresetArg {
    Icon,
    Avatar,
    Readme,
    Banner,
    Logo,
}

impl From<PresetArg> for Preset {
    fn from(p: PresetArg) -> Self {
        match p {
            PresetArg::Icon => Preset::Icon,
            PresetArg::Avatar => Preset::Avatar,
            PresetArg::Readme => Preset::Readme,
            PresetArg::Banner => Preset::Banner,
            PresetArg::Logo => Preset::Logo,
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Base: el preset (si hay) o valores globales por defecto.
    let (base_cols, base_glyph, base_threshold) = match cli.preset {
        Some(p) => Preset::from(p).defaults(),
        None => (80, Glyph::Braille, 128),
    };

    // Los flags explícitos pisan al preset.
    let opts = Options {
        cols: cli.cols.unwrap_or(base_cols),
        glyph: cli.glyph.map(Glyph::from).unwrap_or(base_glyph),
        threshold: cli.threshold.unwrap_or(base_threshold),
        invert: cli.invert,
        dither: cli.dither,
    };

    // TUI: con --tui, o al invocar sin imagen (arranca en la bienvenida).
    if cli.tui || cli.input.is_none() {
        return tui::run(cli.input.as_deref(), opts);
    }

    let input = cli.input.expect("input presente en la rama CLI");
    let img = image::open(&input)?;
    // Con --color: ANSI sombreado por celda. Sin color: texto monocromo plano.
    let art = match cli.color {
        Some(col) => to_ansi_shaded(&convert_canvas(&img, &opts), col.rgb()),
        None => convert(&img, &opts),
    };

    match cli.out {
        Some(path) => {
            fs::write(&path, &art)?;
            eprintln!("Escrito en {path}");
        }
        None => print!("{art}"),
    }
    Ok(())
}
