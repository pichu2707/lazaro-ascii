use image::{DynamicImage, RgbaImage, imageops::FilterType};

/// Relación de aspecto de una celda de terminal (ancho / alto).
/// Un carácter es ~2x más alto que ancho, por eso 0.5.
const CELL_ASPECT: f32 = 0.5;

/// Alfa mínimo para considerar un píxel "presente". Por debajo = fondo (vacío).
const ALPHA_MIN: u8 = 128;

/// Rampa de densidad para modo ASCII (oscuro -> claro).
const RAMP: &[u8] = b" .:-=+*#%@";

/// Tabla de bloques de cuadrante (2x2). Índice por bits: TL=1, TR=2, BL=4, BR=8.
const QUAD: [char; 16] = [
    ' ', '▘', '▝', '▀', '▖', '▌', '▞', '▛', '▗', '▚', '▐', '▜', '▄', '▙', '▟', '█',
];

/// Estrategia de glifo: cómo un bloque de píxeles se vuelve carácter.
#[derive(Clone, Copy, Debug)]
pub enum Glyph {
    /// Rampa de densidad, 1 píxel por celda. Portable, ideal README.
    Ascii,
    /// Braille 2x4 subpíxeles por celda. Máximo detalle, look "puntitos".
    Braille,
    /// Bloques de cuadrante 2x2. Look sólido tipo stencil.
    Blocks,
}

/// Preset por caso de uso: fija cols + glifo + umbral ya calibrados.
/// Son valores de arranque; cualquier flag explícito los pisa.
#[derive(Clone, Copy, Debug)]
pub enum Preset {
    /// Icono chico, alto detalle. ~24 cols, Braille.
    Icon,
    /// Avatar / thumbnail. ~40 cols, Braille.
    Avatar,
    /// Cabecera de README, portable copia-pega. ~80 cols, ASCII.
    Readme,
    /// Banner de GitHub / splash. ~120 cols, bloques.
    Banner,
    /// Logo tipo stencil. ~60 cols, Braille, umbral exigente.
    Logo,
}

impl Preset {
    /// Devuelve (cols, glyph, threshold) calibrados para el caso de uso.
    pub fn defaults(self) -> (u16, Glyph, u8) {
        match self {
            Preset::Icon => (24, Glyph::Braille, 110),
            Preset::Avatar => (40, Glyph::Braille, 110),
            Preset::Readme => (80, Glyph::Ascii, 128),
            Preset::Banner => (120, Glyph::Blocks, 110),
            Preset::Logo => (60, Glyph::Braille, 90),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Options {
    /// Ancho de salida en columnas. El alto se deriva solo.
    pub cols: u16,
    pub glyph: Glyph,
    /// Umbral 0-255 para glifos monocromo (Braille/Blocks): encendido si luma > umbral.
    /// Se ignora en modo dither (usa el punto medio con difusión de error).
    pub threshold: u8,
    /// Invierte claro/oscuro (útil si el sujeto es oscuro sobre fondo claro).
    pub invert: bool,
    /// Dithering Floyd–Steinberg: sombreado por densidad de puntos en Braille/Blocks.
    pub dither: bool,
}

/// Una celda de salida: su carácter y la luminancia (0-255) de la zona que representa.
/// `luma == 0` significa fondo/transparente (sin tinta). Los adaptadores con color
/// escalan el tono de acento por `luma` para lograr sombreado.
#[derive(Clone, Copy)]
pub struct Cell {
    pub ch: char,
    pub luma: u8,
}

/// Artefacto neutro: grilla de celdas, agnóstica de quién la pinta.
pub struct Canvas {
    pub width: u16,
    pub height: u16,
    pub cells: Vec<Cell>, // row-major, width * height
}

impl Canvas {
    fn at(&self, x: u16, y: u16) -> Cell {
        self.cells[y as usize * self.width as usize + x as usize]
    }

    /// Vuelca solo los caracteres (monocromo), con saltos de línea. Para el `.txt`.
    pub fn to_mono(&self) -> String {
        let mut out = String::with_capacity((self.width as usize + 1) * self.height as usize);
        for y in 0..self.height {
            for x in 0..self.width {
                out.push(self.at(x, y).ch);
            }
            out.push('\n');
        }
        out
    }
}

/// Convierte una imagen a arte de texto monocromo (atajo sobre [`convert_canvas`]).
pub fn convert(img: &DynamicImage, opts: &Options) -> String {
    convert_canvas(img, opts).to_mono()
}

/// Convierte una imagen a un [`Canvas`] con luminancia por celda.
///
/// Único parámetro de tamaño: `cols`. Las filas se calculan corrigiendo por la
/// relación de aspecto de la celda, así la imagen NO sale deformada.
pub fn convert_canvas(img: &DynamicImage, opts: &Options) -> Canvas {
    let (w, h) = (img.width().max(1), img.height().max(1));
    let cols = opts.cols.max(1) as u32;
    let rows = ((cols as f32) * CELL_ASPECT * h as f32 / w as f32)
        .round()
        .max(1.0) as u32;

    let cells = match opts.glyph {
        Glyph::Ascii => ascii_cells(img, cols, rows, opts),
        Glyph::Braille => braille_cells(img, cols, rows, opts),
        Glyph::Blocks => blocks_cells(img, cols, rows, opts),
    };
    Canvas {
        width: cols as u16,
        height: rows as u16,
        cells,
    }
}

/// Reescala en RGBA (conservando alfa) al tamaño exacto de subpíxeles pedido.
fn rgba_grid(img: &DynamicImage, w: u32, h: u32) -> RgbaImage {
    img.resize_exact(w, h, FilterType::Triangle).to_rgba8()
}

/// Luminancia por subpíxel, o `None` si es transparente (fondo).
/// Respetar el alfa es lo que evita que un fondo transparente se dibuje.
fn subpixels(img: &DynamicImage, w: u32, h: u32, invert: bool) -> Vec<Option<u8>> {
    let g = rgba_grid(img, w, h);
    (0..w * h)
        .map(|i| {
            let p = g.get_pixel(i % w, i / w);
            if p[3] < ALPHA_MIN {
                return None;
            }
            let l = (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32).round() as u8;
            Some(if invert { 255 - l } else { l })
        })
        .collect()
}

/// Difunde el error de cuantización a un vecino (Floyd–Steinberg).
fn diffuse(buf: &mut [Option<f32>], w: u32, h: u32, x: u32, y: u32, dx: i64, dy: i64, e: f32) {
    let nx = x as i64 + dx;
    let ny = y as i64 + dy;
    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
        return;
    }
    if let Some(v) = buf[(ny as u32 * w + nx as u32) as usize].as_mut() {
        *v += e;
    }
}

/// Rejilla binaria on/off a resolución de subpíxeles.
/// - Sin dither: umbral duro (look stencil).
/// - Con dither: Floyd–Steinberg; los medios tonos se vuelven densidad de puntos,
///   dando sensación de sombra (zonas oscuras con más puntos, claras con menos).
/// Los píxeles transparentes quedan siempre en `false`.
fn on_grid(lum: &[Option<u8>], w: u32, h: u32, opts: &Options) -> Vec<bool> {
    if !opts.dither {
        let t = opts.threshold;
        return lum.iter().map(|o| o.is_some_and(|v| v > t)).collect();
    }

    let mut buf: Vec<Option<f32>> = lum.iter().map(|o| o.map(|v| v as f32)).collect();
    let mut on = vec![false; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) as usize;
            let Some(old) = buf[idx] else { continue };
            let lit = old >= 128.0;
            on[idx] = lit;
            let err = old - if lit { 255.0 } else { 0.0 };
            diffuse(&mut buf, w, h, x, y, 1, 0, err * 7.0 / 16.0);
            diffuse(&mut buf, w, h, x, y, -1, 1, err * 3.0 / 16.0);
            diffuse(&mut buf, w, h, x, y, 0, 1, err * 5.0 / 16.0);
            diffuse(&mut buf, w, h, x, y, 1, 1, err * 1.0 / 16.0);
        }
    }
    on
}

fn ascii_cells(img: &DynamicImage, cols: u32, rows: u32, opts: &Options) -> Vec<Cell> {
    let lum = subpixels(img, cols, rows, opts.invert);
    lum.iter()
        .map(|o| match o {
            None => Cell { ch: ' ', luma: 0 },
            Some(v) => Cell {
                ch: RAMP[*v as usize * (RAMP.len() - 1) / 255] as char,
                luma: *v,
            },
        })
        .collect()
}

fn braille_cells(img: &DynamicImage, cols: u32, rows: u32, opts: &Options) -> Vec<Cell> {
    let (w, h) = (cols * 2, rows * 4);
    let lum = subpixels(img, w, h, opts.invert);
    let on = on_grid(&lum, w, h, opts);
    // Mapa de bits Braille (U+2800). Filas dy=0..4, columnas dx=0..2.
    const MAP: [[u8; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];

    let mut cells = Vec::with_capacity((cols * rows) as usize);
    for cy in 0..rows {
        for cx in 0..cols {
            let (mut bits, mut sum, mut cnt) = (0u8, 0u32, 0u32);
            for dy in 0..4u32 {
                for dx in 0..2u32 {
                    let si = ((cy * 4 + dy) * w + (cx * 2 + dx)) as usize;
                    if on[si] {
                        bits |= MAP[dy as usize][dx as usize];
                    }
                    if let Some(v) = lum[si] {
                        sum += v as u32;
                        cnt += 1;
                    }
                }
            }
            cells.push(Cell {
                ch: char::from_u32(0x2800 + bits as u32).unwrap(),
                // Brillo = promedio de la zona opaca; 0 si la celda no tiene tinta.
                luma: if bits == 0 || cnt == 0 { 0 } else { (sum / cnt) as u8 },
            });
        }
    }
    cells
}

fn blocks_cells(img: &DynamicImage, cols: u32, rows: u32, opts: &Options) -> Vec<Cell> {
    let (w, h) = (cols * 2, rows * 2);
    let lum = subpixels(img, w, h, opts.invert);
    let on = on_grid(&lum, w, h, opts);
    // (dx, dy, bit) -> TL=1, TR=2, BL=4, BR=8.
    const POS: [(u32, u32, usize); 4] = [(0, 0, 1), (1, 0, 2), (0, 1, 4), (1, 1, 8)];

    let mut cells = Vec::with_capacity((cols * rows) as usize);
    for cy in 0..rows {
        for cx in 0..cols {
            let (mut idx, mut sum, mut cnt) = (0usize, 0u32, 0u32);
            for (dx, dy, bit) in POS {
                let si = ((cy * 2 + dy) * w + (cx * 2 + dx)) as usize;
                if on[si] {
                    idx |= bit;
                }
                if let Some(v) = lum[si] {
                    sum += v as u32;
                    cnt += 1;
                }
            }
            cells.push(Cell {
                ch: QUAD[idx],
                luma: if idx == 0 || cnt == 0 { 0 } else { (sum / cnt) as u8 },
            });
        }
    }
    cells
}
