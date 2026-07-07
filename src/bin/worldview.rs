//! worldview — a standalone world-inspection window: eyeball a seed's biome layout,
//! structure spawn rates and flora distribution at a glance, without playing.
//!
//! The modern take on the old Java "level gen preview" debug runnable. Everything is
//! driven by the pure generators (`infinite_gen`, `structures_gen`), so what you see is
//! byte-for-byte what the game generates for that seed.
//!
//! ```sh
//! cargo run --bin worldview -- [seed] [--depth N] [--mode biome|tile] [--zoom 1|2|4]
//! cargo run --bin worldview -- --dump <seed> <out.png> [--mode ...]   # headless PNG
//! ```
//!
//! Controls: arrows / W-A-D pan a chunk at a time, `+`/`-` zoom (1/2/4 px per tile),
//! Tab toggles biome/tile mode, `N` re-rolls a random seed, `S` screenshots to
//! `target/verify/worldview_<seed>.png`, Esc quits.
//!
//! The window/blit code deliberately mirrors `src/platform/mod.rs` (winit 0.30 +
//! softbuffer, scaled nearest-neighbour blit) but stays self-contained — no `Game`,
//! no tick loop.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use fdoom::gfx::{Screen, SpriteSheet, color, font};
use fdoom::level::chunk::{CHUNK_SIZE, chunk_coord};
use fdoom::level::infinite_gen::{Biome, biome_at, gates_in_rect, generate_chunk};
use fdoom::level::structures_gen::{
    MAX_RADIUS, StructureKind, placements_in_rect, trail_writes, trails_in_rect,
};
use fdoom::level::tile::Tiles;

/// Fixed internal framebuffer; the window scales it like the game scales its 288x192.
const VIEW_W: i32 = 960;
const VIEW_H: i32 = 640;

const CHUNK_AREA: usize = (CHUNK_SIZE * CHUNK_SIZE) as usize;

/// Unmapped tile ids render loud magenta so gaps in the color table are obvious.
const UNMAPPED: u32 = 0xFF00FF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Biome,
    Tile,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::Biome => "BIOME",
            Mode::Tile => "TILE",
        }
    }
}

/* ----------------------------------- color tables ----------------------------------- */

fn biome_color(b: Biome) -> u32 {
    match b {
        Biome::DeepOcean => 0x0B2E6B,
        Biome::Ocean => 0x1E5AC8,
        Biome::Beach => 0xE6D793,
        Biome::Mountains => 0x8C8C98,
        Biome::Tundra => 0xE9F1F7,
        Biome::Desert => 0xE4C468,
        Biome::Marsh => 0x4E8A66,
        Biome::Forest => 0x1F7A33,
        Biome::Savanna => 0xC9B457,
        Biome::Plains => 0x7CC353,
    }
}

const BIOME_LEGEND: [(Biome, &str); 10] = [
    (Biome::DeepOcean, "DEEP OCEAN"),
    (Biome::Ocean, "OCEAN"),
    (Biome::Beach, "BEACH"),
    (Biome::Plains, "PLAINS"),
    (Biome::Forest, "FOREST"),
    (Biome::Savanna, "SAVANNA"),
    (Biome::Marsh, "MARSH"),
    (Biome::Tundra, "TUNDRA"),
    (Biome::Desert, "DESERT"),
    (Biome::Mountains, "MOUNTAINS"),
];

fn structure_color(kind: StructureKind) -> u32 {
    match kind {
        StructureKind::Ruins => 0xFF8C1A,
        StructureKind::Cemetery => 0xBE5CFF,
        StructureKind::StandingStones => 0x00E5FF,
        StructureKind::Camp => 0xFFE12E,
        StructureKind::Village => 0xFF3030,
    }
}

const STRUCTURE_LEGEND: [(StructureKind, &str); 5] = [
    (StructureKind::Ruins, "RUINS"),
    (StructureKind::Cemetery, "CEMETERY"),
    (StructureKind::StandingStones, "STONES"),
    (StructureKind::Camp, "CAMP"),
    (StructureKind::Village, "VILLAGE"),
];

const GATE_COLOR: u32 = 0xFFFFFF;
const TRAIL_COLOR: u32 = 0xB08968;

/// Tile-id -> map color, in the spirit of `MapMenu::get_map_image` but covering the
/// full infinite-gen palette (local table on purpose: no screen-module import).
fn tile_color_table(tiles: &Tiles) -> [u32; 256] {
    let entries: &[(&str, u32)] = &[
        ("grass", 0x4CA33F),
        ("dirt", 0x8B6B4A),
        ("sand", 0xD9C77A),
        ("water", 0x2C63D6),
        ("Deep Water", 0x11337A),
        ("lava", 0xFF4E00),
        ("rock", 0x8A8A8A),
        ("hard rock", 0x60606C),
        ("tree", 0x1C5B26),
        ("cactus", 0x2F9E44),
        ("flower", 0x9ADB6E),
        ("small grass", 0x5DB052),
        ("medium grass", 0x55A94A),
        ("tall grass", 0x4A9B40),
        ("snow", 0xF2F6FC),
        ("snow tree", 0xC2D6E4),
        ("Mud", 0x5D4A33),
        ("Pine Tree", 0x14532D),
        ("Dead Tree", 0x9A8264),
        ("Willow", 0x3E7D4C),
        ("Palm Tree", 0x46B052),
        ("Flat-Crown Tree", 0x6B8F3B),
        ("Berry Bush", 0x9E3B63),
        ("Mushroom", 0xC77A5E),
        ("Fruiting Cactus", 0x59C24E),
        ("Seaweed", 0x2F9E77),
        ("Coral", 0xE07A70),
        ("Tidal Flat", 0xA9A26B),
        ("Reeds", 0x6FA34D),
        ("Dry Bush", 0xA68C4A),
        ("iron ore", 0xC98F6B),
        ("gold ore", 0xE8C93C),
        ("gem ore", 0xD861E0),
        ("lapis", 0x2B4FC9),
        ("Stone Wall", 0xB9B9C4),
        ("Stone Bricks", 0x93937B),
        ("Grave stone", 0xD3D3DC),
        ("Fence", 0x7E5C36),
        ("Wood Planks", 0xAD7C4B),
        ("torch dirt", 0xFFCF5A),
        ("Jack-O-Lantern", 0xFF8F1F),
        ("stairs down", 0xFFFFFF),
        ("stairs up", 0xFFFFFF),
        ("obsidian", 0x241B31),
        ("obsidian wall", 0x45305E),
    ];
    let mut table = [UNMAPPED; 256];
    for &(name, col) in entries {
        table[tiles.get(name).id as usize] = col;
    }
    table
}

/* ------------------------------------ world view ------------------------------------ */

/// One generated chunk pre-baked to colors for both modes.
struct Cached {
    tile_px: Vec<u32>,
    biome_px: Vec<u32>,
}

struct WorldView {
    tiles: Tiles,
    tile_colors: [u32; 256],
    cache: HashMap<(i32, i32), Cached>,
    /// Scratch 288x192 game screen used to rasterize the game font.
    text: Screen,
    frame: Vec<u32>,
    seed: i64,
    depth: i32,
    mode: Mode,
    zoom: i32,
    /// Center of the view, in global tile coordinates.
    cx: i32,
    cy: i32,
}

impl WorldView {
    fn new(seed: i64, depth: i32, mode: Mode, zoom: i32) -> WorldView {
        let tiles = Tiles::new();
        let tile_colors = tile_color_table(&tiles);
        WorldView {
            tiles,
            tile_colors,
            cache: HashMap::new(),
            text: Screen::new(Arc::new(SpriteSheet::from_png(fdoom::assets::SPRITES_PNG))),
            frame: vec![0; (VIEW_W * VIEW_H) as usize],
            seed,
            depth,
            mode,
            zoom,
            cx: 0,
            cy: 0,
        }
    }

    fn set_seed(&mut self, seed: i64) {
        self.seed = seed;
        self.cache.clear();
    }

    fn title(&self) -> String {
        format!(
            "worldview — seed {} | depth {} | {} | {}px/tile | center ({}, {})",
            self.seed,
            self.depth,
            self.mode.label(),
            self.zoom,
            self.cx,
            self.cy
        )
    }

    /// The visible tile rect (inclusive), padded by one tile.
    fn view_rect(&self) -> (i32, i32, i32, i32) {
        let hw = VIEW_W / (2 * self.zoom) + 1;
        let hh = VIEW_H / (2 * self.zoom) + 1;
        (self.cx - hw, self.cy - hh, self.cx + hw, self.cy + hh)
    }

    fn ensure_chunk(&mut self, ccx: i32, ccy: i32) {
        if self.cache.contains_key(&(ccx, ccy)) {
            return;
        }
        // generation is pure and fast: a full clear costs one repaint, nothing more
        if self.cache.len() > 2048 {
            self.cache.clear();
        }
        let chunk = generate_chunk(self.seed, self.depth, ccx, ccy, &self.tiles);
        let tile_px: Vec<u32> = chunk
            .tiles
            .iter()
            .map(|&t| self.tile_colors[t as usize])
            .collect();
        let mut biome_px = vec![0u32; CHUNK_AREA];
        let (bx, by) = (ccx * CHUNK_SIZE, ccy * CHUNK_SIZE);
        for ly in 0..CHUNK_SIZE {
            for lx in 0..CHUNK_SIZE {
                biome_px[(lx + ly * CHUNK_SIZE) as usize] =
                    biome_color(biome_at(self.seed, bx + lx, by + ly));
            }
        }
        self.cache.insert((ccx, ccy), Cached { tile_px, biome_px });
    }

    /// Screen position (pixel center) of a global tile.
    fn screen_pos(&self, tx: i32, ty: i32) -> (i32, i32) {
        (
            VIEW_W / 2 + (tx - self.cx) * self.zoom + self.zoom / 2,
            VIEW_H / 2 + (ty - self.cy) * self.zoom + self.zoom / 2,
        )
    }

    fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, col: u32) {
        let (x0, y0) = (x.max(0), y.max(0));
        let (x1, y1) = ((x + w).min(VIEW_W), (y + h).min(VIEW_H));
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.frame[(xx + yy * VIEW_W) as usize] = col;
            }
        }
    }

    /// Rasterize `s` with the game font (8px cells) directly into the frame.
    fn draw_text(&mut self, x: i32, y: i32, s: &str) {
        let white = color::get4(-1, 555, 555, 555);
        // the scratch screen is 288 wide -> 36 chars; draw in chunks
        let chars: Vec<char> = s.chars().collect();
        for (ci, chunk) in chars.chunks(32).enumerate() {
            let part: String = chunk.iter().collect();
            self.text.clear(0);
            font::draw(&part, &mut self.text, 0, 0, white);
            let base_x = x + (ci * 32 * 8) as i32;
            for yy in 0..8 {
                for xx in 0..(part.chars().count() as i32 * 8) {
                    let p = self.text.pixels[(xx + yy * fdoom::gfx::screen::W) as usize];
                    if p != 0 {
                        let (dx, dy) = (base_x + xx, y + yy);
                        if (0..VIEW_W).contains(&dx) && (0..VIEW_H).contains(&dy) {
                            self.frame[(dx + dy * VIEW_W) as usize] = p as u32 & 0x00FF_FFFF;
                        }
                    }
                }
            }
        }
    }

    /// A square marker with a black outline, centered on a tile, fixed screen size.
    fn draw_marker(&mut self, tx: i32, ty: i32, col: u32, half: i32) {
        let (sx, sy) = self.screen_pos(tx, ty);
        if !(-16..=VIEW_W + 16).contains(&sx) || !(-16..=VIEW_H + 16).contains(&sy) {
            return;
        }
        let d = half * 2 + 1;
        self.fill_rect(sx - half - 1, sy - half - 1, d + 2, d + 2, 0x000000);
        self.fill_rect(sx - half, sy - half, d, d, col);
    }

    /// Render one frame into `self.frame`.
    fn render(&mut self) {
        let (tx0, ty0, tx1, ty1) = self.view_rect();
        for ccy in chunk_coord(ty0)..=chunk_coord(ty1) {
            for ccx in chunk_coord(tx0)..=chunk_coord(tx1) {
                self.ensure_chunk(ccx, ccy);
            }
        }

        // base map
        for py in 0..VIEW_H {
            let ty = self.cy + (py - VIEW_H / 2).div_euclid(self.zoom);
            let ly = ty.rem_euclid(CHUNK_SIZE);
            let mut px = 0;
            while px < VIEW_W {
                let tx = self.cx + (px - VIEW_W / 2).div_euclid(self.zoom);
                let next = VIEW_W / 2 + (tx - self.cx + 1) * self.zoom;
                let run = (next - px).clamp(1, VIEW_W - px);
                let cached = &self.cache[&(chunk_coord(tx), chunk_coord(ty))];
                let i = (tx.rem_euclid(CHUNK_SIZE) + ly * CHUNK_SIZE) as usize;
                let col = match self.mode {
                    Mode::Biome => cached.biome_px[i],
                    Mode::Tile => cached.tile_px[i],
                };
                let row = (py * VIEW_W) as usize;
                self.frame[row + px as usize..row + (px + run) as usize].fill(col);
                px += run;
            }
        }

        // trails: real tiles already show them in TILE mode; overlay them in BIOME mode
        if self.depth == 0 && self.mode == Mode::Biome {
            let torch_id = self.tiles.get("torch dirt").id;
            for (a, b) in trails_in_rect(self.seed, tx0, ty0, tx1, ty1) {
                for (x, y, t) in trail_writes(self.seed, a, b, &self.tiles) {
                    if x < tx0 || x > tx1 || y < ty0 || y > ty1 {
                        continue;
                    }
                    let col = if t == torch_id {
                        self.tile_colors[torch_id as usize]
                    } else {
                        TRAIL_COLOR
                    };
                    let (sx, sy) = self.screen_pos(x, y);
                    let h = self.zoom / 2;
                    self.fill_rect(sx - h, sy - h, self.zoom, self.zoom, col);
                }
            }
        }

        // structure markers (both modes; surface only — mines have no structures)
        if self.depth == 0 {
            let pad = MAX_RADIUS;
            for p in placements_in_rect(self.seed, tx0 - pad, ty0 - pad, tx1 + pad, ty1 + pad) {
                let half = if p.kind == StructureKind::Village {
                    5
                } else {
                    3
                };
                self.draw_marker(p.x, p.y, structure_color(p.kind), half);
            }
        }
        // dungeon gates (depth -3 only; `gates_in_rect` is empty elsewhere)
        for (gx, gy) in gates_in_rect(self.seed, self.depth, tx0, ty0, tx1, ty1) {
            self.draw_marker(gx, gy, GATE_COLOR, 4);
        }

        self.draw_legend();
    }

    fn draw_legend(&mut self) {
        let mut rows: Vec<(u32, String)> = Vec::new();
        if self.depth == 0 {
            for (kind, name) in STRUCTURE_LEGEND {
                rows.push((structure_color(kind), name.to_string()));
            }
        }
        if self.depth == -3 {
            rows.push((GATE_COLOR, "DUNGEON GATE".into()));
        }
        if self.mode == Mode::Biome {
            if self.depth == 0 {
                rows.push((TRAIL_COLOR, "TRAIL".into()));
            }
            for (b, name) in BIOME_LEGEND {
                rows.push((biome_color(b), name.to_string()));
            }
        }

        let header = vec![
            format!("SEED {}", self.seed),
            format!(
                "{} DEPTH {} ZOOM {} C {} {}",
                self.mode.label(),
                self.depth,
                self.zoom,
                self.cx,
                self.cy
            ),
        ];
        let text_w = header
            .iter()
            .map(|s| s.chars().count())
            .chain(rows.iter().map(|(_, s)| s.chars().count() + 3))
            .max()
            .unwrap_or(0) as i32
            * 8;
        let w = text_w + 16;
        let h = 12 + header.len() as i32 * 12 + rows.len() as i32 * 12;
        self.fill_rect(6, 6, w + 2, h + 2, 0x3A404A);
        self.fill_rect(7, 7, w, h, 0x14181C);
        let mut y = 14;
        for line in header {
            self.draw_text(15, y, &line);
            y += 12;
        }
        y += 2;
        for (col, name) in rows {
            self.fill_rect(15, y, 9, 9, 0x000000);
            self.fill_rect(16, y + 1, 7, 7, col);
            self.draw_text(15 + 24, y, &name);
            y += 12;
        }
    }

    /// Count placements + gates in the current view and print them (S / --dump / N).
    fn print_view_stats(&self) {
        let (tx0, ty0, tx1, ty1) = self.view_rect();
        let (w, h) = (tx1 - tx0 + 1, ty1 - ty0 + 1);
        println!(
            "seed {} depth {}: structures in view ({}x{} tiles around ({}, {})):",
            self.seed, self.depth, w, h, self.cx, self.cy
        );
        if self.depth == 0 {
            let mut counts: Vec<(StructureKind, usize)> = Vec::new();
            for p in placements_in_rect(self.seed, tx0, ty0, tx1, ty1) {
                match counts.iter_mut().find(|(k, _)| *k == p.kind) {
                    Some((_, n)) => *n += 1,
                    None => counts.push((p.kind, 1)),
                }
            }
            for (kind, name) in STRUCTURE_LEGEND {
                let n = counts
                    .iter()
                    .find(|(k, _)| *k == kind)
                    .map(|&(_, n)| n)
                    .unwrap_or(0);
                println!("  {name:<9} {n}");
            }
        }
        let gates = gates_in_rect(self.seed, self.depth, tx0, ty0, tx1, ty1).len();
        if self.depth == -3 {
            println!("  GATES     {gates}");
        }
    }

    fn dump_png(&self, path: &std::path::Path) {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).expect("create output dir");
        }
        let file = std::fs::File::create(path).expect("create png");
        let mut enc =
            png::Encoder::new(std::io::BufWriter::new(file), VIEW_W as u32, VIEW_H as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().expect("png header");
        let mut data = Vec::with_capacity(self.frame.len() * 3);
        for &p in &self.frame {
            data.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
        }
        writer.write_image_data(&data).expect("png data");
        println!("wrote {}", path.display());
    }
}

/* ---------------------------------- window shell ------------------------------------ */

struct App {
    wv: WorldView,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    needs_render: bool,
}

impl App {
    fn refresh(&mut self) {
        self.needs_render = true;
        if let Some(w) = &self.window {
            w.set_title(&self.wv.title());
            w.request_redraw();
        }
    }

    fn on_key(&mut self, code: KeyCode) {
        let step = CHUNK_SIZE;
        match code {
            KeyCode::ArrowLeft | KeyCode::KeyA => self.wv.cx -= step,
            KeyCode::ArrowRight | KeyCode::KeyD => self.wv.cx += step,
            KeyCode::ArrowUp | KeyCode::KeyW => self.wv.cy -= step,
            KeyCode::ArrowDown => self.wv.cy += step,
            KeyCode::Equal | KeyCode::NumpadAdd => self.wv.zoom = (self.wv.zoom * 2).min(4),
            KeyCode::Minus | KeyCode::NumpadSubtract => self.wv.zoom = (self.wv.zoom / 2).max(1),
            KeyCode::Tab => {
                self.wv.mode = match self.wv.mode {
                    Mode::Biome => Mode::Tile,
                    Mode::Tile => Mode::Biome,
                }
            }
            KeyCode::KeyN => {
                self.wv.set_seed(random_seed());
                println!("new seed: {}", self.wv.seed);
                self.wv.print_view_stats();
            }
            KeyCode::KeyS => {
                if self.needs_render {
                    self.wv.render();
                    self.needs_render = false;
                }
                let path = std::path::PathBuf::from(format!(
                    "target/verify/worldview_{}.png",
                    self.wv.seed
                ));
                self.wv.dump_png(&path);
                self.wv.print_view_stats();
                return; // frame unchanged; no redraw needed
            }
            _ => return,
        }
        self.refresh();
    }

    /// Scaled nearest-neighbour blit, centered — same approach as `platform::App::redraw`.
    fn redraw(&mut self) {
        if self.needs_render {
            self.wv.render();
            self.needs_render = false;
        }
        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else {
            return;
        };
        let size = window.inner_size();
        let (win_w, win_h) = (size.width as i32, size.height as i32);
        let (Some(sw), Some(sh)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return;
        };
        if surface.resize(sw, sh).is_err() {
            return;
        }
        let Ok(mut buffer) = surface.buffer_mut() else {
            return;
        };
        let scale = (win_w as f32 / VIEW_W as f32).min(win_h as f32 / VIEW_H as f32);
        let ww = (VIEW_W as f32 * scale) as i32;
        let hh = (VIEW_H as f32 * scale) as i32;
        let xo = (win_w - ww) / 2;
        let yo = (win_h - hh) / 2;
        buffer.fill(0);
        for dy in 0..hh {
            let sy = ((dy as f32 / scale) as i32).clamp(0, VIEW_H - 1);
            let dest_row = ((dy + yo) * win_w) as usize;
            let src_row = (sy * VIEW_W) as usize;
            for dx in 0..ww {
                let sx = ((dx as f32 / scale) as i32).clamp(0, VIEW_W - 1);
                buffer[dest_row + (dx + xo) as usize] = self.wv.frame[src_row + sx as usize];
            }
        }
        let _ = buffer.present();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(self.wv.title())
            .with_inner_size(LogicalSize::new(VIEW_W as f64, VIEW_H as f64))
            .with_min_inner_size(LogicalSize::new(320.0, 240.0));
        let window = Rc::new(
            event_loop
                .create_window(attrs)
                .expect("could not create window"),
        );
        let context =
            softbuffer::Context::new(window.clone()).expect("could not create graphics context");
        let surface =
            softbuffer::Surface::new(&context, window.clone()).expect("could not create surface");
        self.window = Some(window);
        self.surface = Some(surface);
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) => {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    if let PhysicalKey::Code(code) = event.physical_key {
                        if code == KeyCode::Escape {
                            event_loop.exit();
                            return;
                        }
                        self.on_key(code);
                    }
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }
}

/* --------------------------------------- main ---------------------------------------- */

fn random_seed() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(1)
}

fn parse_seed(s: &str) -> Option<i64> {
    s.strip_prefix("seed=").unwrap_or(s).parse().ok()
}

fn usage() -> ! {
    eprintln!(
        "usage: worldview [seed] [--depth N] [--mode biome|tile] [--zoom 1|2|4] [--center X Y]\n       \
         worldview --dump <seed> <out.png> [--depth N] [--mode biome|tile] [--zoom 1|2|4]"
    );
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut seed: Option<i64> = None;
    let mut out: Option<String> = None;
    let mut dump = false;
    let mut depth = 0i32;
    let mut mode = Mode::Biome;
    let mut zoom = 2i32;
    let mut center = (0i32, 0i32);

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--dump" => dump = true,
            "--depth" => {
                i += 1;
                depth = args
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            "--mode" => {
                i += 1;
                mode = match args.get(i).map(String::as_str) {
                    Some("biome") => Mode::Biome,
                    Some("tile") => Mode::Tile,
                    _ => usage(),
                };
            }
            "--zoom" => {
                i += 1;
                zoom = args
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(|| usage());
            }
            "--center" => {
                let cx = args.get(i + 1).and_then(|s| s.parse().ok());
                let cy = args.get(i + 2).and_then(|s| s.parse().ok());
                match (cx, cy) {
                    (Some(x), Some(y)) => center = (x, y),
                    _ => usage(),
                }
                i += 2;
            }
            // plain `123` or `seed=123` (the latter is what `just worldview seed=123` passes)
            s if seed.is_none() && parse_seed(s).is_some() => seed = parse_seed(s),
            s if dump && out.is_none() => out = Some(s.to_string()),
            _ => usage(),
        }
        i += 1;
    }
    if !(-3..=0).contains(&depth) {
        eprintln!("--depth must be 0 (surface) or -1..-3 (mines)");
        std::process::exit(2);
    }
    if ![1, 2, 4].contains(&zoom) {
        eprintln!("--zoom must be 1, 2 or 4");
        std::process::exit(2);
    }

    let seed = seed.unwrap_or_else(random_seed);
    let mut wv = WorldView::new(seed, depth, mode, zoom);
    (wv.cx, wv.cy) = center;

    if dump {
        let out = out.unwrap_or_else(|| usage());
        wv.render();
        wv.dump_png(std::path::Path::new(&out));
        wv.print_view_stats();
        return;
    }

    println!("worldview — seed {seed}");
    println!("controls: arrows/W-A-D pan | +/- zoom | Tab biome/tile | N new seed");
    println!("          S screenshot -> target/verify/worldview_<seed>.png | Esc quit");
    wv.print_view_stats();

    let event_loop = EventLoop::new().expect("could not create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        wv,
        window: None,
        surface: None,
        needs_render: true,
    };
    event_loop.run_app(&mut app).expect("event loop error");
}
