//! pixel_studio — the game's pixel-art studio. The split sprite PNGs under
//! `assets/sprites/**` are the art source of truth (docs/ART_GUIDE.md); this tool
//! browses, previews, and edits them in place.
//!
//! Two sources, one editor:
//!
//! - **Directory mode** (primary): point it at a folder (default `assets/sprites`)
//!   and the left pane is a file browser over every `*.png` under it. Opening a file
//!   sizes the editor to the image; bigger strips are edited one window at a time.
//!   When the folder has a `manifest.txt` (the atlas manifest), each file's declared
//!   `pal`/`rgb` mode drives precise wrong-mode warnings.
//! - **Sheet mode** (fallback, for `assets/golden_atlas.png` or any stitched atlas):
//!   the left pane shows the whole sheet at 2x. The editing window sits at any 8px
//!   cell (no even-cell snapping); `G` jumps it to the sprite under the cursor with
//!   its true footprint via a built-in sprite map, and the header names that sprite.
//!
//! ```sh
//! cargo run --bin pixel_studio                                  # assets/sprites
//! cargo run --bin pixel_studio -- --sheet assets/golden_atlas.png --cell 15 26
//! cargo run --bin pixel_studio -- <png> --set X Y RRGGBB        # headless batch edit
//! cargo run --bin pixel_studio -- <dir> --file tiles/grass_texture.png --set X Y t
//! cargo run --bin pixel_studio -- <target> --shot out.png       # headless UI frame
//! ```
//!
//! Press `?` in-app for the full key list. Highlights: palette-applied preview (`P`
//! cycles real game palettes so grayscale sprites show as the game draws them),
//! in-context preview over grass/sand/night backdrops, animation preview (`A`),
//! onion skin (`B` capture / `O` toggle), line/rect tools (`L`/`R`/Shift+`R`),
//! mirror-draw (`M`), copy/paste (Ctrl+C/V), shade-shift (`[`/`]`), image nudge
//! (Shift+arrows, wraps), wheel zoom at the cursor, middle-drag pan.
//!
//! Pixel semantics mirror `src/gfx/sprite_sheet.rs`: opaque grays (`r==g==b`) are
//! palette pixels recolored at draw time (legal shades are exactly 0/85/170/255),
//! any saturated color draws literally, alpha < 128 is transparent. Mixing the two
//! modes in one 8x8 cell (or violating a file's manifest mode) gets a warning.
//!
//! The window shell mirrors `worldview` (winit 0.30 + softbuffer, scaled blit); the
//! UI is drawn rects + the game font. No `Game`, no new dependencies.

use std::collections::{HashMap, HashSet, VecDeque};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Window, WindowId};

use fdoom::gfx::{Screen, color, font};

/* ------------------------------------ layout ------------------------------------ */

const VIEW_W: i32 = 960;
const VIEW_H: i32 = 720;

/// Sprite cell edge (must match `sprite_sheet::BOX_WIDTH`).
const CELL: i32 = 8;
/// The only legal palette-mode grays (the loader quantizes `r/64` into shades 0-3).
const GRAYS: [u8; 4] = [0, 85, 170, 255];

const PANE_X: i32 = 8;
const PANE_Y: i32 = 56;
const PANE_W: i32 = 512; // sheet browser: 256 sheet px at 2x; dir mode: file list
const PANE_H: i32 = 512;
const ROW_H: i32 = 12; // file-list line height

const RX: i32 = 536; // right pane origin
const CANVAS_Y: i32 = 56;
const CANVAS_MAX: i32 = 384; // canvas viewport is CANVAS_MAX x CANVAS_MAX
const PAL_A_Y: i32 = 450;
const PAL_B_Y: i32 = 480;
const RECENT_Y: i32 = 520;
const RGB_Y: i32 = 542;
const PREVIEW_Y: i32 = 568;
const SWATCH_X: i32 = RX + 88;

const BG: u32 = 0x14181C;
const PANEL: u32 = 0x0C0F13;
const GRID: u32 = 0x262C34;
const GRID_MAJOR: u32 = 0x3E4854;
const ACCENT: u32 = 0xFFD24A;
const TXT: i32 = 555; // readable-color text values for draw_text
const TXT_DIM: i32 = 333;
const TXT_WARN: i32 = 540;

/// Game walk cadence: mobs flip frames on `walk_dist >> 3` — about every 8 ticks at
/// 60 tps, so ~133 ms per animation frame.
const ANIM_FRAME: Duration = Duration::from_millis(133);

/* --------------------------------- pixel helpers --------------------------------- */

type Rgba = [u8; 4];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Transparent,
    Gray(u8),
    Color,
}

fn classify(p: Rgba) -> Kind {
    if p[3] < 128 {
        Kind::Transparent
    } else if p[0] == p[1] && p[1] == p[2] {
        Kind::Gray(p[0])
    } else {
        Kind::Color
    }
}

fn rgb24(p: Rgba) -> u32 {
    ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32
}

/// Flood-fill / equality key: everything transparent is one bucket, opaque by rgb.
fn key(p: Rgba) -> u32 {
    if p[3] < 128 { u32::MAX } else { rgb24(p) }
}

fn checker(x: i32, y: i32) -> u32 {
    if (x + y) % 2 == 0 { 0x30363E } else { 0x22272E }
}

/// `t/256` blend of `a` over `b` (0 = all b, 256 = all a).
fn blend(a: u32, b: u32, t: u32) -> u32 {
    let f = |sh: u32| {
        let (ca, cb) = ((a >> sh) & 0xFF, (b >> sh) & 0xFF);
        ((ca * t + cb * (256 - t)) >> 8) & 0xFF
    };
    (f(16) << 16) | (f(8) << 8) | f(0)
}

/// The fixed "night grade": a blue-shifted multiply approximating the game's darkest
/// overworld lighting.
fn night(c: u32) -> u32 {
    let r = ((c >> 16 & 0xFF) * 100) >> 8;
    let g = ((c >> 8 & 0xFF) * 115) >> 8;
    let b = ((c & 0xFF) * 175) >> 8;
    (r << 16) | (g << 8) | b
}

/* ------------------------------- game palette data ------------------------------- */

/// Preview palettes: real `color::get4` words from game code, so palette-mode art
/// previews exactly as the game will draw it. Index 0 = raw grays.
/// Sources: player render (`player_behavior.rs`, default shirt color 110),
/// `zombie::LVLCOLS`, `registry::TOOL_LEVEL_COLORS`.
const PREVIEW_PALS: &[(&str, i32)] = &[
    ("RAW GRAYS", 0),
    ("PLAYER", color::get4(-1, 100, 110, 532)),
    ("ZOMBIE L1", color::get4(-1, 10, 152, 40)),
    ("ZOMBIE L2", color::get4(-1, 100, 522, 40)),
    ("ZOMBIE L3", color::get4(-1, 111, 444, 40)),
    ("ZOMBIE L4", color::get4(-1, 0, 111, 20)),
    ("TOOL CRUDE", color::get4(-1, 100, 221, 332)),
    ("TOOL WOOD", color::get4(-1, 100, 321, 431)),
    ("TOOL ROCK", color::get4(-1, 100, 321, 111)),
    ("TOOL IRON", color::get4(-1, 100, 321, 555)),
    ("TOOL GOLD", color::get4(-1, 100, 321, 550)),
    ("TOOL GEM", color::get4(-1, 100, 321, 55)),
];

/// Region map of the classic 256x256 atlas layout (row = 8px cell row), so the
/// sheet-mode browser can label unmapped cells. Dir-mode files are labeled by their
/// folder instead.
fn artgen_region(row: i32) -> &'static str {
    match row {
        0..=3 => "TERRAIN",
        4..=5 => "ITEMS",
        6..=7 => "TITLE LOGO",
        8..=10 => "FURNITURE",
        11..=13 => "UI + GRAVES",
        14..=19 => "MOBS",
        20..=21 => "MOBS + FIRE FX",
        22..=23 => "MOBS",
        24..=25 => "DECOR",
        26..=29 => "FLORA",
        30..=31 => "FONT",
        _ => "?",
    }
}

/// Sprite map for atlas sheets: `(cx, cy, w, h, uw, uh, name)` in 8px cells. An
/// entry is a sprite or a strip of same-size sprites; `uw x uh` is the footprint of
/// one sprite inside it (`G` snaps the window to the unit under the cursor). Mirrors
/// the manifest / `tests/artgen_sheet.rs` inventory — including odd-origin blocks
/// (graves at x 15/17/19..., decor flora at (15,26), ...).
type SpriteSpan = (i32, i32, i32, i32, i32, i32, &'static str);
const SPRITE_MAP: &[SpriteSpan] = &[
    (0, 0, 4, 1, 4, 1, "TERRAIN DOTS TILE"),
    (22, 0, 4, 1, 4, 1, "GRASS TUFT TILE"),
    (26, 0, 4, 1, 4, 1, "SAND RIPPLE TILE"),
    (13, 3, 4, 1, 4, 1, "SNOW DRIFT TILE"),
    (21, 3, 4, 1, 4, 1, "DIRT CLOD TILE"),
    (25, 3, 4, 1, 4, 1, "STONE PLATE TILE"),
    (24, 1, 2, 2, 2, 2, "MUD BLOCK"),
    (4, 0, 3, 3, 3, 3, "ROCK SPARSE BLOB"),
    (7, 0, 2, 2, 2, 2, "ROCK SIDES"),
    (9, 0, 2, 2, 2, 2, "TREE OUTER PIECES"),
    (11, 0, 3, 3, 3, 3, "GRASS SPARSE BLOB"),
    (14, 0, 3, 3, 3, 3, "WATER SPARSE BLOB"),
    (17, 1, 2, 2, 2, 2, "ORE NUB"),
    (22, 1, 2, 2, 2, 2, "QUICKSAND"),
    (0, 2, 2, 2, 2, 2, "STAIRS DOWN"),
    (2, 2, 2, 2, 2, 2, "STAIRS UP"),
    (8, 2, 2, 2, 2, 2, "CACTUS"),
    (19, 2, 2, 2, 2, 2, "FLOOR / LAVA BRICK"),
    (4, 3, 4, 1, 1, 1, "WHEAT STAGE"),
    (0, 4, 32, 1, 1, 1, "ITEM ICON"),
    (0, 5, 32, 1, 1, 1, "ITEM ICON"),
    (0, 6, 15, 2, 15, 2, "TITLE: DOOM STRIP"),
    (16, 6, 15, 2, 15, 2, "TITLE: KICKER STRIP"),
    (0, 8, 22, 2, 2, 2, "FURNITURE"),
    (22, 8, 2, 2, 2, 2, "PUMPKIN"),
    (26, 8, 2, 2, 2, 2, "TALL GRASS: TALL"),
    (30, 8, 2, 2, 2, 2, "TALL GRASS: MEDIUM"),
    (28, 9, 2, 1, 2, 1, "TALL GRASS: SMALL"),
    (0, 10, 18, 1, 1, 1, "FURNITURE / FOOD ICON"),
    (11, 11, 2, 2, 2, 2, "GRAVE: SLAB"),
    (13, 11, 2, 2, 2, 2, "GRAVE: RUBBLE"),
    (15, 11, 2, 2, 2, 2, "GRAVE: ROUNDED"),
    (17, 11, 2, 2, 2, 2, "GRAVE: STONE CROSS"),
    (19, 11, 2, 2, 2, 2, "GRAVE: CRACKED SLAB"),
    (21, 11, 2, 2, 2, 2, "GRAVE: RUBBLE B"),
    (23, 11, 2, 2, 2, 2, "GRAVE: WOODEN CROSS"),
    (25, 11, 2, 2, 2, 2, "GRAVE: BROKEN CROSS"),
    (0, 12, 7, 1, 1, 1, "HUD ICON"),
    (0, 13, 9, 1, 1, 1, "UI FRAME / FX"),
    (0, 14, 8, 2, 2, 2, "PLAYER/ZOMBIE WALK FRAMES"),
    (8, 14, 8, 2, 2, 2, "MARSH LURKER FRAMES"),
    (16, 14, 8, 2, 2, 2, "PIG FRAMES"),
    (24, 14, 8, 2, 2, 2, "KNIGHT FRAMES"),
    (0, 16, 8, 2, 2, 2, "PLAYER CARRY FRAMES"),
    (8, 16, 8, 2, 2, 2, "FERAL HOUND FRAMES"),
    (16, 16, 8, 2, 2, 2, "COW FRAMES"),
    (0, 18, 8, 2, 2, 2, "STONE GOLEM FRAMES"),
    (10, 18, 8, 2, 2, 2, "SHEEP FRAMES"),
    (18, 18, 8, 2, 2, 2, "SNAKE FRAMES"),
    (8, 18, 2, 1, 1, 1, "SMOKE PUFF"),
    (0, 20, 4, 2, 2, 2, "NIGHT WISP FRAMES"),
    (4, 20, 2, 2, 2, 2, "RATTLER COIL"),
    (6, 20, 4, 2, 2, 2, "GHOST PULSE FRAMES"),
    (12, 20, 6, 2, 2, 2, "CAMPFIRE"),
    (10, 21, 2, 1, 1, 1, "TILE-FIRE OVERLAY"),
    (18, 20, 8, 2, 2, 2, "PLAYER SUIT FRAMES"),
    (18, 22, 8, 2, 2, 2, "SUIT CARRY FRAMES"),
    (0, 24, 2, 2, 2, 2, "OPEN DOOR"),
    (2, 24, 2, 2, 2, 2, "CLOSED DOOR"),
    (4, 22, 3, 3, 3, 3, "WOOD WALL SPARSE"),
    (7, 22, 2, 2, 2, 2, "WOOD WALL SIDES"),
    (4, 25, 3, 3, 3, 3, "STONE WALL SPARSE"),
    (7, 24, 2, 2, 2, 2, "STONE WALL SIDES"),
    (0, 26, 4, 3, 2, 3, "PINE / DEAD TREE SET"),
    (7, 26, 8, 3, 2, 3, "TREE SPECIES SET"),
    (15, 26, 16, 2, 2, 2, "DECOR FLORA"),
    (15, 28, 4, 2, 2, 2, "MUSHROOM / DRY BUSH"),
    (19, 28, 12, 2, 2, 2, "TREE VARIANT B"),
    (0, 30, 32, 2, 1, 1, "FONT GLYPH"),
];

/// The most specific (smallest) sprite-map entry containing cell `(ccx, ccy)`.
fn sprite_at(ccx: i32, ccy: i32) -> Option<&'static SpriteSpan> {
    SPRITE_MAP
        .iter()
        .filter(|&&(cx, cy, w, h, ..)| (cx..cx + w).contains(&ccx) && (cy..cy + h).contains(&ccy))
        .min_by_key(|&&(_, _, w, h, ..)| w * h)
}

/* ----------------------------------- png io ----------------------------------- */

struct Image {
    w: i32,
    h: i32,
    px: Vec<Rgba>,
}

fn load_png(path: &Path) -> Result<Image, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; reader.output_buffer_size().ok_or("png too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    if info.bit_depth != png::BitDepth::Eight {
        return Err(format!("{}: only 8-bit PNGs supported", path.display()));
    }
    let channels = match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return Err("unexpanded indexed png".into()),
    };
    let (w, h) = (info.width as i32, info.height as i32);
    let mut px = Vec::with_capacity((w * h) as usize);
    for p in buf[..info.buffer_size()].chunks_exact(channels) {
        px.push(match channels {
            1 => [p[0], p[0], p[0], 255],
            2 => [p[0], p[0], p[0], p[1]],
            3 => [p[0], p[1], p[2], 255],
            _ => [p[0], p[1], p[2], p[3]],
        });
    }
    Ok(Image { w, h, px })
}

fn write_png(path: &Path, img: &Image) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), img.w as u32, img.h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    let mut data = Vec::with_capacity(img.px.len() * 4);
    for p in &img.px {
        data.extend_from_slice(p);
    }
    writer.write_image_data(&data).map_err(|e| e.to_string())
}

fn bak_path(path: &Path) -> PathBuf {
    path.with_extension("bak.png")
}

/// Wrap-shift the whole image by `(dx, dy)` (the Shift+arrows nudge / `--nudge`).
fn nudge_image(img: &mut Image, dx: i32, dy: i32) {
    let (w, h) = (img.w, img.h);
    let mut out = img.px.clone();
    for y in 0..h {
        for x in 0..w {
            let (nx, ny) = ((x + dx).rem_euclid(w), (y + dy).rem_euclid(h));
            out[(nx + ny * w) as usize] = img.px[(x + y * w) as usize];
        }
    }
    img.px = out;
}

/// Copy the `w x h` rect at `(sx, sy)` onto `(dx, dy)`, clipped to the image; all
/// pixels are copied, including transparency (paste = exact stamp). Also `--blit`.
fn blit_rect(img: &mut Image, sx: i32, sy: i32, w: i32, h: i32, dx: i32, dy: i32) {
    let mut buf = vec![[0u8; 4]; (w.max(0) * h.max(0)) as usize];
    for y in 0..h {
        for x in 0..w {
            if (0..img.w).contains(&(sx + x)) && (0..img.h).contains(&(sy + y)) {
                buf[(x + y * w) as usize] = img.px[(sx + x + (sy + y) * img.w) as usize];
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            if (0..img.w).contains(&(dx + x)) && (0..img.h).contains(&(dy + y)) {
                img.px[(dx + x + (dy + y) * img.w) as usize] = buf[(x + y * w) as usize];
            }
        }
    }
}

/// Bresenham line, inclusive.
fn line_points(x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    let mut pts = Vec::new();
    loop {
        pts.push((x, y));
        if x == x1 && y == y1 {
            return pts;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

fn rect_points(x0: i32, y0: i32, x1: i32, y1: i32, fill: bool) -> Vec<(i32, i32)> {
    let (ax, bx) = (x0.min(x1), x0.max(x1));
    let (ay, by) = (y0.min(y1), y0.max(y1));
    let mut pts = Vec::new();
    for y in ay..=by {
        for x in ax..=bx {
            if fill || x == ax || x == bx || y == ay || y == by {
                pts.push((x, y));
            }
        }
    }
    pts
}

/* --------------------------------- file browser --------------------------------- */

struct Entry {
    path: PathBuf,
    /// Display path relative to the root, e.g. `tiles/grass.png` or `tiles/`.
    rel: String,
    depth: i32,
    is_dir: bool,
}

/// Recursive, sorted walk: each directory contributes a header row, then its `*.png`
/// files, then its subdirectories. Backups (`*.bak.png`) and dotfiles are skipped.
fn walk(root: &Path) -> Vec<Entry> {
    fn rec(dir: &Path, root: &Path, depth: i32, out: &mut Vec<Entry>) {
        let rel = dir
            .strip_prefix(root)
            .ok()
            .filter(|r| !r.as_os_str().is_empty())
            .map(|r| format!("{}/", r.display()))
            .unwrap_or_else(|| {
                format!(
                    "{}/",
                    root.file_name()
                        .map(|n| n.to_string_lossy())
                        .unwrap_or_default()
                )
            });
        out.push(Entry {
            path: dir.to_path_buf(),
            rel,
            depth,
            is_dir: true,
        });
        let mut names: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|it| it.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        names.sort();
        for p in names.iter().filter(|p| p.is_file()) {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if !name.to_ascii_lowercase().ends_with(".png")
                || name.to_ascii_lowercase().ends_with(".bak.png")
                || name.starts_with('.')
            {
                continue;
            }
            out.push(Entry {
                path: p.clone(),
                rel: p
                    .strip_prefix(root)
                    .map(|r| r.display().to_string())
                    .unwrap_or(name),
                depth: depth + 1,
                is_dir: false,
            });
        }
        for p in names.iter().filter(|p| p.is_dir()) {
            let hidden = p
                .file_name()
                .map(|n| n.to_string_lossy().starts_with('.'))
                .unwrap_or(true);
            if !hidden {
                rec(p, root, depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    rec(root, root, 0, &mut out);
    out
}

/// Parse the atlas manifest's `<path> <cx> <cy> <w> <h> <pal|rgb>` lines into a
/// rel-path -> is_palette map (used for per-file wrong-mode warnings).
fn load_manifest_modes(root: &Path) -> HashMap<String, bool> {
    let mut modes = HashMap::new();
    let Ok(text) = std::fs::read_to_string(root.join("manifest.txt")) else {
        return modes;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut it = line.split_whitespace();
        let (Some(path), Some(mode)) = (it.next(), it.nth(4)) else {
            continue;
        };
        modes.insert(path.to_string(), mode == "pal");
    }
    modes
}

/* ----------------------------------- the studio ----------------------------------- */

#[derive(Clone, Copy, PartialEq)]
enum Paint {
    Erase,
    Shade(u8),
    Rgb([u8; 3]),
    Custom,
}

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Pencil,
    Line,
    Rect,
    RectFill,
}

impl Tool {
    fn label(self) -> &'static str {
        match self {
            Tool::Pencil => "PENCIL",
            Tool::Line => "LINE",
            Tool::Rect => "RECT",
            Tool::RectFill => "RECT FILL",
        }
    }
}

/// Undo record: a pixel rect (usually the edit window; the whole image for nudges)
/// plus the view to restore alongside it.
struct Snap {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    px: Vec<Rgba>,
    view: (i32, i32, i32, i32), // bx, by, view_w, view_h
}

struct Clip {
    w: i32,
    h: i32,
    px: Vec<Rgba>,
}

struct Onion {
    w: i32,
    h: i32,
    px: Vec<Rgba>,
    label: String,
}

enum Source {
    /// Monolithic atlas: the left pane is the sheet itself, browsed by cell.
    Sheet,
    /// Directory tree: the left pane is a file browser.
    Tree {
        entries: Vec<Entry>,
        sel: usize,
        scroll: i32,
    },
}

struct Studio {
    source: Source,
    path: PathBuf, // currently open PNG
    img: Image,
    manifest: HashMap<String, bool>, // rel path -> is_palette (dir mode, may be empty)
    bx: i32, // edit-window origin in image px (free; keyboard steps one 8px cell)
    by: i32,
    view_w: i32, // selected window size (Tab: 8/16; G: sprite footprint)
    view_h: i32,
    zoom_ovr: Option<i32>, // wheel-zoom override of the fit zoom
    pan: (i32, i32),       // canvas pan (zoomed px) when the window outgrows the canvas
    cur: Paint,
    prev_paint: Paint, // what `C` toggles back to
    custom: [u8; 3],
    chan: usize,
    swatches: Vec<[u8; 3]>,
    recent: VecDeque<[u8; 3]>, // last 8 painted colors
    tool: Tool,
    drag_anchor: Option<(i32, i32)>, // line/rect start (block px)
    mirror: bool,
    clipboard: Option<Clip>,
    paste_armed: bool,
    pal_idx: usize, // PREVIEW_PALS index
    anim_on: bool,
    anim_files: Vec<Image>, // dir mode: sibling frames; empty = strip flip
    anim_i: usize,
    onion_on: bool,
    onion: Option<Onion>,
    help_on: bool,
    undo: Vec<Snap>,
    dirty: bool,
    backed_up: HashSet<PathBuf>,
    status: String,
    hover: Option<(i32, i32)>,       // block-relative pixel under cursor
    sheet_hover: Option<(i32, i32)>, // sheet-pane px under cursor (sheet mode)
    esc_armed: bool,
    text: Screen, // scratch 288x192 screen to rasterize the game font
    frame: Vec<u32>,
}

impl Studio {
    fn new(source: Source, path: PathBuf, img: Image, size: i32) -> Studio {
        let mut s = Studio {
            source,
            path,
            img,
            manifest: HashMap::new(),
            bx: 0,
            by: 0,
            view_w: size,
            view_h: size,
            zoom_ovr: None,
            pan: (0, 0),
            cur: Paint::Shade(3),
            prev_paint: Paint::Shade(3),
            custom: [224, 96, 48],
            chan: 0,
            swatches: Vec::new(),
            recent: VecDeque::new(),
            tool: Tool::Pencil,
            drag_anchor: None,
            mirror: false,
            clipboard: None,
            paste_armed: false,
            pal_idx: 0,
            anim_on: false,
            anim_files: Vec::new(),
            anim_i: 0,
            onion_on: false,
            onion: None,
            help_on: false,
            undo: Vec::new(),
            dirty: false,
            backed_up: HashSet::new(),
            status: String::new(),
            hover: None,
            sheet_hover: None,
            esc_armed: false,
            text: Screen::new(Arc::new(fdoom::assets::sprite_sheet())),
            frame: vec![0; (VIEW_W * VIEW_H) as usize],
        };
        s.build_swatches();
        s
    }

    /* ------------------------------ geometry & access ------------------------------ */

    /// The edited rect: the whole image when it fits in 16x16, else the selected
    /// window clamped at the image edge (strips of any size work).
    fn block_rect(&self) -> (i32, i32, i32, i32) {
        if self.img.w <= 16 && self.img.h <= 16 {
            (0, 0, self.img.w, self.img.h)
        } else {
            let bw = self.view_w.min(self.img.w - self.bx).max(1);
            let bh = self.view_h.min(self.img.h - self.by).max(1);
            (self.bx, self.by, bw, bh)
        }
    }

    fn zoom(&self) -> i32 {
        let (_, _, bw, bh) = self.block_rect();
        let fit = (CANVAS_MAX / bw.max(bh)).clamp(1, 40);
        self.zoom_ovr.unwrap_or(fit)
    }

    fn clamp_pan(&mut self) {
        let (_, _, bw, bh) = self.block_rect();
        let z = self.zoom();
        self.pan.0 = self.pan.0.clamp(0, (bw * z - CANVAS_MAX).max(0));
        self.pan.1 = self.pan.1.clamp(0, (bh * z - CANVAS_MAX).max(0));
    }

    fn get(&self, x: i32, y: i32) -> Rgba {
        self.img.px[(x + y * self.img.w) as usize]
    }

    fn put(&mut self, x: i32, y: i32, v: Rgba) {
        let i = (x + y * self.img.w) as usize;
        if self.img.px[i] != v {
            self.img.px[i] = v;
            self.dirty = true;
            self.esc_armed = false;
        }
    }

    /// Move the window origin by whole 8px cells. Any cell is a legal origin — no
    /// even-cell snapping (graves live at cell x 15/17/19, flora at (15,26), ...).
    fn move_block(&mut self, dx: i32, dy: i32) {
        let nx = (self.bx.div_euclid(CELL) + dx) * CELL;
        let ny = (self.by.div_euclid(CELL) + dy) * CELL;
        self.set_origin(nx, ny);
    }

    fn set_origin(&mut self, nx: i32, ny: i32) {
        self.bx = nx.clamp(0, (self.img.w - CELL).max(0));
        self.by = ny.clamp(0, (self.img.h - CELL).max(0));
        self.hover = None;
        self.drag_anchor = None;
        self.clamp_pan();
    }

    fn set_view(&mut self, w: i32, h: i32) {
        self.view_w = w;
        self.view_h = h;
        self.zoom_ovr = None;
        self.pan = (0, 0);
        self.drag_anchor = None;
    }

    /// `G`: jump the window to the sprite under the sheet-pane cursor, using the
    /// sprite map's per-unit footprint (odd origins included).
    fn snap_to_sprite(&mut self) {
        let Source::Sheet = self.source else {
            self.status = "G: SHEET MODE ONLY (FILES ARE ALREADY PER-SPRITE)".into();
            return;
        };
        let Some((sx, sy)) = self.sheet_hover else {
            self.status = "G: HOVER THE SHEET PANE FIRST".into();
            return;
        };
        let (ccx, ccy) = (sx / CELL, sy / CELL);
        match sprite_at(ccx, ccy) {
            Some(&(cx, cy, _, _, uw, uh, name)) => {
                let ox = cx + ((ccx - cx) / uw) * uw;
                let oy = cy + ((ccy - cy) / uh) * uh;
                self.set_view(uw * CELL, uh * CELL);
                self.set_origin(ox * CELL, oy * CELL);
                self.status = format!("SNAP: {name}");
            }
            None => {
                self.set_view(CELL, CELL);
                self.set_origin(ccx * CELL, ccy * CELL);
                self.status = "SNAP: UNMAPPED CELL (8X8)".into();
            }
        }
    }

    /* ---------------------------------- painting ---------------------------------- */

    fn paint_rgba(&self) -> Rgba {
        match self.cur {
            Paint::Erase => [0, 0, 0, 0],
            Paint::Shade(s) => {
                let v = GRAYS[s as usize & 3];
                [v, v, v, 255]
            }
            Paint::Rgb(c) => [c[0], c[1], c[2], 255],
            Paint::Custom => [self.custom[0], self.custom[1], self.custom[2], 255],
        }
    }

    fn note_recent(&mut self) {
        let p = self.paint_rgba();
        if p[3] < 128 {
            return;
        }
        let c = [p[0], p[1], p[2]];
        self.recent.retain(|&r| r != c);
        self.recent.push_front(c);
        self.recent.truncate(8);
    }

    /// Stamp `pts` (block-relative) with the current paint; mirror-draw doubles
    /// every point across the vertical axis of the window.
    fn stamp(&mut self, pts: &[(i32, i32)]) {
        let (bx, by, bw, bh) = self.block_rect();
        let v = self.paint_rgba();
        for &(x, y) in pts {
            if (0..bw).contains(&x) && (0..bh).contains(&y) {
                self.put(bx + x, by + y, v);
                if self.mirror {
                    self.put(bx + (bw - 1 - x), by + y, v);
                }
            }
        }
        self.note_recent();
    }

    fn eyedrop(&mut self, px: i32, py: i32) {
        let (bx, by, bw, bh) = self.block_rect();
        if !(0..bw).contains(&px) || !(0..bh).contains(&py) {
            return;
        }
        let p = self.get(bx + px, by + py);
        self.cur = match classify(p) {
            Kind::Transparent => Paint::Erase,
            Kind::Gray(v) => Paint::Shade(v / 64),
            Kind::Color => Paint::Rgb([p[0], p[1], p[2]]),
        };
    }

    fn flood_fill(&mut self, px: i32, py: i32) {
        let (bx, by, bw, bh) = self.block_rect();
        if !(0..bw).contains(&px) || !(0..bh).contains(&py) {
            return;
        }
        let target = key(self.get(bx + px, by + py));
        if target == key(self.paint_rgba()) {
            return;
        }
        self.push_undo_block();
        let mut region = Vec::new();
        let mut stack = vec![(px, py)];
        let mut seen = vec![false; (bw * bh) as usize];
        while let Some((x, y)) = stack.pop() {
            if !(0..bw).contains(&x) || !(0..bh).contains(&y) || seen[(x + y * bw) as usize] {
                continue;
            }
            if key(self.get(bx + x, by + y)) != target {
                continue;
            }
            seen[(x + y * bw) as usize] = true;
            region.push((x, y));
            stack.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]);
        }
        self.stamp(&region);
    }

    fn flip(&mut self, horizontal: bool) {
        self.push_undo_block();
        let (bx, by, bw, bh) = self.block_rect();
        for y in 0..bh {
            for x in 0..bw {
                let (mx, my) = if horizontal {
                    (bw - 1 - x, y)
                } else {
                    (x, bh - 1 - y)
                };
                if (horizontal && x * 2 < bw) || (!horizontal && y * 2 < bh) {
                    let a = self.get(bx + x, by + y);
                    let b = self.get(bx + mx, by + my);
                    self.put(bx + x, by + y, b);
                    self.put(bx + mx, by + my, a);
                }
            }
        }
    }

    /// `[` / `]`: shift the hovered pixel one legal shade step — grays walk the
    /// 4-shade ladder, colors step +-16 per channel, transparent is left alone.
    fn shade_shift(&mut self, up: bool) {
        let Some((px, py)) = self.hover else {
            self.status = "SHADE SHIFT: HOVER A CANVAS PIXEL FIRST".into();
            return;
        };
        let (bx, by, bw, bh) = self.block_rect();
        if !(0..bw).contains(&px) || !(0..bh).contains(&py) {
            return;
        }
        let p = self.get(bx + px, by + py);
        let nv = match classify(p) {
            Kind::Transparent => return,
            Kind::Gray(v) => {
                let s = (v / 64) as i32 + if up { 1 } else { -1 };
                let g = GRAYS[s.clamp(0, 3) as usize];
                [g, g, g, 255]
            }
            Kind::Color => {
                let step = |c: u8| {
                    if up {
                        c.saturating_add(16)
                    } else {
                        c.saturating_sub(16)
                    }
                };
                [step(p[0]), step(p[1]), step(p[2]), 255]
            }
        };
        if nv != p {
            self.push_undo_block();
            self.put(bx + px, by + py, nv);
        }
    }

    /// Ctrl+C: copy the current window. Ctrl+V arms paste; clicking stamps it.
    fn copy_block(&mut self) {
        let (bx, by, bw, bh) = self.block_rect();
        let mut px = Vec::with_capacity((bw * bh) as usize);
        for y in 0..bh {
            for x in 0..bw {
                px.push(self.get(bx + x, by + y));
            }
        }
        self.clipboard = Some(Clip { w: bw, h: bh, px });
        self.status = format!("COPIED {bw}X{bh} (CTRL+V TO PASTE)");
    }

    fn paste_at(&mut self, px: i32, py: i32) {
        let Some(clip) = self.clipboard.take() else {
            return;
        };
        let (bx, by, _, _) = self.block_rect();
        let (dx, dy) = (bx + px, by + py);
        self.push_undo_rect(dx, dy, clip.w, clip.h);
        for y in 0..clip.h {
            for x in 0..clip.w {
                if (0..self.img.w).contains(&(dx + x)) && (0..self.img.h).contains(&(dy + y)) {
                    self.put(dx + x, dy + y, clip.px[(x + y * clip.w) as usize]);
                }
            }
        }
        self.status = format!("PASTED {}X{} AT {px},{py}", clip.w, clip.h);
        self.clipboard = Some(clip);
        self.paste_armed = false;
    }

    /// Shift+arrows: wrap-nudge the entire image.
    fn nudge(&mut self, dx: i32, dy: i32) {
        self.push_undo_rect(0, 0, self.img.w, self.img.h);
        nudge_image(&mut self.img, dx, dy);
        self.dirty = true;
        self.status = format!("NUDGED {dx},{dy} (WRAPS)");
    }

    /* ------------------------------------ undo ------------------------------------ */

    fn push_undo_rect(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let x = x.clamp(0, self.img.w);
        let y = y.clamp(0, self.img.h);
        let w = w.min(self.img.w - x).max(0);
        let h = h.min(self.img.h - y).max(0);
        let mut px = Vec::with_capacity((w * h) as usize);
        for yy in y..y + h {
            for xx in x..x + w {
                px.push(self.get(xx, yy));
            }
        }
        self.undo.push(Snap {
            x,
            y,
            w,
            h,
            px,
            view: (self.bx, self.by, self.view_w, self.view_h),
        });
        if self.undo.len() > 64 {
            self.undo.remove(0);
        }
    }

    fn push_undo_block(&mut self) {
        let (bx, by, bw, bh) = self.block_rect();
        self.push_undo_rect(bx, by, bw, bh);
    }

    fn undo_pop(&mut self) {
        let Some(s) = self.undo.pop() else {
            self.status = "NOTHING TO UNDO".into();
            return;
        };
        for y in 0..s.h {
            for x in 0..s.w {
                self.put(s.x + x, s.y + y, s.px[(x + y * s.w) as usize]);
            }
        }
        (self.bx, self.by, self.view_w, self.view_h) = s.view;
        self.clamp_pan();
        self.dirty = true;
        self.status = format!("UNDO ({} LEFT)", self.undo.len());
    }

    /* ---------------------------------- save/open ---------------------------------- */

    fn save(&mut self) {
        if !self.backed_up.contains(&self.path) {
            let bak = bak_path(&self.path);
            if let Err(e) = std::fs::copy(&self.path, &bak) {
                self.status = format!("BACKUP FAILED: {e}");
                return;
            }
            self.backed_up.insert(self.path.clone());
        }
        match write_png(&self.path, &self.img) {
            Ok(()) => {
                self.dirty = false;
                self.esc_armed = false;
                self.status = format!("SAVED {}", self.file_label());
            }
            Err(e) => self.status = format!("SAVE FAILED: {e}"),
        }
    }

    /// `X`: reload the open file from disk, dropping unsaved edits.
    fn revert(&mut self) {
        match load_png(&self.path) {
            Ok(img) => {
                self.img = img;
                self.dirty = false;
                self.undo.clear();
                self.esc_armed = false;
                self.set_origin(self.bx, self.by);
                self.status = "REVERTED FROM DISK".into();
            }
            Err(e) => self.status = format!("REVERT FAILED: {e}"),
        }
    }

    /// Dir mode: open the file entry at `idx` (blocked while dirty unless `force`).
    fn open_entry(&mut self, idx: usize, force: bool) {
        let Source::Tree { entries, sel, .. } = &mut self.source else {
            return;
        };
        if idx >= entries.len() || entries[idx].is_dir || idx == *sel {
            return;
        }
        if self.dirty && !force {
            self.status = "UNSAVED EDITS: S SAVE, X REVERT, OR SHIFT+CLICK TO DISCARD".into();
            return;
        }
        let path = entries[idx].path.clone();
        match load_png(&path) {
            Ok(img) => {
                *sel = idx;
                self.path = path;
                self.img = img;
                self.bx = 0;
                self.by = 0;
                self.zoom_ovr = None;
                self.pan = (0, 0);
                self.undo.clear();
                self.dirty = false;
                self.hover = None;
                self.drag_anchor = None;
                self.esc_armed = false;
                self.anim_on = false;
                self.anim_files.clear();
                self.status = String::new();
            }
            Err(e) => self.status = format!("OPEN FAILED: {e}"),
        }
    }

    /// Dir mode: move the file selection up/down, skipping folder headers.
    fn move_file_sel(&mut self, dir: i32) {
        let Source::Tree { entries, sel, .. } = &self.source else {
            return;
        };
        let mut i = *sel as i32;
        loop {
            i += dir;
            if i < 0 || i >= entries.len() as i32 {
                return;
            }
            if !entries[i as usize].is_dir {
                break;
            }
        }
        self.open_entry(i as usize, false);
    }

    /* --------------------------------- animation --------------------------------- */

    /// `A`: dir mode plays the sibling files of the open file's folder (walk frames
    /// as files, at the game's walk cadence); with no siblings — or in sheet mode —
    /// it flips the window between two side-by-side frames instead (2-frame flames,
    /// mob frame strips on the atlas).
    fn toggle_anim(&mut self) {
        if self.anim_on {
            self.anim_on = false;
            self.anim_files.clear();
            return;
        }
        self.anim_files.clear();
        if let Source::Tree { entries, sel, .. } = &self.source {
            let dir = entries[*sel].path.parent().map(Path::to_path_buf);
            if let Some(dir) = dir {
                let sib: Vec<&Entry> = entries
                    .iter()
                    .filter(|e| !e.is_dir && e.path.parent() == Some(dir.as_path()))
                    .collect();
                if sib.len() > 1 {
                    for e in sib {
                        if let Ok(img) = load_png(&e.path) {
                            self.anim_files.push(img);
                        }
                    }
                }
            }
        }
        let (_, _, bw, _) = self.block_rect();
        if self.anim_files.is_empty() && self.bx + bw * 2 > self.img.w {
            self.status = "ANIM: NO SIBLING FRAMES / NO ROOM TO FLIP".into();
            return;
        }
        self.anim_i = 0;
        self.anim_on = true;
        let n = if self.anim_files.is_empty() {
            2
        } else {
            self.anim_files.len()
        };
        self.status = format!("ANIM: {n} FRAMES AT WALK CADENCE");
    }

    fn anim_advance(&mut self) {
        let n = if self.anim_files.is_empty() {
            2
        } else {
            self.anim_files.len()
        };
        self.anim_i = (self.anim_i + 1) % n;
    }

    /* ----------------------------- swatches & analysis ----------------------------- */

    /// The true-color bank: the ~24 most-used saturated colors across the source
    /// (whole sheet in sheet mode; every file in the tree, capped, in dir mode).
    fn build_swatches(&mut self) {
        let mut counts: HashMap<u32, u64> = HashMap::new();
        let mut tally = |img: &Image| {
            for &p in &img.px {
                if classify(p) == Kind::Color {
                    *counts.entry(key(p)).or_insert(0) += 1;
                }
            }
        };
        tally(&self.img);
        if let Source::Tree { entries, .. } = &self.source {
            for e in entries.iter().filter(|e| !e.is_dir).take(256) {
                if e.path != self.path
                    && let Ok(img) = load_png(&e.path)
                {
                    tally(&img);
                }
            }
        }
        let mut all: Vec<(u32, u64)> = counts.into_iter().collect();
        all.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        all.truncate(24);
        self.swatches = all
            .iter()
            .map(|&(c, _)| [(c >> 16) as u8, (c >> 8) as u8, c as u8])
            .collect();
    }

    /// Wrong-mode warning. With a manifest entry the file's declared mode is the
    /// contract: a `pal` file must contain only ladder grays, an `rgb` file must
    /// never contain palette grays. Without one, warn when a single 8x8 cell of the
    /// window mixes grays with colors.
    fn art_warning(&self) -> Option<String> {
        if let Source::Tree { entries, sel, .. } = &self.source
            && let Some(&is_pal) = self.manifest.get(&entries[*sel].rel)
        {
            let (mut color_px, mut gray_px, mut off_ladder) = (false, false, false);
            for &p in &self.img.px {
                match classify(p) {
                    Kind::Color => color_px = true,
                    Kind::Gray(v) => {
                        gray_px = true;
                        off_ladder |= !GRAYS.contains(&v);
                    }
                    Kind::Transparent => {}
                }
            }
            return if is_pal && color_px {
                Some("! PAL FILE CONTAINS COLOR PIXELS".into())
            } else if is_pal && off_ladder {
                Some("! PAL FILE HAS OFF-LADDER GRAYS".into())
            } else if !is_pal && gray_px {
                Some("! RGB FILE CONTAINS GRAY (PAL) PIXELS".into())
            } else {
                None
            };
        }
        if self.block_mixes_modes() {
            Some("! GRAY + COLOR MIXED IN CELL".into())
        } else {
            None
        }
    }

    /// True when any single 8x8 cell of the window mixes palette grays with
    /// saturated colors — usually a mistake.
    fn block_mixes_modes(&self) -> bool {
        let (bx, by, bw, bh) = self.block_rect();
        for cy in (0..bh).step_by(CELL as usize) {
            for cx in (0..bw).step_by(CELL as usize) {
                let (mut gray, mut colored) = (false, false);
                for y in cy..(cy + CELL).min(bh) {
                    for x in cx..(cx + CELL).min(bw) {
                        match classify(self.get(bx + x, by + y)) {
                            Kind::Gray(_) => gray = true,
                            Kind::Color => colored = true,
                            Kind::Transparent => {}
                        }
                    }
                }
                if gray && colored {
                    return true;
                }
            }
        }
        false
    }

    /* ------------------------------ palette preview ------------------------------ */

    /// How a sheet pixel displays under the active preview palette: palette grays go
    /// through the packed `get4` word exactly like `Screen::render` (byte 255 =
    /// transparent, else `color::upgrade`); true colors and RAW mode pass through.
    fn shown(&self, p: Rgba) -> Option<u32> {
        match classify(p) {
            Kind::Transparent => None,
            Kind::Color => Some(rgb24(p)),
            Kind::Gray(v) => {
                if self.pal_idx == 0 {
                    return Some(rgb24(p));
                }
                let (_, pal) = PREVIEW_PALS[self.pal_idx];
                let shade = (v / 64) as i32;
                let byte = (pal >> ((3 - shade) * 8)) & 0xFF;
                if byte >= 255 {
                    None
                } else {
                    Some(color::upgrade(byte) as u32)
                }
            }
        }
    }

    fn shown_img(&self, img: &Image, x: i32, y: i32) -> Option<u32> {
        self.shown(img.px[(x + y * img.w) as usize])
    }

    /* ---------------------------------- labels ---------------------------------- */

    fn file_label(&self) -> String {
        match &self.source {
            Source::Sheet => self
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            Source::Tree { entries, sel, .. } => entries[*sel].rel.clone(),
        }
    }

    /// Sheet mode: the sprite-map name for the window origin (region as fallback).
    /// Dir mode: the folder plus the manifest-declared pal/rgb mode.
    fn sprite_label(&self) -> String {
        match &self.source {
            Source::Sheet if self.img.w == 256 => {
                let (ccx, ccy) = (self.bx / CELL, self.by / CELL);
                match sprite_at(ccx, ccy) {
                    Some(&(.., name)) => name.to_string(),
                    None => artgen_region(ccy).to_string(),
                }
            }
            Source::Sheet => "SHEET".into(),
            Source::Tree { entries, sel, .. } => {
                let rel = &entries[*sel].rel;
                let folder = match rel.rfind('/') {
                    Some(i) => rel[..i + 1].to_uppercase(),
                    None => "/".into(),
                };
                match self.manifest.get(rel) {
                    Some(true) => format!("{folder} PAL"),
                    Some(false) => format!("{folder} RGB"),
                    None => folder,
                }
            }
        }
    }

    fn paint_desc(&self) -> String {
        match self.cur {
            Paint::Erase => "ERASE (TRANSPARENT)".into(),
            Paint::Shade(s) => format!("SHADE {} (GRAY {})", s, GRAYS[s as usize & 3]),
            Paint::Rgb(c) => format!("RGB {:02X}{:02X}{:02X}", c[0], c[1], c[2]),
            Paint::Custom => {
                let c = self.custom;
                let gray = if c[0] == c[1] && c[1] == c[2] {
                    " = GRAY!"
                } else {
                    ""
                };
                format!(
                    "CUSTOM {}R {} {}G {} {}B {}{gray}",
                    mark(self.chan == 0),
                    c[0],
                    mark(self.chan == 1),
                    c[1],
                    mark(self.chan == 2),
                    c[2],
                )
            }
        }
    }

    fn title(&self) -> String {
        let (bx, by, bw, bh) = self.block_rect();
        format!(
            "pixel studio — {}{} | {}x{} at ({}, {}) cell ({}, {}) | {}",
            self.file_label(),
            if self.dirty { " *" } else { "" },
            bw,
            bh,
            bx,
            by,
            bx / CELL,
            by / CELL,
            self.sprite_label()
        )
    }

    /* ---------------------------------- rendering ---------------------------------- */

    fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, col: u32) {
        self.fill_clip(x, y, w, h, col, (0, 0, VIEW_W, VIEW_H));
    }

    fn fill_clip(&mut self, x: i32, y: i32, w: i32, h: i32, col: u32, clip: (i32, i32, i32, i32)) {
        let (cx, cy, cw, ch) = clip;
        let (x0, y0) = (x.max(cx).max(0), y.max(cy).max(0));
        let x1 = (x + w).min(cx + cw).min(VIEW_W);
        let y1 = (y + h).min(cy + ch).min(VIEW_H);
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.frame[(xx + yy * VIEW_W) as usize] = col;
            }
        }
    }

    fn outline(&mut self, x: i32, y: i32, w: i32, h: i32, col: u32) {
        self.fill_rect(x, y, w, 1, col);
        self.fill_rect(x, y + h - 1, w, 1, col);
        self.fill_rect(x, y, 1, h, col);
        self.fill_rect(x + w - 1, y, 1, h, col);
    }

    /// Rasterize `s` with the game font into the frame (worldview's trick: draw on
    /// a scratch 288-wide screen in 32-char chunks, then copy lit pixels).
    fn draw_text(&mut self, x: i32, y: i32, s: &str, readable: i32) {
        let col = color::get4(-1, readable, readable, readable);
        let chars: Vec<char> = s.chars().collect();
        for (ci, chunk) in chars.chunks(32).enumerate() {
            let part: String = chunk.iter().collect();
            self.text.clear(0);
            font::draw(&part, &mut self.text, 0, 0, col);
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

    fn render(&mut self) {
        self.frame.fill(BG);
        self.draw_header();
        match &self.source {
            Source::Sheet => self.draw_sheet_pane(),
            Source::Tree { .. } => self.draw_tree_pane(),
        }
        self.draw_legend();
        self.draw_canvas();
        self.draw_palette();
        self.draw_preview();
        if self.help_on {
            self.draw_help();
        }
    }

    fn draw_header(&mut self) {
        let (bx, by, bw, bh) = self.block_rect();
        let line1 = format!(
            "PIXEL STUDIO  {}  {}X{} AT {},{} CELL {},{}  {}",
            self.file_label(),
            bw,
            bh,
            bx,
            by,
            bx / CELL,
            by / CELL,
            self.sprite_label()
        );
        self.draw_text(PANE_X, 8, &line1, TXT);
        if self.dirty {
            let x = PANE_X + 8 * (line1.chars().count() as i32 + 1);
            self.draw_text(x.min(VIEW_W - 152), 8, "*UNSAVED", TXT_WARN);
        }
        let hover = match self.hover {
            Some((px, py)) if px < bw && py < bh => {
                let p = self.get(bx + px, by + py);
                match classify(p) {
                    Kind::Transparent => format!("  PX {px},{py} TRANSPARENT"),
                    Kind::Gray(v) => format!("  PX {px},{py} SHADE {} (GRAY {v})", v / 64),
                    Kind::Color => {
                        format!("  PX {px},{py} {:02X}{:02X}{:02X}", p[0], p[1], p[2])
                    }
                }
            }
            _ => String::new(),
        };
        let mut flags = String::new();
        if self.tool != Tool::Pencil {
            flags += &format!("  [{}]", self.tool.label());
        }
        if self.mirror {
            flags += "  [MIRROR]";
        }
        if self.paste_armed {
            flags += "  [PASTE: CLICK TO PLACE]";
        }
        if self.onion_on {
            flags += "  [ONION]";
        }
        let line2 = format!("PAINT: {}{hover}{flags}", self.paint_desc());
        self.draw_text(PANE_X, 20, &line2, TXT);
        let status = std::mem::take(&mut self.status);
        let status_col = if self.esc_armed { TXT_WARN } else { TXT_DIM };
        self.draw_text(PANE_X, 32, &status, status_col);
        self.status = status;
        if let Some(w) = self.art_warning() {
            self.draw_text(RX + 100, 32, &w, TXT_WARN);
        }
        self.draw_text(VIEW_W - 72, 8, "? HELP", TXT_DIM);
        self.fill_rect(0, 46, VIEW_W, 1, GRID);
    }

    /// Sheet mode left pane: the whole sheet at 2x, window outlined.
    fn draw_sheet_pane(&mut self) {
        self.fill_rect(PANE_X - 1, PANE_Y - 1, PANE_W + 2, PANE_H + 2, GRID);
        let view = PANE_W / 2; // sheet px shown per axis
        let off_x = clamp_scroll(self.bx, self.view_w, self.img.w, view);
        let off_y = clamp_scroll(self.by, self.view_h, self.img.h, view);
        let vw = self.img.w.min(view);
        let vh = self.img.h.min(view);
        for sy in 0..vh {
            for sx in 0..vw {
                let p = self.get(off_x + sx, off_y + sy);
                let col = match self.shown(p) {
                    Some(c) => c,
                    None => checker(sx / 4, sy / 4),
                };
                self.fill_rect(PANE_X + sx * 2, PANE_Y + sy * 2, 2, 2, col);
            }
        }
        let (bx, by, bw, bh) = self.block_rect();
        self.outline(
            PANE_X + (bx - off_x) * 2 - 1,
            PANE_Y + (by - off_y) * 2 - 1,
            bw * 2 + 2,
            bh * 2 + 2,
            ACCENT,
        );
    }

    /// Dir mode left pane: the recursive file list with folder headers.
    fn draw_tree_pane(&mut self) {
        let Source::Tree {
            entries,
            sel,
            scroll,
        } = &mut self.source
        else {
            return;
        };
        let rows_fit = PANE_H / ROW_H;
        let sel_row = *sel as i32;
        if sel_row < *scroll {
            *scroll = sel_row;
        }
        if sel_row >= *scroll + rows_fit {
            *scroll = sel_row - rows_fit + 1;
        }
        let scroll = *scroll;
        let sel = *sel;
        // borrow dance: collect the visible rows, then draw
        let rows: Vec<(String, i32, bool, bool)> = entries
            .iter()
            .enumerate()
            .skip(scroll.max(0) as usize)
            .take(rows_fit as usize)
            .map(|(i, e)| (e.rel.clone(), e.depth, e.is_dir, i == sel))
            .collect();
        for (i, (rel, depth, is_dir, selected)) in rows.into_iter().enumerate() {
            let y = PANE_Y + i as i32 * ROW_H;
            let x = PANE_X + depth * 12;
            if selected {
                self.fill_rect(PANE_X - 2, y - 2, PANE_W, ROW_H, 0x2A3340);
            }
            let name = if is_dir {
                rel
            } else {
                rel.rsplit('/').next().unwrap_or(&rel).to_string()
            };
            let col = if selected {
                TXT
            } else if is_dir {
                TXT_DIM
            } else {
                444
            };
            self.draw_text(x, y, &name, col);
            if selected {
                self.draw_text(PANE_X, y, ">", TXT_WARN);
            }
        }
    }

    fn draw_legend(&mut self) {
        let lines: [&str; 5] = match self.source {
            Source::Sheet => [
                "SHEET: CLICK/ARROWS MOVE - G SNAP TO SPRITE - TAB 8/16",
                "CANVAS: L-PAINT R-PICK F FILL L/R TOOLS H/V FLIP",
                "WHEEL ZOOM - MID-DRAG PAN - P PALETTE - A ANIM",
                "U UNDO - S SAVE - B/O ONION - CTRL+C/V COPY/PASTE",
                "? FULL KEY LIST - ESC QUIT",
            ],
            Source::Tree { .. } => [
                "FILES: CLICK OR UP/DOWN - SHIFT+CLICK DISCARDS",
                "CANVAS: L-PAINT R-PICK F FILL L/R TOOLS H/V FLIP",
                "WHEEL ZOOM - MID-DRAG PAN - P PALETTE - A ANIM",
                "U UNDO - S SAVE - B/O ONION - CTRL+C/V COPY/PASTE",
                "? FULL KEY LIST - ESC QUIT",
            ],
        };
        for (i, l) in lines.iter().enumerate() {
            self.draw_text(PANE_X, PANE_Y + PANE_H + 8 + i as i32 * 12, l, TXT_DIM);
        }
    }

    fn draw_canvas(&mut self) {
        let (bx, by, bw, bh) = self.block_rect();
        let z = self.zoom();
        let (pan_x, pan_y) = self.pan;
        let clip = (RX, CANVAS_Y, CANVAS_MAX, CANVAS_MAX);
        let vis_w = (bw * z - pan_x).min(CANVAS_MAX);
        let vis_h = (bh * z - pan_y).min(CANVAS_MAX);
        self.fill_rect(RX - 1, CANVAS_Y - 1, vis_w + 2, vis_h + 2, GRID_MAJOR);

        // shape preview (line/rect drag in progress)
        let ghost_pts: Vec<(i32, i32)> = match (self.drag_anchor, self.hover) {
            (Some((ax, ay)), Some((hx, hy))) => match self.tool {
                Tool::Line => line_points(ax, ay, hx, hy),
                Tool::Rect => rect_points(ax, ay, hx, hy, false),
                Tool::RectFill => rect_points(ax, ay, hx, hy, true),
                Tool::Pencil => Vec::new(),
            },
            _ => Vec::new(),
        };

        let onion = if self.onion_on {
            self.onion.take()
        } else {
            None
        };
        for y in 0..bh {
            for x in 0..bw {
                let sx = RX + x * z - pan_x;
                let sy = CANVAS_Y + y * z - pan_y;
                if sx + z <= RX
                    || sy + z <= CANVAS_Y
                    || sx >= RX + CANVAS_MAX
                    || sy >= CANVAS_Y + CANVAS_MAX
                {
                    continue;
                }
                let p = self.get(bx + x, by + y);
                let mut col = match self.shown(p) {
                    Some(c) => c,
                    None => {
                        // onion skin: ghost the reference at ~30% under transparency
                        let base = checker(x, y);
                        match &onion {
                            Some(o) if x < o.w && y < o.h => {
                                let rp = o.px[(x + y * o.w) as usize];
                                match self.shown(rp) {
                                    Some(rc) => blend(rc, base, 77), // ~30%
                                    None => base,
                                }
                            }
                            _ => base,
                        }
                    }
                };
                if ghost_pts.contains(&(x, y)) {
                    col = blend(ACCENT, col, 130);
                }
                self.fill_clip(sx, sy, z, z, col, clip);
            }
        }
        if self.onion.is_none() {
            self.onion = onion;
        }

        // paste ghost follows the cursor
        if self.paste_armed
            && let (Some(clipb), Some((hx, hy))) = (&self.clipboard, self.hover)
        {
            let mut ghost = Vec::new();
            for y in 0..clipb.h {
                for x in 0..clipb.w {
                    ghost.push((hx + x, hy + y, clipb.px[(x + y * clipb.w) as usize]));
                }
            }
            for (x, y, p) in ghost {
                if x < bw && y < bh {
                    let sx = RX + x * z - pan_x;
                    let sy = CANVAS_Y + y * z - pan_y;
                    let under = checker(x, y);
                    let col = blend(self.shown(p).unwrap_or(under), under, 150);
                    self.fill_clip(sx, sy, z, z, col, clip);
                }
            }
        }

        // pixel grid; a brighter line on 8px cell boundaries
        if z >= 4 {
            for x in 1..bw {
                let col = if x % CELL == 0 { GRID_MAJOR } else { GRID };
                self.fill_clip(RX + x * z - pan_x, CANVAS_Y, 1, vis_h, col, clip);
            }
            for y in 1..bh {
                let col = if y % CELL == 0 { GRID_MAJOR } else { GRID };
                self.fill_clip(RX, CANVAS_Y + y * z - pan_y, vis_w, 1, col, clip);
            }
        }
        // mirror axis
        if self.mirror {
            let mx = RX + (bw * z) / 2 - pan_x;
            self.fill_clip(mx, CANVAS_Y, 1, vis_h, 0xE06060, clip);
        }
        if let Some((px, py)) = self.hover
            && px < bw
            && py < bh
        {
            self.outline(
                RX + px * z - pan_x,
                CANVAS_Y + py * z - pan_y,
                z + 1,
                z + 1,
                ACCENT,
            );
        }
    }

    fn draw_palette(&mut self) {
        // bank A: the four legal palette shades + transparent
        self.draw_text(RX, PAL_A_Y + 6, "SHADES", TXT_DIM);
        for (i, g) in GRAYS.iter().enumerate() {
            let x = SWATCH_X + i as i32 * 26;
            let col = rgb24([*g, *g, *g, 255]);
            self.fill_rect(x, PAL_A_Y, 20, 20, col);
            self.outline(x - 1, PAL_A_Y - 1, 22, 22, GRID_MAJOR);
            if self.cur == Paint::Shade(i as u8) {
                self.outline(x - 2, PAL_A_Y - 2, 24, 24, ACCENT);
            }
        }
        let tx = SWATCH_X + 4 * 26;
        for yy in 0..5 {
            for xx in 0..5 {
                self.fill_rect(tx + xx * 4, PAL_A_Y + yy * 4, 4, 4, checker(xx, yy));
            }
        }
        self.outline(tx - 1, PAL_A_Y - 1, 22, 22, GRID_MAJOR);
        if self.cur == Paint::Erase {
            self.outline(tx - 2, PAL_A_Y - 2, 24, 24, ACCENT);
        }
        self.draw_text(tx + 26, PAL_A_Y + 6, "0-3 + T", TXT_DIM);

        // bank B: sampled true colors, 2 rows of 12, plus the custom swatch
        self.draw_text(RX, PAL_B_Y + 8, "COLORS", TXT_DIM);
        for i in 0..self.swatches.len().min(24) {
            let c = self.swatches[i];
            let (row, coln) = (i as i32 / 12, i as i32 % 12);
            let (x, y) = (SWATCH_X + coln * 17, PAL_B_Y + row * 17);
            self.fill_rect(x, y, 14, 14, rgb24([c[0], c[1], c[2], 255]));
            if self.cur == Paint::Rgb(c) {
                self.outline(x - 1, y - 1, 16, 16, ACCENT);
            }
        }
        let cx = SWATCH_X + 12 * 17 + 10;
        let c = self.custom;
        self.fill_rect(cx, PAL_B_Y, 31, 31, rgb24([c[0], c[1], c[2], 255]));
        self.outline(cx - 1, PAL_B_Y - 1, 33, 33, GRID_MAJOR);
        if self.cur == Paint::Custom {
            self.outline(cx - 2, PAL_B_Y - 2, 35, 35, ACCENT);
        }

        // recent colors: the last 8 painted
        self.draw_text(RX, RECENT_Y + 3, "RECENT", TXT_DIM);
        let recent: Vec<[u8; 3]> = self.recent.iter().copied().collect();
        for (i, c) in recent.iter().enumerate() {
            let x = SWATCH_X + i as i32 * 17;
            self.fill_rect(x, RECENT_Y, 14, 14, rgb24([c[0], c[1], c[2], 255]));
            self.outline(x - 1, RECENT_Y - 1, 16, 16, GRID_MAJOR);
        }

        let rgb_line = format!(
            "CUSTOM (C): {}R {:3} {}G {:3} {}B {:3}  ARROWS ADJUST",
            mark(self.chan == 0),
            c[0],
            mark(self.chan == 1),
            c[1],
            mark(self.chan == 2),
            c[2]
        );
        let col_txt = if self.cur == Paint::Custom {
            TXT
        } else {
            TXT_DIM
        };
        self.draw_text(RX, RGB_Y, &rgb_line, col_txt);
    }

    /// Backdrop texel for the in-context previews.
    fn backdrop(kind: usize, x: i32, y: i32) -> u32 {
        let speck = ((x * 7 + y * 13 + (x * y) % 5) % 7) == 0;
        match kind {
            0 => {
                if speck {
                    0x4EA341
                } else {
                    0x3F8C33
                }
            }
            1 => {
                if speck {
                    0xC9B569
                } else {
                    0xD9C77A
                }
            }
            _ => night(if speck { 0x4EA341 } else { 0x3F8C33 }),
        }
    }

    fn draw_preview(&mut self) {
        let (bx, by, bw, bh) = self.block_rect();
        let (pal_name, _) = PREVIEW_PALS[self.pal_idx];
        let label = format!(
            "PREVIEW (P: {pal_name})  1X 2X 4X | GRASS SAND NIGHT | {}TILED",
            if self.anim_on { "ANIM | " } else { "" }
        );
        self.draw_text(RX, PREVIEW_Y - 12, &label, TXT_DIM);

        let mut x = RX;
        // raw scales
        for scale in [1, 2, 4] {
            self.fill_rect(x - 1, PREVIEW_Y - 1, bw * scale + 2, bh * scale + 2, PANEL);
            for y in 0..bh {
                for px in 0..bw {
                    if let Some(c) = self.shown(self.get(bx + px, by + y)) {
                        self.fill_rect(x + px * scale, PREVIEW_Y + y * scale, scale, scale, c);
                    }
                }
            }
            x += bw * scale + 8;
        }
        x += 4;
        // in-context: grass day, sand, night-graded grass at 2x with a 4px apron
        for kind in 0..3usize {
            let (w2, h2) = (bw * 2 + 8, bh * 2 + 8);
            for yy in 0..h2 {
                for xx in 0..w2 {
                    self.fill_rect(x + xx, PREVIEW_Y + yy, 1, 1, Self::backdrop(kind, xx, yy));
                }
            }
            for y in 0..bh {
                for px in 0..bw {
                    if let Some(mut c) = self.shown(self.get(bx + px, by + y)) {
                        if kind == 2 {
                            c = night(c);
                        }
                        self.fill_rect(x + 4 + px * 2, PREVIEW_Y + 4 + y * 2, 2, 2, c);
                    }
                }
            }
            x += w2 + 8;
        }
        // animation frame at 2x over grass
        if self.anim_on {
            let (fw, fh) = if self.anim_files.is_empty() {
                (bw, bh)
            } else {
                let f = &self.anim_files[self.anim_i.min(self.anim_files.len() - 1)];
                (f.w.min(24), f.h.min(24))
            };
            let (w2, h2) = (fw * 2 + 8, fh * 2 + 8);
            for yy in 0..h2 {
                for xx in 0..w2 {
                    self.fill_rect(x + xx, PREVIEW_Y + yy, 1, 1, Self::backdrop(0, xx, yy));
                }
            }
            if self.anim_files.is_empty() {
                // strip flip: window at bx vs bx + bw
                let fx = bx + (self.anim_i as i32) * bw;
                for y in 0..bh {
                    for px in 0..bw {
                        if fx + px < self.img.w
                            && let Some(c) = self.shown(self.get(fx + px, by + y))
                        {
                            self.fill_rect(x + 4 + px * 2, PREVIEW_Y + 4 + y * 2, 2, 2, c);
                        }
                    }
                }
            } else {
                let mut texels = Vec::new();
                {
                    let f = &self.anim_files[self.anim_i.min(self.anim_files.len() - 1)];
                    for y in 0..fh {
                        for px in 0..fw {
                            texels.push((px, y, self.shown_img(f, px, y)));
                        }
                    }
                }
                for (px, y, c) in texels {
                    if let Some(c) = c {
                        self.fill_rect(x + 4 + px * 2, PREVIEW_Y + 4 + y * 2, 2, 2, c);
                    }
                }
            }
            x += w2 + 8;
        }
        // tiled 3x3 at 2x: judge seamless tiling (16px windows only, space allowing)
        if bw <= 16 && bh <= 16 && x + bw * 6 + 2 <= VIEW_W - 4 {
            self.fill_rect(x - 1, PREVIEW_Y - 1, bw * 6 + 2, bh * 6 + 2, PANEL);
            for ty in 0..3 {
                for tx in 0..3 {
                    for y in 0..bh {
                        for px in 0..bw {
                            if let Some(c) = self.shown(self.get(bx + px, by + y)) {
                                self.fill_rect(
                                    x + (tx * bw + px) * 2,
                                    PREVIEW_Y + (ty * bh + y) * 2,
                                    2,
                                    2,
                                    c,
                                );
                            }
                        }
                    }
                }
            }
        }
        // onion reference note
        if let Some(o) = &self.onion {
            let s = format!(
                "ONION REF: {} ({})",
                o.label,
                if self.onion_on { "ON" } else { "OFF - O" }
            );
            self.draw_text(RX, VIEW_H - 16, &s, TXT_DIM);
        }
    }

    fn draw_help(&mut self) {
        let (x, y, w, h) = (120, 80, VIEW_W - 240, VIEW_H - 160);
        self.fill_rect(x - 2, y - 2, w + 4, h + 4, GRID_MAJOR);
        self.fill_rect(x, y, w, h, PANEL);
        self.draw_text(x + 16, y + 10, "PIXEL STUDIO KEYS", TXT);
        let col1 = [
            "L-CLICK/DRAG   PAINT",
            "R-CLICK        EYEDROP",
            "F              FLOOD FILL",
            "L              LINE TOOL",
            "R / SHIFT+R    RECT / FILLED",
            "M              MIRROR-DRAW",
            "BRACKET KEYS   SHADE SHIFT",
            "H / V          FLIP WINDOW",
            "CTRL+C         COPY WINDOW",
            "CTRL+V         PASTE (CLICK)",
            "SHIFT+ARROWS   NUDGE (WRAPS)",
            "U / CTRL+Z     UNDO",
            "E              ERASER",
            "C              CUSTOM COLOR",
        ];
        let col2 = [
            "ARROWS         MOVE CELL/FILE",
            "I / K          STEP VERTICALLY",
            "TAB            8/16 WINDOW",
            "G              SNAP TO SPRITE",
            "WHEEL          ZOOM AT CURSOR",
            "MIDDLE-DRAG    PAN",
            "P / SHIFT+P    PREVIEW PALETTE",
            "A              ANIMATE FRAMES",
            "B              SET ONION REF",
            "O              ONION ON/OFF",
            "S / CTRL+S     SAVE (+.BAK)",
            "X              REVERT FROM DISK",
            "SHIFT+CLICK    DISCARD + OPEN",
            "ESC            CLOSE / QUIT",
        ];
        for (i, l) in col1.iter().enumerate() {
            self.draw_text(x + 16, y + 30 + i as i32 * 12, l, TXT_DIM);
        }
        for (i, l) in col2.iter().enumerate() {
            self.draw_text(x + w / 2 + 8, y + 30 + i as i32 * 12, l, TXT_DIM);
        }
        self.draw_text(
            x + 16,
            y + h - 20,
            "PAL GRAYS 0/85/170/255 ONLY - NEVER MIX PAL + RGB IN A FILE",
            TXT_WARN,
        );
    }
}

fn mark(active: bool) -> &'static str {
    if active { ">" } else { " " }
}

/// Sheet-pane scroll for sheets larger than the 256px view: keep the window visible.
fn clamp_scroll(sel_px: i32, block: i32, dim: i32, view: i32) -> i32 {
    if dim <= view {
        0
    } else {
        (sel_px + block / 2 - view / 2).clamp(0, dim - view)
    }
}

/* ---------------------------------- window shell ---------------------------------- */

enum Hit {
    SheetPane(i32, i32), // sheet px
    TreeRow(usize),      // entry index
    Canvas(i32, i32),    // block-relative px
    ShadeSwatch(usize),  // 0-3, 4 = transparent
    ColorSwatch(usize),
    RecentSwatch(usize),
    CustomSwatch,
    None,
}

struct App {
    st: Studio,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    needs_render: bool,
    mods: ModifiersState,
    left_down: bool,
    mid_drag: Option<(i32, i32)>, // last cursor frame pos while middle-panning
    mid_acc: (i32, i32),
    cursor: Option<(i32, i32)>, // frame coords
    anim_next: Option<Instant>,
}

impl App {
    fn refresh(&mut self) {
        self.needs_render = true;
        if let Some(w) = &self.window {
            w.set_title(&self.st.title());
            w.request_redraw();
        }
    }

    fn hit(&self, fx: i32, fy: i32) -> Hit {
        let st = &self.st;
        let (_, _, bw, bh) = st.block_rect();
        let z = st.zoom();
        if (RX..RX + CANVAS_MAX).contains(&fx) && (CANVAS_Y..CANVAS_Y + CANVAS_MAX).contains(&fy) {
            let px = (fx - RX + st.pan.0) / z;
            let py = (fy - CANVAS_Y + st.pan.1) / z;
            if px < bw && py < bh {
                return Hit::Canvas(px, py);
            }
        }
        if (PAL_A_Y - 2..PAL_A_Y + 22).contains(&fy) && fx >= SWATCH_X {
            let i = (fx - SWATCH_X) / 26;
            if (0..=4).contains(&i) && (fx - SWATCH_X) % 26 < 22 {
                return Hit::ShadeSwatch(i as usize);
            }
        }
        if (PAL_B_Y - 1..PAL_B_Y + 34).contains(&fy) && fx >= SWATCH_X {
            let cx = SWATCH_X + 12 * 17 + 10;
            if fx >= cx - 2 && fx < cx + 33 {
                return Hit::CustomSwatch;
            }
            if fy < PAL_B_Y + 31 {
                let (coln, row) = ((fx - SWATCH_X) / 17, (fy - PAL_B_Y) / 17);
                let i = (row * 12 + coln) as usize;
                if coln < 12 && row < 2 && i < st.swatches.len() {
                    return Hit::ColorSwatch(i);
                }
            }
        }
        if (RECENT_Y - 1..RECENT_Y + 16).contains(&fy) && fx >= SWATCH_X {
            let i = (fx - SWATCH_X) / 17;
            if i >= 0 && (i as usize) < st.recent.len() && (fx - SWATCH_X) % 17 < 15 {
                return Hit::RecentSwatch(i as usize);
            }
        }
        if (PANE_X..PANE_X + PANE_W).contains(&fx) && (PANE_Y..PANE_Y + PANE_H).contains(&fy) {
            match &st.source {
                Source::Sheet => {
                    let view = PANE_W / 2;
                    let off_x = clamp_scroll(st.bx, st.view_w, st.img.w, view);
                    let off_y = clamp_scroll(st.by, st.view_h, st.img.h, view);
                    let (sx, sy) = (off_x + (fx - PANE_X) / 2, off_y + (fy - PANE_Y) / 2);
                    if sx < st.img.w && sy < st.img.h {
                        return Hit::SheetPane(sx, sy);
                    }
                }
                Source::Tree {
                    entries, scroll, ..
                } => {
                    let row = *scroll + (fy - PANE_Y) / ROW_H;
                    if row >= 0 && (row as usize) < entries.len() {
                        return Hit::TreeRow(row as usize);
                    }
                }
            }
        }
        Hit::None
    }

    fn on_mouse_press(&mut self, button: MouseButton) {
        let Some((fx, fy)) = self.cursor else { return };
        if button == MouseButton::Middle {
            self.mid_drag = Some((fx, fy));
            self.mid_acc = (0, 0);
            return;
        }
        match (self.hit(fx, fy), button) {
            (Hit::Canvas(px, py), MouseButton::Left) => {
                if self.st.paste_armed {
                    self.st.paste_at(px, py);
                } else {
                    match self.st.tool {
                        Tool::Pencil => {
                            self.st.push_undo_block();
                            self.st.stamp(&[(px, py)]);
                            self.left_down = true;
                        }
                        _ => self.st.drag_anchor = Some((px, py)),
                    }
                }
            }
            (Hit::Canvas(px, py), MouseButton::Right) => self.st.eyedrop(px, py),
            (Hit::SheetPane(sx, sy), MouseButton::Left) => {
                self.st.set_origin(sx - sx % CELL, sy - sy % CELL);
            }
            (Hit::TreeRow(i), MouseButton::Left) => {
                self.st.open_entry(i, self.mods.shift_key());
            }
            (Hit::ShadeSwatch(4), MouseButton::Left) => self.st.cur = Paint::Erase,
            (Hit::ShadeSwatch(i), MouseButton::Left) => self.st.cur = Paint::Shade(i as u8),
            (Hit::ColorSwatch(i), MouseButton::Left) => {
                self.st.cur = Paint::Rgb(self.st.swatches[i]);
            }
            (Hit::RecentSwatch(i), MouseButton::Left) => {
                let c = self.st.recent[i];
                self.st.cur = if c[0] == c[1] && c[1] == c[2] {
                    Paint::Shade(c[0] / 64)
                } else {
                    Paint::Rgb(c)
                };
            }
            (Hit::CustomSwatch, MouseButton::Left) => self.st.cur = Paint::Custom,
            _ => return,
        }
        self.refresh();
    }

    fn on_mouse_release(&mut self, button: MouseButton) {
        match button {
            MouseButton::Middle => self.mid_drag = None,
            MouseButton::Left => {
                self.left_down = false;
                if let (Some((ax, ay)), Some((hx, hy))) = (self.st.drag_anchor, self.st.hover) {
                    let pts = match self.st.tool {
                        Tool::Line => line_points(ax, ay, hx, hy),
                        Tool::Rect => rect_points(ax, ay, hx, hy, false),
                        Tool::RectFill => rect_points(ax, ay, hx, hy, true),
                        Tool::Pencil => Vec::new(),
                    };
                    if !pts.is_empty() {
                        self.st.push_undo_block();
                        self.st.stamp(&pts);
                    }
                }
                self.st.drag_anchor = None;
                self.refresh();
            }
            _ => {}
        }
    }

    fn on_cursor(&mut self, fx: i32, fy: i32) {
        self.cursor = Some((fx, fy));
        // middle-drag: pan the window origin across the image (free, per-pixel)
        if let Some((lx, ly)) = self.mid_drag {
            let z = self.st.zoom().max(1);
            self.mid_acc.0 += lx - fx;
            self.mid_acc.1 += ly - fy;
            self.mid_drag = Some((fx, fy));
            let (dx, dy) = (self.mid_acc.0 / z, self.mid_acc.1 / z);
            if dx != 0 || dy != 0 {
                self.mid_acc.0 -= dx * z;
                self.mid_acc.1 -= dy * z;
                self.st.set_origin(self.st.bx + dx, self.st.by + dy);
                self.refresh();
            }
            return;
        }
        let hover = match self.hit(fx, fy) {
            Hit::Canvas(px, py) => Some((px, py)),
            _ => None,
        };
        self.st.sheet_hover = match self.hit(fx, fy) {
            Hit::SheetPane(sx, sy) => Some((sx, sy)),
            _ => None,
        };
        if self.left_down
            && self.st.tool == Tool::Pencil
            && let Some((px, py)) = hover
        {
            self.st.stamp(&[(px, py)]);
        }
        let changed = hover != self.st.hover;
        self.st.hover = hover;
        if changed || self.left_down || self.st.drag_anchor.is_some() || self.st.paste_armed {
            self.refresh();
        }
    }

    /// Mouse wheel: zoom the canvas around the hovered pixel.
    fn on_wheel(&mut self, up: bool) {
        let Some((fx, fy)) = self.cursor else { return };
        if !(RX..RX + CANVAS_MAX).contains(&fx) || !(CANVAS_Y..CANVAS_Y + CANVAS_MAX).contains(&fy)
        {
            return;
        }
        let z = self.st.zoom();
        let nz = if up {
            (z * 5 / 4 + 1).min(48)
        } else {
            (z * 4 / 5).max(2)
        };
        if nz == z {
            return;
        }
        // keep the pixel under the cursor stationary
        let (ppx, ppy) = (
            (fx - RX + self.st.pan.0) / z,
            (fy - CANVAS_Y + self.st.pan.1) / z,
        );
        self.st.zoom_ovr = Some(nz);
        self.st.pan.0 = ppx * nz - (fx - RX);
        self.st.pan.1 = ppy * nz - (fy - CANVAS_Y);
        self.st.clamp_pan();
        self.refresh();
    }

    fn on_key(&mut self, code: KeyCode) {
        let st = &mut self.st;
        let shift = self.mods.shift_key();
        let ctrl = self.mods.control_key() || self.mods.super_key();
        let custom = st.cur == Paint::Custom;
        match code {
            // arrows: RGB stepper while the custom swatch is active
            KeyCode::ArrowLeft if custom => st.chan = (st.chan + 2) % 3,
            KeyCode::ArrowRight if custom => st.chan = (st.chan + 1) % 3,
            KeyCode::ArrowUp if custom => {
                let step = if shift { 1 } else { 8 };
                st.custom[st.chan] = st.custom[st.chan].saturating_add(step);
            }
            KeyCode::ArrowDown if custom => {
                let step = if shift { 1 } else { 8 };
                st.custom[st.chan] = st.custom[st.chan].saturating_sub(step);
            }
            // Shift+arrows: wrap-nudge the whole image
            KeyCode::ArrowLeft if shift => st.nudge(-1, 0),
            KeyCode::ArrowRight if shift => st.nudge(1, 0),
            KeyCode::ArrowUp if shift => st.nudge(0, -1),
            KeyCode::ArrowDown if shift => st.nudge(0, 1),
            // arrows: navigate (files in dir mode, cells in sheet mode)
            KeyCode::ArrowUp => match st.source {
                Source::Tree { .. } => st.move_file_sel(-1),
                Source::Sheet => st.move_block(0, -1),
            },
            KeyCode::ArrowDown => match st.source {
                Source::Tree { .. } => st.move_file_sel(1),
                Source::Sheet => st.move_block(0, 1),
            },
            KeyCode::ArrowLeft => st.move_block(-1, 0),
            KeyCode::ArrowRight => st.move_block(1, 0),
            // vertical window stepping inside tall images (dir-mode strips)
            KeyCode::KeyI => st.move_block(0, -1),
            KeyCode::KeyK => st.move_block(0, 1),
            KeyCode::Tab => {
                let s = if st.view_w == 16 && st.view_h == 16 {
                    8
                } else {
                    16
                };
                st.set_view(s, s);
            }
            KeyCode::KeyG => st.snap_to_sprite(),
            KeyCode::KeyE => st.cur = Paint::Erase,
            KeyCode::KeyC if ctrl => st.copy_block(),
            KeyCode::KeyV if ctrl => {
                if st.clipboard.is_some() {
                    st.paste_armed = !st.paste_armed;
                    st.status = if st.paste_armed {
                        "PASTE: CLICK THE CANVAS TO PLACE".into()
                    } else {
                        String::new()
                    };
                } else {
                    st.status = "PASTE: NOTHING COPIED YET (CTRL+C)".into();
                }
            }
            KeyCode::KeyC => {
                if custom {
                    st.cur = st.prev_paint;
                } else {
                    st.prev_paint = st.cur;
                    st.cur = Paint::Custom;
                }
            }
            KeyCode::KeyF => {
                if let Some((px, py)) = st.hover {
                    st.flood_fill(px, py);
                } else {
                    st.status = "FILL: HOVER A CANVAS PIXEL FIRST".into();
                }
            }
            KeyCode::KeyL => {
                st.tool = if st.tool == Tool::Line {
                    Tool::Pencil
                } else {
                    Tool::Line
                };
            }
            KeyCode::KeyR if shift => {
                st.tool = if st.tool == Tool::RectFill {
                    Tool::Pencil
                } else {
                    Tool::RectFill
                };
            }
            KeyCode::KeyR => {
                st.tool = if st.tool == Tool::Rect {
                    Tool::Pencil
                } else {
                    Tool::Rect
                };
            }
            KeyCode::KeyM => st.mirror = !st.mirror,
            KeyCode::BracketLeft => st.shade_shift(false),
            KeyCode::BracketRight => st.shade_shift(true),
            KeyCode::KeyH => st.flip(true),
            KeyCode::KeyV => st.flip(false),
            KeyCode::KeyU => st.undo_pop(),
            KeyCode::KeyZ if ctrl => st.undo_pop(),
            KeyCode::KeyS => st.save(), // plain S and Ctrl+S both save
            KeyCode::KeyX => st.revert(),
            KeyCode::KeyP if shift => {
                st.pal_idx = (st.pal_idx + PREVIEW_PALS.len() - 1) % PREVIEW_PALS.len();
            }
            KeyCode::KeyP => st.pal_idx = (st.pal_idx + 1) % PREVIEW_PALS.len(),
            KeyCode::KeyA => st.toggle_anim(),
            KeyCode::KeyB => {
                let (bx, by, bw, bh) = st.block_rect();
                let mut px = Vec::with_capacity((bw * bh) as usize);
                for y in 0..bh {
                    for x in 0..bw {
                        px.push(st.get(bx + x, by + y));
                    }
                }
                st.onion = Some(Onion {
                    w: bw,
                    h: bh,
                    px,
                    label: st.file_label(),
                });
                st.onion_on = true;
                st.status = "ONION REFERENCE CAPTURED (O TOGGLES)".into();
            }
            KeyCode::KeyO => {
                st.onion_on = !st.onion_on;
                if st.onion.is_none() {
                    st.status = "ONION: CAPTURE A REFERENCE FIRST (B)".into();
                    st.onion_on = false;
                }
            }
            KeyCode::Slash if shift => st.help_on = !st.help_on,
            _ => return,
        }
        self.refresh();
    }

    /// Window coords -> internal frame coords (inverse of the `redraw` blit).
    fn to_frame(&self, px: f64, py: f64) -> Option<(i32, i32)> {
        let window = self.window.as_ref()?;
        let size = window.inner_size();
        let (win_w, win_h) = (size.width as i32, size.height as i32);
        let scale = (win_w as f32 / VIEW_W as f32).min(win_h as f32 / VIEW_H as f32);
        if scale <= 0.0 {
            return None;
        }
        let xo = (win_w - (VIEW_W as f32 * scale) as i32) / 2;
        let yo = (win_h - (VIEW_H as f32 * scale) as i32) / 2;
        let fx = ((px as f32 - xo as f32) / scale) as i32;
        let fy = ((py as f32 - yo as f32) / scale) as i32;
        ((0..VIEW_W).contains(&fx) && (0..VIEW_H).contains(&fy)).then_some((fx, fy))
    }

    /// Scaled nearest-neighbour blit, centered — same approach as worldview.
    fn redraw(&mut self) {
        if self.needs_render {
            self.st.render();
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
                buffer[dest_row + (dx + xo) as usize] = self.st.frame[src_row + sx as usize];
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
            .with_title(self.st.title())
            .with_inner_size(LogicalSize::new(VIEW_W as f64, VIEW_H as f64))
            .with_min_inner_size(LogicalSize::new(480.0, 360.0));
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
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                if let Some((fx, fy)) = self.to_frame(position.x, position.y) {
                    self.on_cursor(fx, fy);
                } else {
                    self.cursor = None;
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let up = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y > 0.0,
                    MouseScrollDelta::PixelDelta(p) => p.y > 0.0,
                };
                self.on_wheel(up);
            }
            WindowEvent::MouseInput { state, button, .. } => match state {
                ElementState::Pressed => self.on_mouse_press(button),
                ElementState::Released => self.on_mouse_release(button),
            },
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed
                    && let PhysicalKey::Code(code) = event.physical_key
                {
                    if code == KeyCode::Escape {
                        if self.st.help_on {
                            self.st.help_on = false;
                        } else if self.st.paste_armed {
                            self.st.paste_armed = false;
                            self.st.status.clear();
                        } else if self.st.dirty && !self.st.esc_armed {
                            self.st.esc_armed = true;
                            self.st.status =
                                "UNSAVED EDITS: ESC AGAIN TO QUIT, S TO SAVE, X TO REVERT".into();
                        } else {
                            event_loop.exit();
                            return;
                        }
                        self.refresh();
                        return;
                    }
                    self.on_key(code);
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.st.anim_on {
            let now = Instant::now();
            let next = *self.anim_next.get_or_insert(now);
            if now >= next {
                self.st.anim_advance();
                self.anim_next = Some(now + ANIM_FRAME);
                self.needs_render = true;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.anim_next.unwrap()));
        } else {
            self.anim_next = None;
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

/* --------------------------------------- main --------------------------------------- */

fn parse_set_color(s: &str) -> Option<Rgba> {
    if s.eq_ignore_ascii_case("t") || s.eq_ignore_ascii_case("transparent") {
        return Some([0, 0, 0, 0]);
    }
    let s = s.trim_start_matches('#');
    match s.len() {
        6 => u32::from_str_radix(s, 16)
            .ok()
            .map(|v| [(v >> 16) as u8, (v >> 8) as u8, v as u8, 255]),
        8 => u32::from_str_radix(s, 16)
            .ok()
            .map(|v| [(v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8]),
        _ => None,
    }
}

fn usage() -> ! {
    eprintln!(
        "usage: pixel_studio [<dir> | <sheet.png>] [--sheet <png>] [--cell X Y] [--size 8|16] [--pal N]\n       \
         pixel_studio <png> [--set X Y (RRGGBB[AA]|t)]... [--blit SX SY W H DX DY]... [--nudge DX DY]\n       \
         pixel_studio <dir> --file <rel.png> [--set ...]...   # headless edits resolve via the tree walk\n       \
         pixel_studio <target> --shot <out.png>               # render one UI frame headlessly and exit\n\n\
         default target: assets/sprites (directory) if it exists, else assets/golden_atlas.png"
    );
    std::process::exit(2);
}

fn arg_i32(args: &[String], i: usize) -> i32 {
    args.get(i)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| usage())
}

enum BatchOp {
    Set(i32, i32, Rgba),
    Blit(i32, i32, i32, i32, i32, i32),
    Nudge(i32, i32),
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut target: Option<PathBuf> = None;
    let mut force_sheet = false;
    let mut cell: Option<(i32, i32)> = None;
    let mut size = 16i32;
    let mut pal = 0usize;
    let mut file_rel: Option<String> = None;
    let mut shot: Option<PathBuf> = None;
    let mut batch: Vec<BatchOp> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--sheet" => {
                target = Some(PathBuf::from(args.get(i + 1).unwrap_or_else(|| usage())));
                force_sheet = true;
                i += 1;
            }
            "--cell" => {
                cell = Some((arg_i32(&args, i + 1), arg_i32(&args, i + 2)));
                i += 2;
            }
            "--size" => {
                size = arg_i32(&args, i + 1);
                i += 1;
            }
            "--pal" => {
                pal = arg_i32(&args, i + 1).clamp(0, PREVIEW_PALS.len() as i32 - 1) as usize;
                i += 1;
            }
            "--file" => {
                file_rel = Some(args.get(i + 1).cloned().unwrap_or_else(|| usage()));
                i += 1;
            }
            "--shot" => {
                shot = Some(PathBuf::from(args.get(i + 1).unwrap_or_else(|| usage())));
                i += 1;
            }
            "--set" => {
                let (x, y) = (arg_i32(&args, i + 1), arg_i32(&args, i + 2));
                let c = args
                    .get(i + 3)
                    .and_then(|s| parse_set_color(s))
                    .unwrap_or_else(|| usage());
                batch.push(BatchOp::Set(x, y, c));
                i += 3;
            }
            "--blit" => {
                batch.push(BatchOp::Blit(
                    arg_i32(&args, i + 1),
                    arg_i32(&args, i + 2),
                    arg_i32(&args, i + 3),
                    arg_i32(&args, i + 4),
                    arg_i32(&args, i + 5),
                    arg_i32(&args, i + 6),
                ));
                i += 6;
            }
            "--nudge" => {
                batch.push(BatchOp::Nudge(arg_i32(&args, i + 1), arg_i32(&args, i + 2)));
                i += 2;
            }
            s if !s.starts_with('-') && target.is_none() => target = Some(PathBuf::from(s)),
            _ => usage(),
        }
        i += 1;
    }
    if ![8, 16].contains(&size) {
        eprintln!("--size must be 8 or 16");
        std::process::exit(2);
    }

    let target = target.unwrap_or_else(|| {
        let dir = PathBuf::from("assets/sprites");
        if dir.is_dir() {
            dir
        } else {
            PathBuf::from("assets/golden_atlas.png")
        }
    });
    let dir_mode = target.is_dir() && !force_sheet;

    // resolve the PNG to open (and, in dir mode, the walked entry list)
    let (entries, open_idx, path) = if dir_mode {
        let entries = walk(&target);
        let idx = match &file_rel {
            Some(rel) => entries
                .iter()
                .position(|e| !e.is_dir && e.rel == *rel)
                .unwrap_or_else(|| {
                    eprintln!("--file {rel}: not found under {}", target.display());
                    std::process::exit(2);
                }),
            None => match entries.iter().position(|e| !e.is_dir) {
                Some(i) => i,
                None => {
                    eprintln!("no *.png files under {}", target.display());
                    std::process::exit(2);
                }
            },
        };
        let path = entries[idx].path.clone();
        (Some(entries), idx, path)
    } else {
        if file_rel.is_some() {
            eprintln!("--file only applies to directory mode");
            std::process::exit(2);
        }
        (None, 0, target.clone())
    };

    let mut img = match load_png(&path) {
        Ok(img) => img,
        Err(e) => {
            eprintln!("pixel_studio: {e}");
            std::process::exit(1);
        }
    };

    // headless batch mode: apply edits in argument order, back up, save, exit
    if !batch.is_empty() {
        for op in &batch {
            match *op {
                BatchOp::Set(x, y, c) => {
                    if !(0..img.w).contains(&x) || !(0..img.h).contains(&y) {
                        eprintln!("--set {x} {y}: out of bounds ({}x{})", img.w, img.h);
                        std::process::exit(2);
                    }
                    img.px[(x + y * img.w) as usize] = c;
                }
                BatchOp::Blit(sx, sy, w, h, dx, dy) => blit_rect(&mut img, sx, sy, w, h, dx, dy),
                BatchOp::Nudge(dx, dy) => nudge_image(&mut img, dx, dy),
            }
        }
        let bak = bak_path(&path);
        if let Err(e) = std::fs::copy(&path, &bak) {
            eprintln!("backup failed: {e}");
            std::process::exit(1);
        }
        if let Err(e) = write_png(&path, &img) {
            eprintln!("save failed: {e}");
            std::process::exit(1);
        }
        println!(
            "wrote {} ({} op{}), backup at {}",
            path.display(),
            batch.len(),
            if batch.len() == 1 { "" } else { "s" },
            bak.display()
        );
        return;
    }

    let manifest = if dir_mode {
        load_manifest_modes(&target)
    } else {
        HashMap::new()
    };
    let source = match entries {
        Some(entries) => Source::Tree {
            entries,
            sel: open_idx,
            scroll: 0,
        },
        None => Source::Sheet,
    };
    let mut st = Studio::new(source, path, img, size);
    st.manifest = manifest;
    st.pal_idx = pal;
    if let Some((cx, cy)) = cell {
        // any 8px cell is a legal origin — no even-cell snapping
        st.set_origin(cx * CELL, cy * CELL);
    }

    // headless UI screenshot: render one frame, write it, print the title, exit
    if let Some(out) = shot {
        st.render();
        let img = Image {
            w: VIEW_W,
            h: VIEW_H,
            px: st
                .frame
                .iter()
                .map(|&p| [(p >> 16) as u8, (p >> 8) as u8, p as u8, 255])
                .collect(),
        };
        if let Err(e) = write_png(&out, &img) {
            eprintln!("shot failed: {e}");
            std::process::exit(1);
        }
        println!("{}", st.title());
        println!("wrote {}", out.display());
        return;
    }

    println!("{}", st.title());
    println!("controls: press ? in-app for the full key list");

    let event_loop = EventLoop::new().expect("could not create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        st,
        window: None,
        surface: None,
        needs_render: true,
        mods: ModifiersState::empty(),
        left_down: false,
        mid_drag: None,
        mid_acc: (0, 0),
        cursor: None,
        anim_next: None,
    };
    event_loop.run_app(&mut app).expect("event loop error");
}
