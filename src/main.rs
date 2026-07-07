mod tui;

use clap::{Parser, ValueEnum};
use lazarobox_ascii::converter::{Glyph, Options, Preset, convert, convert_canvas};
use lazarobox_ascii::{Direction, Effect, Rgb, Style, export, to_ansi_gradient};
use std::fs;

/// Conversor de imágenes a arte de texto (ASCII / Braille / bloques).
#[derive(Parser)]
#[command(name = "lazarobox-ascii", version, about)]
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

    /// Color de acento único (atajo). Para multi-tono usá `--colors`.
    #[arg(long, value_enum)]
    color: Option<ColorArg>,

    /// Lista de colores del gradiente, separados por coma. Nombres (cyan, purple, ...)
    /// o hex (`#B99BF2`). Ej: `--colors cyan,#7FB4CA,purple`.
    #[arg(long, value_delimiter = ',')]
    colors: Vec<String>,

    /// Dirección del gradiente.
    #[arg(long, value_enum)]
    gradient: Option<GradientArg>,

    /// Efecto animado (se materializa en la TUI y en el renderer de referencia; el
    /// `.ans` se hornea en t=0).
    #[arg(long, value_enum)]
    effect: Option<EffectArg>,

    /// Velocidad de la animación (ciclos/seg, aprox).
    #[arg(long, default_value_t = 1.0)]
    speed: f32,

    /// Formato de salida. Si se omite, se infiere de la extensión de `--out`.
    #[arg(long, short, value_enum)]
    format: Option<FormatArg>,

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
    fn rgb(self) -> Rgb {
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

/// Parsea un color por nombre o hex (`#rrggbb` / `rrggbb`).
fn parse_color(s: &str) -> Result<Rgb, String> {
    let s = s.trim();
    let hex = s.strip_prefix('#').unwrap_or(s);
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        let n = |a: usize, b: usize| u8::from_str_radix(&hex[a..b], 16).unwrap();
        return Ok((n(0, 2), n(2, 4), n(4, 6)));
    }
    match s.to_ascii_lowercase().as_str() {
        "cyan" | "cian" => Ok((0, 229, 255)),
        "green" | "verde" => Ok((0, 255, 128)),
        "magenta" => Ok((255, 0, 200)),
        "yellow" | "amarillo" => Ok((255, 214, 0)),
        "orange" | "naranja" => Ok((255, 120, 0)),
        "white" | "blanco" => Ok((235, 235, 235)),
        "blue" | "azul" => Ok((127, 180, 202)),
        "purple" | "violeta" | "morado" => Ok((185, 155, 242)),
        "red" | "rojo" => Ok((203, 124, 148)),
        other => Err(format!("color desconocido: '{other}' (usá un nombre o #rrggbb)")),
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum GradientArg {
    Horizontal,
    Vertical,
    Diagonal,
    Radial,
}

impl From<GradientArg> for Direction {
    fn from(g: GradientArg) -> Self {
        match g {
            GradientArg::Horizontal => Direction::Horizontal,
            GradientArg::Vertical => Direction::Vertical,
            GradientArg::Diagonal => Direction::Diagonal,
            GradientArg::Radial => Direction::Radial,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum EffectArg {
    None,
    Scroll,
    Pulse,
    Wave,
    Hue,
}

impl From<EffectArg> for Effect {
    fn from(e: EffectArg) -> Self {
        match e {
            EffectArg::None => Effect::None,
            EffectArg::Scroll => Effect::Scroll,
            EffectArg::Pulse => Effect::Pulse,
            EffectArg::Wave => Effect::Wave,
            EffectArg::Hue => Effect::Hue,
        }
    }
}

#[derive(Clone, Copy, PartialEq, ValueEnum)]
enum FormatArg {
    /// Glifos crudos monocromo.
    Txt,
    /// ANSI truecolor con el gradiente horneado (t=0).
    Ans,
    /// Datos + estilo agnósticos de lenguaje (schema lazarobox-ascii/v1).
    Json,
    /// Módulo Rust autocontenido con renderer de referencia.
    Rust,
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

/// Infiere el formato desde la extensión del archivo de salida.
fn format_from_ext(path: &str) -> Option<FormatArg> {
    let ext = path.rsplit('.').next()?.to_ascii_lowercase();
    match ext.as_str() {
        "txt" => Some(FormatArg::Txt),
        "ans" => Some(FormatArg::Ans),
        "json" => Some(FormatArg::Json),
        "rs" => Some(FormatArg::Rust),
        _ => None,
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

    // ── Estilo de color desde los flags ────────────────────────────────────────
    let stops: Vec<Rgb> = if !cli.colors.is_empty() {
        cli.colors
            .iter()
            .map(|s| parse_color(s))
            .collect::<Result<_, _>>()?
    } else if let Some(c) = cli.color {
        vec![c.rgb()]
    } else {
        vec![(0, 229, 255)]
    };
    let effect = cli.effect.map(Effect::from).unwrap_or(Effect::None);
    let style = Style {
        colors: stops,
        direction: cli.gradient.map(Direction::from).unwrap_or(Direction::Horizontal),
        effect,
        speed: cli.speed,
    };

    // ¿El usuario pidió color? (colores, gradiente, efecto o un formato con color)
    let asked_color = !cli.colors.is_empty()
        || cli.color.is_some()
        || cli.gradient.is_some()
        || effect != Effect::None;

    // ── Resolución de formato: explícito > extensión de --out > por defecto ─────
    let format = cli
        .format
        .or_else(|| cli.out.as_deref().and_then(format_from_ext))
        .unwrap_or(if asked_color {
            FormatArg::Ans
        } else {
            FormatArg::Txt
        });

    let img = image::open(&cli.input.expect("input presente en la rama CLI"))?;
    let canvas = convert_canvas(&img, &opts);

    let art = match format {
        FormatArg::Txt => convert(&img, &opts),
        FormatArg::Ans => to_ansi_gradient(&canvas, &style, 0.0),
        FormatArg::Json => export::to_json(&canvas, &style),
        FormatArg::Rust => export::to_rust(&canvas, &style),
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
