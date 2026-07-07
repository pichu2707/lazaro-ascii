use image::DynamicImage;
use lazarobox_ascii::converter::{Canvas, Glyph, Options, convert_canvas};
use lazarobox_ascii::{Direction, Effect, Rgb, Style as ArtStyle, color_at, export, to_ansi_gradient};
use ratatui::crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Paragraph};
use std::error::Error;
use std::fs;
use std::io::stdout;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Piso de brillo del preview (mismo criterio que el export sombreado).
const SHADE_FLOOR: f32 = 0.30;
const GREEN: Color = Color::Rgb(55, 209, 130);
const CYAN: Color = Color::Rgb(72, 214, 230);

/// Assets de la pantalla de bienvenida.
const TITLE: &str = include_str!("assets/title.txt");
/// PNG del camaleón embebido; se rasteriza a Canvas sombreado en el arranque.
const CAM_BYTES: &[u8] = include_bytes!("assets/camaleon.png");

/// Gradientes curados seleccionables en el editor. Elegir "más de un color" en una TUI
/// se resuelve mejor con presets que editando paradas a mano.
const GRADIENTS: &[(&str, &[Rgb])] = &[
    ("Cyan", &[(0, 229, 255)]),
    ("Verde", &[(0, 255, 128)]),
    ("Cyan→Púrpura", &[(0, 229, 255), (185, 155, 242)]),
    ("Cyan→Azul→Púrpura", &[(0, 229, 255), (127, 180, 202), (185, 155, 242)]),
    ("Verde→Amarillo", &[(0, 255, 128), (255, 214, 0)]),
    ("Magenta→Cyan", &[(255, 0, 200), (0, 229, 255)]),
    ("Fuego", &[(255, 214, 0), (255, 120, 0), (255, 0, 200)]),
    ("Hielo", &[(235, 235, 235), (127, 180, 202), (0, 229, 255)]),
];

const DIRECTIONS: [Direction; 4] = [
    Direction::Horizontal,
    Direction::Vertical,
    Direction::Diagonal,
    Direction::Radial,
];

const EFFECTS: [Effect; 5] = [
    Effect::None,
    Effect::Scroll,
    Effect::Pulse,
    Effect::Wave,
    Effect::Hue,
];

const GLYPHS: [Glyph; 3] = [Glyph::Ascii, Glyph::Braille, Glyph::Blocks];

/// Guía de valores recomendados, visible en el panel lateral del editor.
const GUIDE: &str = "\
Caso     cols  glifo
─────────────────────
Icono     24   Braille
Avatar    40   Braille
README    80   ASCII
Banner   120   Blocks
Logo      60   Braille

Color y efectos
• c = gradiente (1+ tonos)
• x = dirección
• f = efecto (anima el
  preview; se hornea en
  el .ans en t=0)
• , . = velocidad
• s exporta txt+ans+
  json+rs (embebible).";

/// Extensiones de imagen reconocidas por el selector.
fn is_image(p: &Path) -> bool {
    matches!(
        p.extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tif" | "tiff" | "ico" | "tga")
    )
}

fn glyph_name(g: Glyph) -> &'static str {
    match g {
        Glyph::Ascii => "ASCII",
        Glyph::Braille => "Braille",
        Glyph::Blocks => "Blocks",
    }
}

fn glyph_index(g: Glyph) -> usize {
    match g {
        Glyph::Ascii => 0,
        Glyph::Braille => 1,
        Glyph::Blocks => 2,
    }
}

fn rgb_of(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (255, 255, 255),
    }
}

/// Escala el color de acento por el brillo de la celda (con piso).
fn shade(base: Color, luma: u8) -> Color {
    let (r, g, b) = rgb_of(base);
    let f = SHADE_FLOOR + (1.0 - SHADE_FLOOR) * (luma as f32 / 255.0);
    Color::Rgb((r as f32 * f) as u8, (g as f32 * f) as u8, (b as f32 * f) as u8)
}

/// Atenúa un color por un factor 0.0..=1.0 (para fade-in y pulsos).
fn dim(c: Color, f: f32) -> Color {
    let (r, g, b) = rgb_of(c);
    Color::Rgb((r as f32 * f) as u8, (g as f32 * f) as u8, (b as f32 * f) as u8)
}

fn lerp(a: u8, b: u8, f: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * f).round() as u8
}

/// Gradiente cíclico cian → verde → magenta → cian. `t` en cualquier float.
fn grad(t: f32) -> Color {
    const STOPS: [(u8, u8, u8); 4] = [
        (0, 229, 255),
        (0, 255, 128),
        (255, 0, 200),
        (0, 229, 255),
    ];
    let t = t.rem_euclid(1.0) * 3.0;
    let i = t.floor() as usize;
    let f = t - i as f32;
    let (r1, g1, b1) = STOPS[i];
    let (r2, g2, b2) = STOPS[i + 1];
    Color::Rgb(lerp(r1, r2, f), lerp(g1, g2, f), lerp(b1, b2, f))
}

/// Construye el preview aplicando el estilo de color completo (gradiente + efecto)
/// en el instante `t`. Usa el MISMO `color_at` que el export, así lo que ves es lo
/// que se guarda.
fn preview_text(canvas: &Canvas, style: &ArtStyle, t: f32) -> Text<'static> {
    let (w, h) = (canvas.width, canvas.height);
    let mut lines = Vec::with_capacity(h as usize);
    for y in 0..h {
        let mut spans = Vec::with_capacity(w as usize);
        for x in 0..w {
            let c = canvas.cells[y as usize * w as usize + x as usize];
            if c.ch == ' ' || c.luma == 0 {
                spans.push(Span::raw(" "));
            } else {
                let (r, g, b) = color_at(style, c.luma, x, y, w, h, t);
                spans.push(Span::styled(
                    c.ch.to_string(),
                    Style::default().fg(Color::Rgb(r, g, b)),
                ));
            }
        }
        lines.push(Line::from(spans));
    }
    Text::from(lines)
}

/// Pantallas de la aplicación.
enum Screen {
    Splash,
    Picker,
    Editor,
}

struct Item {
    name: String,
    path: PathBuf,
    is_dir: bool,
}

/// Estado del editor (solo válido cuando hay una imagen cargada).
struct Editor {
    img: DynamicImage,
    name: String,
    cols: u16,
    glyph_idx: usize,
    threshold: u8,
    invert: bool,
    dither: bool,
    grad_idx: usize,
    dir_idx: usize,
    effect_idx: usize,
    speed: f32,
    show_guide: bool,
    canvas: Canvas,
    status: String,
    status_ok: Option<bool>,
}

impl Editor {
    fn new(img: DynamicImage, name: String, start: Options) -> Self {
        let mut ed = Editor {
            img,
            name,
            cols: start.cols,
            glyph_idx: glyph_index(start.glyph),
            threshold: start.threshold,
            invert: start.invert,
            dither: start.dither,
            grad_idx: 3,  // Cyan→Azul→Púrpura (marca)
            dir_idx: 2,   // diagonal
            effect_idx: 0, // sin animación
            speed: 1.0,
            show_guide: true,
            canvas: Canvas {
                width: 0,
                height: 0,
                cells: Vec::new(),
            },
            status: "Ajusta los valores y exporta con s".into(),
            status_ok: None,
        };
        ed.rerender();
        ed
    }

    fn opts(&self) -> Options {
        Options {
            cols: self.cols,
            glyph: GLYPHS[self.glyph_idx],
            threshold: self.threshold,
            invert: self.invert,
            dither: self.dither,
        }
    }

    /// Estilo de color actual (gradiente + dirección + efecto + velocidad).
    fn style(&self) -> ArtStyle {
        ArtStyle {
            colors: GRADIENTS[self.grad_idx].1.to_vec(),
            direction: DIRECTIONS[self.dir_idx],
            effect: EFFECTS[self.effect_idx],
            speed: self.speed,
        }
    }

    fn animating(&self) -> bool {
        self.effect_idx != 0
    }

    fn rerender(&mut self) {
        self.canvas = convert_canvas(&self.img, &self.opts());
    }

    fn export(&mut self) {
        let stem = Path::new(&self.name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let style = self.style();
        let txt = format!("{stem}.txt");
        let ans = format!("{stem}.ans");
        let json = format!("{stem}.json");
        let rs = format!("{stem}.rs");
        let ok = [
            fs::write(&txt, self.canvas.to_mono()),
            fs::write(&ans, to_ansi_gradient(&self.canvas, &style, 0.0)),
            fs::write(&json, export::to_json(&self.canvas, &style)),
            fs::write(&rs, export::to_rust(&self.canvas, &style)),
        ]
        .iter()
        .all(|r| r.is_ok());

        if ok {
            let dir = std::env::current_dir()
                .map(|d| d.display().to_string())
                .unwrap_or_else(|_| ".".into());
            self.status = format!("✓ {dir}/  →  {txt} + {ans} + {json} + {rs}");
            self.status_ok = Some(true);
        } else {
            self.status = "✗ Error al exportar".into();
            self.status_ok = Some(false);
        }
    }
}

struct App {
    screen: Screen,
    cwd: PathBuf,
    entries: Vec<Item>,
    sel: usize,
    editor: Option<Editor>,
    start: Options,
    pick_status: String,
    /// Contador de frames para animar la bienvenida.
    tick: u64,
    /// Reloj de arranque para la animación en tiempo real del editor.
    started: Instant,
    /// Camaleón de la bienvenida, rasterizado con sombreado por celda.
    splash: Canvas,
}

impl App {
    fn new(start: Options) -> Self {
        // El camaleón se rasteriza una vez con Braille + sombreado (luma por celda).
        let splash = image::load_from_memory(CAM_BYTES)
            .map(|img| {
                crop_blank_rows(convert_canvas(
                    &img,
                    &Options {
                        cols: 56,
                        glyph: Glyph::Braille,
                        threshold: 55,
                        invert: false,
                        dither: false,
                    },
                ))
            })
            .unwrap_or(Canvas {
                width: 0,
                height: 0,
                cells: Vec::new(),
            });

        App {
            screen: Screen::Splash,
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            entries: Vec::new(),
            sel: 0,
            editor: None,
            start,
            pick_status: String::new(),
            tick: 0,
            started: Instant::now(),
            splash,
        }
    }

    fn enter_picker(&mut self) {
        self.reload_entries();
        self.sel = 0;
        self.screen = Screen::Picker;
    }

    fn reload_entries(&mut self) {
        self.entries = read_entries(&self.cwd);
        if self.sel >= self.entries.len() {
            self.sel = 0;
        }
    }

    fn open_selected(&mut self) {
        let Some(it) = self.entries.get(self.sel) else {
            return;
        };
        if it.is_dir {
            self.cwd = it.path.clone();
            self.reload_entries();
            self.sel = 0;
        } else {
            match image::open(&it.path) {
                Ok(img) => {
                    let name = it.name.clone();
                    self.editor = Some(Editor::new(img, name, self.start));
                    self.screen = Screen::Editor;
                    self.pick_status.clear();
                }
                Err(e) => self.pick_status = format!("No se pudo abrir: {e}"),
            }
        }
    }

    fn go_parent(&mut self) {
        if let Some(p) = self.cwd.parent() {
            self.cwd = p.to_path_buf();
            self.reload_entries();
            self.sel = 0;
        }
    }
}

fn read_entries(cwd: &Path) -> Vec<Item> {
    let (mut dirs, mut files) = (Vec::new(), Vec::new());
    if let Ok(rd) = fs::read_dir(cwd) {
        for e in rd.flatten() {
            let path = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                dirs.push(Item { name, path, is_dir: true });
            } else if is_image(&path) {
                files.push(Item { name, path, is_dir: false });
            }
        }
    }
    dirs.sort_by_key(|i| i.name.to_lowercase());
    files.sort_by_key(|i| i.name.to_lowercase());

    let mut items = Vec::new();
    if let Some(parent) = cwd.parent() {
        items.push(Item {
            name: "..".into(),
            path: parent.to_path_buf(),
            is_dir: true,
        });
    }
    items.extend(dirs);
    items.extend(files);
    items
}

/// Lanza la aplicación. Con una imagen válida arranca en el editor; si no,
/// muestra la pantalla de bienvenida.
pub fn run(initial: Option<&str>, start: Options) -> Result<(), Box<dyn Error>> {
    let mut app = App::new(start);
    if let Some(path) = initial {
        if let Ok(img) = image::open(path) {
            app.editor = Some(Editor::new(img, path.to_string(), start));
            app.screen = Screen::Editor;
        }
    }

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let mut term = Terminal::new(CrosstermBackend::new(stdout()))?;

    let res = event_loop(&mut term, &mut app);

    disable_raw_mode()?;
    execute!(term.backend_mut(), LeaveAlternateScreen)?;
    res
}

fn event_loop<B: Backend>(term: &mut Terminal<B>, app: &mut App) -> Result<(), Box<dyn Error>> {
    loop {
        term.draw(|f| ui(f, app))?;

        // Se anima (redibuja cada 80ms) la bienvenida y el editor con un efecto activo.
        // El resto bloquea en el evento (sin gasto de CPU).
        let animating = matches!(app.screen, Screen::Splash)
            || matches!((&app.screen, &app.editor), (Screen::Editor, Some(ed)) if ed.animating());
        if animating && !event::poll(Duration::from_millis(80))? {
            app.tick = app.tick.wrapping_add(1);
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match app.screen {
            Screen::Splash => match key.code {
                KeyCode::Enter | KeyCode::Char(' ') => app.enter_picker(),
                KeyCode::Char('q') | KeyCode::Esc => break,
                _ => {}
            },
            Screen::Picker => match key.code {
                KeyCode::Up | KeyCode::Char('k') => app.sel = app.sel.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    if app.sel + 1 < app.entries.len() {
                        app.sel += 1;
                    }
                }
                KeyCode::Enter => app.open_selected(),
                KeyCode::Backspace | KeyCode::Left => app.go_parent(),
                KeyCode::Esc => {
                    app.screen = Screen::Splash;
                    app.tick = 0;
                }
                KeyCode::Char('q') => break,
                _ => {}
            },
            Screen::Editor => match key.code {
                KeyCode::Char('q') => break,
                KeyCode::Esc => app.screen = Screen::Picker,
                other => {
                    if let Some(ed) = app.editor.as_mut() {
                        handle_editor_key(ed, other);
                    }
                }
            },
        }
    }
    Ok(())
}

fn handle_editor_key(ed: &mut Editor, code: KeyCode) {
    match code {
        KeyCode::Char('g') => {
            ed.glyph_idx = (ed.glyph_idx + 1) % GLYPHS.len();
            ed.rerender();
        }
        KeyCode::Char('c') => ed.grad_idx = (ed.grad_idx + 1) % GRADIENTS.len(),
        KeyCode::Char('x') => ed.dir_idx = (ed.dir_idx + 1) % DIRECTIONS.len(),
        KeyCode::Char('f') => ed.effect_idx = (ed.effect_idx + 1) % EFFECTS.len(),
        KeyCode::Char(',') => ed.speed = (ed.speed - 0.25).max(0.1),
        KeyCode::Char('.') => ed.speed = (ed.speed + 0.25).min(8.0),
        KeyCode::Char('d') => {
            ed.dither = !ed.dither;
            ed.rerender();
        }
        KeyCode::Char('h') => ed.show_guide = !ed.show_guide,
        KeyCode::Char('i') => {
            ed.invert = !ed.invert;
            ed.rerender();
        }
        KeyCode::Left | KeyCode::Char('[') => {
            ed.cols = ed.cols.saturating_sub(2).max(8);
            ed.rerender();
        }
        KeyCode::Right | KeyCode::Char(']') => {
            ed.cols = (ed.cols + 2).min(400);
            ed.rerender();
        }
        KeyCode::Up | KeyCode::Char('+') => {
            ed.threshold = ed.threshold.saturating_add(5);
            ed.rerender();
        }
        KeyCode::Down | KeyCode::Char('-') => {
            ed.threshold = ed.threshold.saturating_sub(5);
            ed.rerender();
        }
        KeyCode::Char('s') | KeyCode::Char('e') => ed.export(),
        _ => {}
    }
}

fn ui(f: &mut Frame, app: &App) {
    match app.screen {
        Screen::Splash => splash_ui(f, app),
        Screen::Picker => picker_ui(f, app),
        Screen::Editor => {
            if let Some(ed) = app.editor.as_ref() {
                let t = app.started.elapsed().as_secs_f32();
                editor_ui(f, ed, t);
            }
        }
    }
}

/// Línea de texto con gradiente animado por carácter.
fn grad_line(text: &str, width: f32, tick: u64, fade: f32) -> Line<'static> {
    let mut spans = Vec::with_capacity(text.chars().count());
    for (col, ch) in text.chars().enumerate() {
        if ch == ' ' {
            spans.push(Span::raw(" "));
        } else {
            let phase = col as f32 / width + tick as f32 * 0.02;
            spans.push(Span::styled(
                ch.to_string(),
                Style::default().fg(dim(grad(phase), fade)),
            ));
        }
    }
    Line::from(spans)
}

/// Recorta las filas totalmente vacías (arriba y abajo) de un Canvas.
/// Elimina el margen transparente de la imagen para compactar el splash.
fn crop_blank_rows(c: Canvas) -> Canvas {
    let w = c.width as usize;
    if w == 0 {
        return c;
    }
    let blank = |y: usize| (0..w).all(|x| c.cells[y * w + x].luma == 0);
    let h = c.height as usize;
    let mut top = 0;
    while top < h && blank(top) {
        top += 1;
    }
    let mut bot = h;
    while bot > top && blank(bot - 1) {
        bot -= 1;
    }
    Canvas {
        width: c.width,
        height: (bot - top) as u16,
        cells: c.cells[top * w..bot * w].to_vec(),
    }
}

/// Pinta un Canvas como líneas, con sombreado por celda y atenuación (fade).
fn canvas_lines(canvas: &Canvas, base: Color, fade: f32) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(canvas.height as usize);
    for y in 0..canvas.height {
        let mut spans = Vec::with_capacity(canvas.width as usize);
        for x in 0..canvas.width {
            let c = canvas.cells[y as usize * canvas.width as usize + x as usize];
            if c.ch == ' ' || c.luma == 0 {
                spans.push(Span::raw(" "));
            } else {
                spans.push(Span::styled(
                    c.ch.to_string(),
                    Style::default().fg(dim(shade(base, c.luma), fade)),
                ));
            }
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn splash_ui(f: &mut Frame, app: &App) {
    let area = f.area();
    // Fade-in en los primeros ~12 frames (~1s).
    let fade = (app.tick as f32 / 12.0).min(1.0);
    let width = TITLE
        .lines()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(1)
        .max(1) as f32;

    let mut lines: Vec<Line> = Vec::new();
    // Título LZBOX + subtítulo con gradiente cian→verde→magenta fluyendo.
    for l in TITLE.lines() {
        lines.push(grad_line(l, width, app.tick, fade));
    }
    lines.push(grad_line("· A S C I I ·", 14.0, app.tick, fade));
    lines.push(Line::raw(""));
    // Camaleón sombreado (luz/sombra en verde), atenuado por el fade-in.
    lines.extend(canvas_lines(&app.splash, GREEN, fade));
    lines.push(Line::raw(""));
    // Prompt con pulso suave.
    let pulse = 0.55 + 0.45 * ((app.tick as f32 * 0.15).sin() * 0.5 + 0.5);
    lines.push(Line::from(Span::styled(
        "Elige la imagen que vas a transformar",
        Style::default().fg(dim(GREEN, fade * pulse)),
    )));
    lines.push(Line::from(Span::styled(
        "Enter  elegir imagen      ·      q  salir",
        Style::default().fg(dim(Color::Rgb(120, 140, 130), fade)),
    )));

    // Centrado vertical.
    let pad = area.height.saturating_sub(lines.len() as u16) / 2;
    let mut all: Vec<Line> = vec![Line::raw(""); pad as usize];
    all.extend(lines);

    f.render_widget(Paragraph::new(all).alignment(Alignment::Center), area);
}

fn picker_ui(f: &mut Frame, app: &App) {
    let rows = Layout::vertical([Constraint::Min(1), Constraint::Length(4)]).split(f.area());

    let block = Block::bordered().title(format!(" {} ", app.cwd.display()));
    let inner = block.inner(rows[0]);
    f.render_widget(block, rows[0]);

    // Ventana de scroll para mantener la selección visible.
    let h = inner.height as usize;
    let off = if h > 0 && app.sel >= h { app.sel + 1 - h } else { 0 };
    let mut lines = Vec::new();
    for (i, it) in app.entries.iter().enumerate().skip(off).take(h) {
        let marker = if i == app.sel { "▶ " } else { "  " };
        let label = if it.is_dir {
            format!("{marker}{}/", it.name)
        } else {
            format!("{marker}{}", it.name)
        };
        let style = if i == app.sel {
            Style::default().fg(Color::Black).bg(GREEN)
        } else if it.is_dir {
            Style::default().fg(CYAN)
        } else {
            Style::default()
        };
        lines.push(Line::from(Span::styled(label, style)));
    }
    if app.entries.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (no hay imágenes ni carpetas aquí)",
            Style::default().fg(Color::DarkGray),
        )));
    }
    f.render_widget(Paragraph::new(lines), inner);

    let hint = Line::from("↑↓ mover  ·  Enter abrir/elegir  ·  ⌫ subir  ·  Esc atrás  ·  q salir");
    let status = Line::from(Span::styled(
        app.pick_status.clone(),
        Style::default().fg(Color::Red),
    ));
    f.render_widget(
        Paragraph::new(vec![hint, status]).block(Block::bordered().title(" seleccionar imagen ")),
        rows[1],
    );
}

fn editor_ui(f: &mut Frame, ed: &Editor, t: f32) {
    let rows = Layout::vertical([Constraint::Min(3), Constraint::Length(5)]).split(f.area());

    let style = ed.style();
    let preview = Paragraph::new(preview_text(&ed.canvas, &style, t))
        .block(Block::bordered().title(format!(" {} ", ed.name)));

    if ed.show_guide {
        let top = Layout::horizontal([Constraint::Min(20), Constraint::Length(27)]).split(rows[0]);
        f.render_widget(preview, top[0]);
        let guide = Paragraph::new(GUIDE)
            .block(Block::bordered().title(" guía "))
            .style(Style::default().fg(Color::Gray));
        f.render_widget(guide, top[1]);
    } else {
        f.render_widget(preview, rows[0]);
    }

    let grad_name = GRADIENTS[ed.grad_idx].0;
    let dir_name = DIRECTIONS[ed.dir_idx].as_str();
    let eff_name = EFFECTS[ed.effect_idx].as_str();
    let line1 = Line::from(vec![
        Span::styled("g", Style::default().fg(Color::Yellow)),
        format!(" glifo:{}  ", glyph_name(GLYPHS[ed.glyph_idx])).into(),
        Span::styled("←→", Style::default().fg(Color::Yellow)),
        format!(" cols:{}  ", ed.cols).into(),
        Span::styled("↑↓", Style::default().fg(Color::Yellow)),
        format!(" umbral:{}  ", ed.threshold).into(),
        Span::styled("d", Style::default().fg(Color::Yellow)),
        format!(" dither:{}  ", if ed.dither { "sí" } else { "no" }).into(),
        Span::styled("i", Style::default().fg(Color::Yellow)),
        format!(" invertir:{}", if ed.invert { "sí" } else { "no" }).into(),
    ]);
    let line2 = Line::from(vec![
        Span::styled("c", Style::default().fg(Color::Yellow)),
        format!(" color:{grad_name}  ").into(),
        Span::styled("x", Style::default().fg(Color::Yellow)),
        format!(" dir:{dir_name}  ").into(),
        Span::styled("f", Style::default().fg(Color::Yellow)),
        format!(" efecto:{eff_name}  ").into(),
        Span::styled(",.", Style::default().fg(Color::Yellow)),
        format!(" vel:{:.2}  ", ed.speed).into(),
        Span::styled("s", Style::default().fg(Color::Green)),
        " exportar  ".into(),
        Span::styled("Esc", Style::default().fg(Color::Cyan)),
        " atrás  ".into(),
        Span::styled("q", Style::default().fg(Color::Red)),
        " salir".into(),
    ]);
    let status_style = match ed.status_ok {
        Some(true) => Style::default().fg(Color::Green),
        Some(false) => Style::default().fg(Color::Red),
        None => Style::default().fg(Color::DarkGray),
    };
    let status = Line::from(ed.status.clone()).style(status_style);
    f.render_widget(
        Paragraph::new(vec![line1, line2, status]).block(Block::bordered().title(" controles ")),
        rows[1],
    );
}
