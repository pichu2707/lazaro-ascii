pub mod converter;

use converter::Canvas;

/// Piso de brillo: aun las celdas más oscuras conservan algo de tono (no se van a negro).
const SHADE_FLOOR: f32 = 0.30;

/// Escala un color de acento por un factor de brillo `0.0..=1.0`, con piso.
fn shade(r: u8, g: u8, b: u8, luma: u8) -> (u8, u8, u8) {
    let f = SHADE_FLOOR + (1.0 - SHADE_FLOOR) * (luma as f32 / 255.0);
    (
        (r as f32 * f) as u8,
        (g as f32 * f) as u8,
        (b as f32 * f) as u8,
    )
}

/// Envuelve el arte monocromo en un color de acento ANSI truecolor uniforme.
/// Para exportar `.ans` sin sombreado (un solo tono).
pub fn to_ansi(art: &str, (r, g, b): (u8, u8, u8)) -> String {
    format!("\x1b[38;2;{r};{g};{b}m{art}\x1b[0m")
}

/// Exporta un [`Canvas`] a ANSI truecolor **con sombreado por celda**: cada carácter
/// toma el tono de acento escalado por su luminancia. Es el look "cuadraditos con
/// sombra" — brillante en las luces, oscuro en las sombras.
pub fn to_ansi_shaded(canvas: &Canvas, (r, g, b): (u8, u8, u8)) -> String {
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
