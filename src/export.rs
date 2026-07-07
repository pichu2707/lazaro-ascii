//! Serializadores embebibles: además del `.txt`/`.ans`, exporta la GRILLA + el ESTILO
//! como datos reutilizables para que cualquier herramienta o TUI los renderice (y anime)
//! en su propio loop.
//!
//! - [`to_json`]: formato agnóstico de lenguaje (schema `lazarobox-ascii/v1`).
//! - [`to_rust`]: módulo Rust autocontenido (cero dependencias) con la data + un
//!   `color_at(luma, x, y, t)` de referencia idéntico al motor de la herramienta.

use crate::{Style, converter::Canvas};

/// Escapa un carácter para incrustarlo en una cadena JSON.
fn json_char(ch: char) -> String {
    match ch {
        '"' => "\\\"".to_string(),
        '\\' => "\\\\".to_string(),
        '\n' => "\\n".to_string(),
        '\t' => "\\t".to_string(),
        '\r' => "\\r".to_string(),
        c if (c as u32) < 0x20 => format!("\\u{:04x}", c as u32),
        c => c.to_string(),
    }
}

fn colors_json(style: &Style) -> String {
    let stops: Vec<String> = style
        .colors
        .iter()
        .map(|(r, g, b)| format!("[{r},{g},{b}]"))
        .collect();
    format!("[{}]", stops.join(","))
}

/// Serializa un [`Canvas`] + [`Style`] a JSON (`lazarobox-ascii/v1`).
///
/// `grid` es row-major de longitud `width * height`; cada celda es `{"c":<char>,"l":<luma>}`.
/// Las celdas vacías van con `"l":0`, así el consumidor recibe la grilla rectangular exacta.
pub fn to_json(canvas: &Canvas, style: &Style) -> String {
    let mut grid = String::new();
    for (i, cell) in canvas.cells.iter().enumerate() {
        if i > 0 {
            grid.push(',');
        }
        grid.push_str(&format!(
            "{{\"c\":\"{}\",\"l\":{}}}",
            json_char(cell.ch),
            cell.luma
        ));
    }
    format!(
        concat!(
            "{{\n",
            "  \"format\": \"lazarobox-ascii/v1\",\n",
            "  \"width\": {w},\n",
            "  \"height\": {h},\n",
            "  \"style\": {{\n",
            "    \"colors\": {colors},\n",
            "    \"direction\": \"{dir}\",\n",
            "    \"effect\": \"{eff}\",\n",
            "    \"speed\": {speed}\n",
            "  }},\n",
            "  \"grid\": [{grid}]\n",
            "}}\n"
        ),
        w = canvas.width,
        h = canvas.height,
        colors = colors_json(style),
        dir = style.direction.as_str(),
        eff = style.effect.as_str(),
        speed = style.speed,
        grid = grid,
    )
}

/// Motor de referencia inlineado en el módulo Rust generado. Es un espejo EXACTO de
/// `lib::color_at` para que el arte exportado se vea/anime igual sin depender del crate.
const RUST_ENGINE: &str = r##"
/// Reference color engine — mirrors lazarobox-ascii. Zero dependencies.
fn lerp8(a: u8, b: u8, f: f32) -> u8 { (a as f32 + (b as f32 - a as f32) * f).round().clamp(0.0, 255.0) as u8 }

fn sample(stops: &[(u8, u8, u8)], t: f32) -> (u8, u8, u8) {
    match stops.len() {
        0 => (255, 255, 255),
        1 => stops[0],
        n => {
            let t = t.clamp(0.0, 1.0);
            let seg = t * (n - 1) as f32;
            let i = (seg.floor() as usize).min(n - 2);
            let f = seg - i as f32;
            let (a, b) = (stops[i], stops[i + 1]);
            (lerp8(a.0, b.0, f), lerp8(a.1, b.1, f), lerp8(a.2, b.2, f))
        }
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let h = h.rem_euclid(360.0) / 60.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0), 1 => (x, c, 0.0), 2 => (0.0, c, x),
        3 => (0.0, x, c), 4 => (x, 0.0, c), _ => (c, 0.0, x),
    };
    (((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8)
}

fn shade(r: u8, g: u8, b: u8, luma: u8) -> (u8, u8, u8) {
    let f = 0.30 + 0.70 * (luma as f32 / 255.0);
    ((r as f32 * f) as u8, (g as f32 * f) as u8, (b as f32 * f) as u8)
}

fn wrap01(v: f32) -> f32 { v.rem_euclid(1.0) }

fn position(x: usize, y: usize) -> f32 {
    let (xf, yf) = (x as f32, y as f32);
    let wf = (WIDTH.max(2) - 1) as f32;
    let hf = (HEIGHT.max(2) - 1) as f32;
    match DIRECTION {
        "vertical" => yf / hf,
        "diagonal" => (xf + yf) / (wf + hf),
        "radial" => {
            let (cx, cy) = (wf / 2.0, hf / 2.0);
            let (dx, dy) = (xf - cx, yf - cy);
            let max = ((cx * cx + cy * cy).sqrt()).max(1.0);
            ((dx * dx + dy * dy).sqrt() / max).min(1.0)
        }
        _ => xf / wf, // horizontal
    }
}

/// Color RGB de la celda (x,y) con su `luma` en el instante `t` (segundos).
pub fn color_at(luma: u8, x: usize, y: usize, t: f32) -> (u8, u8, u8) {
    use std::f32::consts::TAU;
    let mut p = position(x, y);
    let phase = t * SPEED;
    match EFFECT {
        "scroll" => p = wrap01(p + phase),
        "wave" => p = wrap01(p + 0.15 * (TAU * (p * 2.0 + phase)).sin()),
        _ => {}
    }
    let base = if EFFECT == "hue" {
        hsv_to_rgb(wrap01(p + phase) * 360.0, 0.9, 1.0)
    } else {
        sample(COLORS, p)
    };
    let (mut r, mut g, mut b) = shade(base.0, base.1, base.2, luma);
    if EFFECT == "pulse" {
        let f = 0.55 + 0.45 * (0.5 + 0.5 * (TAU * phase).sin());
        r = (r as f32 * f) as u8; g = (g as f32 * f) as u8; b = (b as f32 * f) as u8;
    }
    (r, g, b)
}
"##;

/// Escapa un carácter para un literal `char` de Rust (`'x'`).
fn rust_char(ch: char) -> String {
    match ch {
        '\'' => "\\'".to_string(),
        '\\' => "\\\\".to_string(),
        c => c.to_string(),
    }
}

/// Serializa un [`Canvas`] + [`Style`] a un módulo Rust autocontenido y sin dependencias.
///
/// Emite `WIDTH`/`HEIGHT`/`CELLS`/`COLORS`/`DIRECTION`/`EFFECT`/`SPEED` y un
/// `color_at(luma, x, y, t)` de referencia. Copiás el `.rs` a tu TUI y listo.
pub fn to_rust(canvas: &Canvas, style: &Style) -> String {
    let mut cells = String::new();
    for (i, cell) in canvas.cells.iter().enumerate() {
        if i % canvas.width as usize == 0 {
            cells.push_str("\n    ");
        }
        cells.push_str(&format!("('{}',{}),", rust_char(cell.ch), cell.luma));
    }

    let colors: Vec<String> = style
        .colors
        .iter()
        .map(|(r, g, b)| format!("({r},{g},{b})"))
        .collect();

    format!(
        concat!(
            "// Generated by lazarobox-ascii. Self-contained, zero-dependency ASCII art.\n",
            "// CELLS is row-major (WIDTH * HEIGHT) of (char, luma). Call `color_at(luma, x, y, t)`\n",
            "// from your render loop; advance `t` (seconds) to animate the effect.\n\n",
            "pub const WIDTH: usize = {w};\n",
            "pub const HEIGHT: usize = {h};\n",
            "pub const DIRECTION: &str = \"{dir}\";\n",
            "pub const EFFECT: &str = \"{eff}\";\n",
            "pub const SPEED: f32 = {speed:?};\n\n",
            "/// Gradient color stops.\n",
            "pub const COLORS: &[(u8, u8, u8)] = &[{colors}];\n\n",
            "/// (character, luma) per cell, row-major.\n",
            "pub const CELLS: &[(char, u8)] = &[{cells}\n];\n",
            "{engine}"
        ),
        w = canvas.width,
        h = canvas.height,
        dir = style.direction.as_str(),
        eff = style.effect.as_str(),
        speed = style.speed,
        colors = colors.join(", "),
        cells = cells,
        engine = RUST_ENGINE,
    )
}
