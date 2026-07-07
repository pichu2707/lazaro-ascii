pub mod converter;
pub mod export;

use converter::Canvas;
use std::f32::consts::TAU;

/// Piso de brillo: aun las celdas más oscuras conservan algo de tono (no se van a negro).
const SHADE_FLOOR: f32 = 0.30;

/// Un color RGB de 8 bits por canal.
pub type Rgb = (u8, u8, u8);

/// Escala un color de acento por un factor de brillo `0.0..=1.0`, con piso.
fn shade(r: u8, g: u8, b: u8, luma: u8) -> Rgb {
    let f = SHADE_FLOOR + (1.0 - SHADE_FLOOR) * (luma as f32 / 255.0);
    (
        (r as f32 * f) as u8,
        (g as f32 * f) as u8,
        (b as f32 * f) as u8,
    )
}

/// Envuelve el arte monocromo en un color de acento ANSI truecolor uniforme.
/// Para exportar `.ans` sin sombreado (un solo tono).
pub fn to_ansi(art: &str, (r, g, b): Rgb) -> String {
    format!("\x1b[38;2;{r};{g};{b}m{art}\x1b[0m")
}

/// Exporta un [`Canvas`] a ANSI truecolor **con sombreado por celda**: cada carácter
/// toma el tono de acento escalado por su luminancia. Es el look "cuadraditos con
/// sombra" — brillante en las luces, oscuro en las sombras.
pub fn to_ansi_shaded(canvas: &Canvas, (r, g, b): Rgb) -> String {
    let mut out = String::new();
    for y in 0..canvas.height {
        for x in 0..canvas.width {
            let c = canvas.cells[y as usize * canvas.width as usize + x as usize];
            if c.ch == ' ' || c.luma == 0 {
                out.push(' ');
                continue;
            }
            let (sr, sg, sb) = shade(r, g, b, c.luma);
            out.push_str(&format!("\x1b[38;2;{sr};{sg};{sb}m{}", c.ch));
        }
        out.push_str("\x1b[0m\n");
    }
    out
}

// ─────────────────────────── Motor de color multi-tono ───────────────────────────

/// Dirección en la que corre el gradiente a lo largo del arte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Izquierda → derecha.
    Horizontal,
    /// Arriba → abajo.
    Vertical,
    /// Esquina superior-izquierda → inferior-derecha.
    Diagonal,
    /// Centro → bordes.
    Radial,
}

impl Direction {
    /// Nombre estable para serializar (JSON/Rust).
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Horizontal => "horizontal",
            Direction::Vertical => "vertical",
            Direction::Diagonal => "diagonal",
            Direction::Radial => "radial",
        }
    }
}

/// Efecto animado que modula el gradiente en función del tiempo `t` (segundos).
/// Con `t = 0` todos degeneran en el gradiente estático (salvo el brillo medio de Pulse).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Sin animación: gradiente fijo.
    None,
    /// El gradiente se desplaza a lo largo de la dirección.
    Scroll,
    /// Latido de brillo global.
    Pulse,
    /// Onda que ondula la posición del gradiente.
    Wave,
    /// Ciclo de matiz (arcoíris) que ignora los `colors` y recorre la rueda HSV.
    Hue,
}

impl Effect {
    pub fn as_str(self) -> &'static str {
        match self {
            Effect::None => "none",
            Effect::Scroll => "scroll",
            Effect::Pulse => "pulse",
            Effect::Wave => "wave",
            Effect::Hue => "hue",
        }
    }
}

/// Estilo de color completo: los tonos, cómo se mapean y el efecto animado opcional.
#[derive(Clone, Debug)]
pub struct Style {
    /// Paradas de color (1 o más). Con 1 sola parada es un tono plano sombreado.
    pub colors: Vec<Rgb>,
    pub direction: Direction,
    pub effect: Effect,
    /// Velocidad de la animación (ciclos por segundo, aprox).
    pub speed: f32,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            colors: vec![(0, 229, 255)],
            direction: Direction::Horizontal,
            effect: Effect::None,
            speed: 1.0,
        }
    }
}

/// Interpola linealmente dos bytes.
fn lerp8(a: u8, b: u8, f: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * f).round().clamp(0.0, 255.0) as u8
}

/// Muestrea la rampa de paradas en `t ∈ [0,1]`.
fn sample(colors: &[Rgb], t: f32) -> Rgb {
    match colors.len() {
        0 => (255, 255, 255),
        1 => colors[0],
        n => {
            let t = t.clamp(0.0, 1.0);
            let seg = t * (n - 1) as f32;
            let i = (seg.floor() as usize).min(n - 2);
            let f = seg - i as f32;
            let (a, b) = (colors[i], colors[i + 1]);
            (lerp8(a.0, b.0, f), lerp8(a.1, b.1, f), lerp8(a.2, b.2, f))
        }
    }
}

/// HSV → RGB. `h` en grados 0-360, `s`/`v` en 0-1.
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Rgb {
    let h = h.rem_euclid(360.0) / 60.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// Envuelve un `f32` a `[0,1)`.
fn wrap01(v: f32) -> f32 {
    v.rem_euclid(1.0)
}

/// Posición normalizada `[0,1]` de la celda `(x,y)` según la dirección.
fn position(dir: Direction, x: u16, y: u16, w: u16, h: u16) -> f32 {
    let (xf, yf) = (x as f32, y as f32);
    let (wf, hf) = ((w.max(1) - 1).max(1) as f32, (h.max(1) - 1).max(1) as f32);
    match dir {
        Direction::Horizontal => xf / wf,
        Direction::Vertical => yf / hf,
        Direction::Diagonal => (xf + yf) / (wf + hf),
        Direction::Radial => {
            let (cx, cy) = (wf / 2.0, hf / 2.0);
            let (dx, dy) = (xf - cx, yf - cy);
            let max = (cx * cx + cy * cy).sqrt().max(1.0);
            ((dx * dx + dy * dy).sqrt() / max).min(1.0)
        }
    }
}

/// Color final de una celda: muestrea el gradiente en su posición, aplica el efecto
/// según `t` y lo sombrea por su `luma`. Es la ÚNICA fuente de verdad del color, así
/// el export estático (`t = 0`), la animación de la TUI y el renderer de referencia
/// producen exactamente el mismo resultado.
pub fn color_at(style: &Style, luma: u8, x: u16, y: u16, w: u16, h: u16, t: f32) -> Rgb {
    let mut p = position(style.direction, x, y, w, h);
    let phase = t * style.speed;

    match style.effect {
        Effect::Scroll => p = wrap01(p + phase),
        Effect::Wave => p = wrap01(p + 0.15 * (TAU * (p * 2.0 + phase)).sin()),
        _ => {}
    }

    let base = if style.effect == Effect::Hue {
        hsv_to_rgb(wrap01(p + phase) * 360.0, 0.9, 1.0)
    } else {
        sample(&style.colors, p)
    };

    let (mut r, mut g, mut b) = shade(base.0, base.1, base.2, luma);

    if style.effect == Effect::Pulse {
        let f = 0.55 + 0.45 * (0.5 + 0.5 * (TAU * phase).sin());
        r = (r as f32 * f) as u8;
        g = (g as f32 * f) as u8;
        b = (b as f32 * f) as u8;
    }
    (r, g, b)
}

/// Exporta un [`Canvas`] a ANSI truecolor con **gradiente multi-tono** en el instante
/// `t` (usá `t = 0.0` para el `.ans` estático). Cada celda toma su color de
/// [`color_at`].
pub fn to_ansi_gradient(canvas: &Canvas, style: &Style, t: f32) -> String {
    let (w, h) = (canvas.width, canvas.height);
    let mut out = String::new();
    for y in 0..h {
        for x in 0..w {
            let c = canvas.cells[y as usize * w as usize + x as usize];
            if c.ch == ' ' || c.luma == 0 {
                out.push(' ');
                continue;
            }
            let (r, g, b) = color_at(style, c.luma, x, y, w, h, t);
            out.push_str(&format!("\x1b[38;2;{r};{g};{b}m{}", c.ch));
        }
        out.push_str("\x1b[0m\n");
    }
    out
}
