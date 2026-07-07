<p align="center">
  <img src="img/lazarobox-ascii.png" alt="LazaroBox ASCII" width="680">
</p>

<h1 align="center">LazaroBox ASCII</h1>

<p align="center">
  Conversor de imágenes a arte de terminal — ASCII, Braille y bloques Unicode.<br>
  CLI para automatizar · TUI con preview en vivo para afinar a ojo.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-2024-orange?logo=rust" alt="Rust 2024">
  <img src="https://img.shields.io/badge/versión-0.2.0-blue" alt="versión 0.2.0">
  <img src="https://img.shields.io/badge/UI-ratatui-purple" alt="ratatui">
  <img src="https://img.shields.io/badge/licencia-MIT-green" alt="MIT">
</p>

---

Conversor de imágenes a arte de texto (ASCII / Braille / bloques Unicode) escrito en Rust.
Pensado para generar logos, banners, avatares, splash screens y cabeceras de README a
distintos tamaños, listos para pegar en una TUI (ratatui), la portada de Neovim, o un `.md`.

## ✨ Características

- 🎨 **Tres glifos** — `ascii` (portable), `braille` (máximo detalle) y `blocks` (sólido).
- ⚡ **Presets calibrados** — `icon`, `avatar`, `readme`, `banner`, `logo`.
- 🖥️ **TUI con preview en vivo** — ajustás parámetros con el teclado y ves el resultado al instante, con la animación corriendo.
- 🌈 **Gradientes multi-color y efectos** — varias paradas de color (nombres o hex) con dirección (horizontal/vertical/diagonal/radial) y efectos animados (`scroll`, `pulse`, `wave`, `hue`).
- 📐 **Sin deformación** — el tamaño se mide en columnas y el alto se corrige por la relación de aspecto de la celda de terminal.
- 🧩 **Exportación embebible** — `.txt`, `.ans` (gradiente incrustado), `.json` (datos + estilo, agnóstico de lenguaje) y `.rs` (módulo Rust autocontenido con renderer de referencia) para acoplar el arte en cualquier herramienta o TUI.

## 💡 Idea

El núcleo (`src/converter.rs`) es **agnóstico**: recibe una imagen y devuelve un `String`.
No sabe nada de la terminal, de ratatui ni de archivos. La CLI (`src/main.rs`) es solo un
adaptador encima. El diseño separa tres ejes independientes:

- **Glifo** — cómo un bloque de píxeles se vuelve carácter (ASCII / Braille / bloques).
- **Tamaño** — medido en **columnas**; el alto se deriva solo, corrigiendo la relación de
  aspecto de la celda de terminal (~2× más alta que ancha) para que la imagen **no salga
  deformada**.
- **Umbral** — para vaciar el fondo en modos monocromo y lograr el look "stencil".

## 📦 Instalación

Necesitás [Rust](https://rustup.rs) (edición 2024).

```bash
git clone https://github.com/pichu2707/lazaro-ascii.git
cd lazaro-ascii

# Compilar en modo release
cargo build --release

# O instalarlo en el PATH
cargo install --path .
```

El binario queda en `target/release/lazarobox-ascii`.

## 🖥️ Uso

El binario necesita una imagen de entrada. Con `cargo run`, separá los argumentos con `--`:

```bash
cargo run --release -- entrada.png --preset logo -o salida.txt
```

O directamente el binario ya compilado:

```bash
./target/release/lazarobox-ascii entrada.png --preset logo -o salida.txt
```

Sin `-o`, imprime por stdout.

## 🎛️ Modo interactivo (TUI)

Al ejecutar la herramienta **sin argumentos** arranca en la pantalla de bienvenida
(título + logo), desde donde eliges la imagen con un **selector de archivos** y pasas
al editor:

```bash
cargo run --release            # bienvenida → selector → editor
```

También puedes abrir el editor directamente sobre una imagen con `--tui`, con **preview
en vivo**: ajustas los parámetros con el teclado y ves el resultado al instante.

```bash
cargo run --release -- entrada.png --tui
# o partiendo de un preset:
cargo run --release -- entrada.png --preset logo --tui
```

Flujo: **Bienvenida** (`Enter` para elegir) → **Selector** (`↑↓` mover, `Enter`
abrir/elegir, `⌫` subir carpeta) → **Editor**. En el editor, `Esc` vuelve al selector
para cargar otra imagen.

Controles:

| Tecla    | Acción                                     |
| -------- | ------------------------------------------ |
| `g`      | Cambia de glifo (ASCII → Braille → Blocks)   |
| `← →`    | Ancho en columnas                            |
| `↑ ↓`    | Umbral                                       |
| `d`      | Dither on/off                                |
| `i`      | Invierte claro/oscuro                        |
| `c`      | Cicla gradiente (1+ colores)                 |
| `x`      | Cicla dirección del gradiente                |
| `f`      | Cicla efecto (none/scroll/pulse/wave/hue)    |
| `, .`    | Baja / sube la velocidad de la animación     |
| `h`      | Muestra/oculta el panel de guía              |
| `s`      | Exporta los 4 formatos (`.txt .ans .json .rs`) |
| `q`      | Salir                                        |

Con un efecto activo el preview **anima en vivo**. Al exportar genera **cuatro archivos**
(ver [Formatos embebibles](#-formatos-embebibles)): `.txt`, `.ans`, `.json` y `.rs`.

> El TUI necesita una terminal real (usa raw mode). No corre por pipes ni en CI.

## 🎚️ Presets

Cada preset fija `cols` + `glyph` + `threshold` calibrados. Cualquier flag explícito lo pisa.

| `--preset` | cols | glifo   | Caso de uso                     |
| ---------- | ---- | ------- | ------------------------------- |
| `icon`     | 24   | braille | Iconos                          |
| `avatar`   | 40   | braille | Avatares / thumbnails           |
| `readme`   | 80   | ascii   | Cabecera de README (copia-pega) |
| `banner`   | 120  | blocks  | Banner de GitHub / splash       |
| `logo`     | 60   | braille | Logo tipo stencil               |

## 🚩 Flags

| Flag              | Descripción                                                       | Default   |
| ----------------- | ---------------------------------------------------------------- | --------- |
| `--preset <p>`    | Preset por caso de uso (ver tabla).                              | —         |
| `--cols <n>`      | Ancho en columnas. El alto se calcula solo.                      | 80        |
| `--glyph <g>`     | `ascii`, `braille` o `blocks`.                                   | `braille` |
| `--threshold <n>` | Umbral 0-255 para Braille/Blocks. El fondo queda vacío por debajo. | 128       |
| `--invert`        | Invierte claro/oscuro.                                           | off       |
| `--dither`        | Dithering Floyd–Steinberg (sombra por densidad de puntos).      | off       |
| `--color <c>`     | Color de acento único (atajo). `cyan`, `green`, `magenta`, `yellow`, `orange`, `white`. | — |
| `--colors <lista>`| Paradas del gradiente separadas por coma. Nombres o hex. Ej: `cyan,#7FB4CA,purple`. | — |
| `--gradient <d>`  | Dirección: `horizontal`, `vertical`, `diagonal`, `radial`.       | horizontal |
| `--effect <e>`    | Efecto: `none`, `scroll`, `pulse`, `wave`, `hue`.               | none      |
| `--speed <f>`     | Velocidad de la animación (ciclos/seg, aprox).                  | 1.0       |
| `-f, --format <f>`| Salida: `txt`, `ans`, `json`, `rust`. Si se omite, se infiere de la extensión de `--out`. | auto |
| `-o, --out <f>`   | Archivo de salida. Sin esto, imprime por stdout.                | stdout    |

## 🔤 Glifos

- **`ascii`** — rampa de densidad (`" .:-=+*#%@"`), 1 píxel por celda. Portable, se copia y
  pega en cualquier lado. Ideal para README.
- **`braille`** — 2×4 subpíxeles por celda (bloque Unicode U+2800). Máximo detalle, look de
  puntitos. Ideal para logos e iconos chicos donde cada píxel cuenta.
- **`blocks`** — bloques de cuadrante 2×2. Look sólido tipo stencil.

## 🌈 Colores y efectos

El color no es un tono plano: cada celda toma su tono de un **gradiente** muestreado en su
posición y **sombreado por la luminancia** de esa zona (`color_at`). Con un **efecto** el
gradiente se modula en función del tiempo `t`.

```bash
# Gradiente diagonal de 3 paradas (nombres + hex)
lazarobox-ascii aguila.png --glyph braille --cols 70 \
  --colors cyan,#7FB4CA,purple --gradient diagonal -o aguila.ans

# Arcoíris radial
lazarobox-ascii aguila.png --effect hue --gradient radial -o aguila.ans
```

- **Dirección** (`--gradient`): `horizontal`, `vertical`, `diagonal`, `radial`.
- **Efectos** (`--effect`): `scroll` (el gradiente se desplaza), `pulse` (latido de brillo),
  `wave` (ondula la posición), `hue` (ciclo de matiz arcoíris, ignora `--colors`).
- El `.ans` estático se hornea en `t = 0`; la animación se ve en la TUI y en el renderer de
  referencia del `.rs`/`.json`.

## 🧩 Formatos embebibles

La misma grilla + estilo se exporta en formatos que **otra herramienta o TUI puede consumir**.
Una animación no se guarda como frames: se exporta la **receta** (colores + dirección +
efecto + velocidad) y los **datos** (celdas + luma); el consumidor la reproduce en su propio
render loop.

**`.json`** — agnóstico de lenguaje (schema `lazarobox-ascii/v1`):

```json
{
  "format": "lazarobox-ascii/v1",
  "width": 70,
  "height": 24,
  "style": {
    "colors": [[0,229,255],[127,180,202],[185,155,242]],
    "direction": "diagonal",
    "effect": "scroll",
    "speed": 1.0
  },
  "grid": [{ "c": "⣿", "l": 200 }, { "c": "⠿", "l": 140 }]
}
```

`grid` es row-major de longitud `width * height`; las celdas vacías van con `"l": 0`.

**`.rs`** — módulo Rust autocontenido (cero dependencias). Copiás el archivo a tu TUI y
llamás a `color_at(luma, x, y, t)` desde tu loop, avanzando `t` (segundos) para animar:

```rust
mod art; // el .rs exportado

// dentro de tu render (ratatui), por cada celda:
let (r, g, b) = art::color_at(luma, x, y, t);
// spans.push(Span::styled(ch.to_string(), Style::default().fg(Color::Rgb(r, g, b))));
```

Expone `WIDTH`, `HEIGHT`, `CELLS: &[(char, u8)]`, `COLORS`, `DIRECTION`, `EFFECT`, `SPEED`.

## 🎯 Valores recomendados (calidad)

Punto de partida por caso de uso. Ajustá desde acá según tu imagen.

| Caso de uso | `--cols` | glifo   | Notas                                         |
| ----------- | -------- | ------- | --------------------------------------------- |
| Icono       | 24       | braille | Alto detalle en poco espacio.                 |
| Avatar      | 40       | braille | Thumbnail reconocible.                        |
| README      | 80       | ascii   | Portable, copia-pega en cualquier `.md`.      |
| Banner      | 120      | blocks  | Ancho, sólido, buen impacto visual.           |
| Logo        | 60       | braille | Stencil limpio; subí el umbral si hace falta. |

Reglas para que salga con calidad:

- **Más `cols` = más detalle** (y más ancho de salida). Es la palanca principal.
- **Fondo transparente**: Braille y Blocks lucen mejor que ASCII. La transparencia se
  respeta automáticamente — el fondo queda vacío, no se rellena.
- **Sujeto claro sobre fondo oscuro**: NO uses `--invert`.
- **Umbral**: bajo = más relleno; subilo para limpiar el fondo en modos monocromo.

Esta misma guía aparece en el panel lateral del modo `--tui`.

## 💡 Tips

- **El tamaño se mide en columnas, no en píxeles.** Pasá solo `--cols`; el alto se ajusta
  para mantener la proporción real.
- **Sujeto claro sobre fondo oscuro → SIN `--invert`.** Invertir enciende todo el fondo.
  Usá `--invert` solo cuando el sujeto es oscuro sobre fondo claro.
- **Recortá la fuente al sujeto.** No le pases un screenshot entero: el sujeto queda chico
  y pierde detalle.
- Subí o bajá `--threshold` para controlar cuánto detalle entra en los modos monocromo.

## 📕 Ejemplos

```bash
# Logo stencil guardado a archivo
cargo run --release -- lobo.png --preset logo -o lobo.txt

# Banner ancho, override del preset
cargo run --release -- lobo.png --preset banner --cols 100

# ASCII portable para README, sujeto oscuro sobre fondo claro
cargo run --release -- foto.png --glyph ascii --invert
```

## 📄 Licencia

[MIT](LICENSE) © pichu2707

---

<p align="center">Hecho con 🦀 · <b>LazaroBox</b></p>
