//! pixel_studio — a standalone pixel-art studio for the game's sprite art.
//!
//! Two sources, one editor:
//!
//! - **Directory mode** (primary, for the split-sheet era): point it at a folder
//!   (default `assets/sprites` when it exists) and the left pane becomes a file
//!   browser over every `*.png` under it. Opening a file sizes the editor to the
//!   image itself — 8x8 items, 16x16 tiles/mob frames, and bigger strips are edited
//!   one 8/16px block at a time.
//! - **Sheet mode** (fallback, for the legacy monolithic `assets/sprites.png` and for
//!   inspecting any stitched atlas): the left pane shows the whole sheet at 2x and
//!   you pick 8x8 / 16x16 cells; rows carry the legacy sheet's region labels.
//!
//! ```sh
//! cargo run --bin pixel_studio                                  # assets/sprites[.png]
//! cargo run --bin pixel_studio -- assets/sprites.png --cell 4 10 --size 16
//! cargo run --bin pixel_studio -- --sheet target/atlas.png
//! cargo run --bin pixel_studio -- <png> --set X Y RRGGBB        # headless batch edit
//! cargo run --bin pixel_studio -- <dir> --file tiles/grass.png --set X Y t
//! ```
//!
//! Controls (also listed under the left pane in-app):
//! - Left pane: click a file/cell; Up/Down browse files (dir mode) or move the cell
//!   (sheet mode); Shift+move discards unsaved edits when switching files.
//! - Canvas: left-click/drag paints, right-click eyedrops, `F` flood-fills at the
//!   cursor, `H`/`V` flip the block, `E` selects the eraser (transparent),
//!   `U`/Ctrl+Z undo (64 levels), Tab toggles 8/16 block stepping.
//! - Palette: click a swatch. `C` toggles the custom swatch; while it is active the
//!   arrows become an RGB stepper (Left/Right pick the channel, Up/Down step it,
//!   Shift for +-1). In dir mode `I`/`K` step the block vertically inside strips.
//! - `S` saves in place (first save of a session copies `<name>.bak.png` alongside).
//!   Esc quits (asks twice if dirty).
//!
//! Pixel semantics mirror `src/gfx/sprite_sheet.rs`: opaque grays (`r==g==b`) are
//! palette pixels recolored at draw time (legal shades are exactly 0/85/170/255),
//! any saturated color draws literally, alpha < 128 is transparent. The studio warns
//! when a single 8x8 cell mixes palette grays with saturated colors — that is almost
//! always a mistake (the palette half will recolor, the rest will not).
//!
//! The window shell mirrors `worldview` (winit 0.30 + softbuffer, scaled blit); the
//! UI is drawn rects + the game font. No `Game`, no new dependencies.

use std::collections::{HashMap, HashSet, VecDeque};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Window, WindowId};

use fdoom::gfx::{Screen, SpriteSheet, color, font};

/* ------------------------------------ layout ------------------------------------ */

const VIEW_W: i32 = 960;
const VIEW_H: i32 = 656;

/// Sprite cell edge (must match `sprite_sheet::BOX_WIDTH`).
const CELL: i32 = 8;
/// The only legal palette-mode grays (artgen G0..G3; the loader quantizes `r/64`).
const GRAYS: [u8; 4] = [0, 85, 170, 255];

const PANE_X: i32 = 8;
const PANE_Y: i32 = 56;
const PANE_W: i32 = 512; // sheet browser: 256 sheet px at 2x; dir mode: file list
const PANE_H: i32 = 512;
const ROW_H: i32 = 12; // file-list line height

const RX: i32 = 536; // right pane origin
const CANVAS_Y: i32 = 56;
const CANVAS_MAX: i32 = 384;
const PAL_A_Y: i32 = 452;
const PAL_B_Y: i32 = 482;
const RGB_Y: i32 = 520;
const PREVIEW_Y: i32 = 544;
const SWATCH_X: i32 = RX + 88;

const BG: u32 = 0x14181C;
const PANEL: u32 = 0x0C0F13;
const GRID: u32 = 0x262C34;
const GRID_MAJOR: u32 = 0x3E4854;
const ACCENT: u32 = 0xFFD24A;
const TXT: i32 = 555; // readable-color text values for draw_text
const TXT_DIM: i32 = 333;
const TXT_WARN: i32 = 540;

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

/// Flood-fill / equality key: everything transparent is one bucket, opaque by rgb.
fn key(p: Rgba) -> u32 {
    if p[3] < 128 {
        u32::MAX
    } else {
        ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32
    }
}

fn checker(x: i32, y: i32) -> u32 {
    if (x + y) % 2 == 0 { 0x30363E } else { 0x22272E }
}

/// Region map of the legacy 256x256 monolithic sheet (row = 8px cell row), so the
/// sheet-mode browser can label what a cell belongs to. Shown for 256px-wide sheets
/// only; dir-mode files are labeled by their folder instead.
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

/* ----------------------------------- the studio ----------------------------------- */

#[derive(Clone, Copy, PartialEq)]
enum Paint {
    Erase,
    Shade(u8),
    Rgb([u8; 3]),
    Custom,
}

struct Snap {
    bx: i32,
    by: i32,
    bw: i32,
    bh: i32,
    px: Vec<Rgba>,
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
    bx: i32, // edited block origin (image px)
    by: i32,
    block: i32, // stepping size: 8 or 16
    cur: Paint,
    prev_paint: Paint, // what `C` toggles back to
    custom: [u8; 3],
    chan: usize,
    swatches: Vec<[u8; 3]>,
    undo: VecDeque<Snap>,
    dirty: bool,
    backed_up: HashSet<PathBuf>,
    status: String,
    hover: Option<(i32, i32)>, // block-relative pixel under the cursor
    esc_armed: bool,
    text: Screen, // scratch 288x192 screen to rasterize the game font
    frame: Vec<u32>,
}

impl Studio {
    fn new(source: Source, path: PathBuf, img: Image, block: i32) -> Studio {
        let mut s = Studio {
            source,
            path,
            img,
            bx: 0,
            by: 0,
            block,
            cur: Paint::Shade(3),
            prev_paint: Paint::Shade(3),
            custom: [224, 96, 48],
            chan: 0,
            swatches: Vec::new(),
            undo: VecDeque::new(),
            dirty: false,
            backed_up: HashSet::new(),
            status: String::new(),
            hover: None,
            esc_armed: false,
            text: Screen::new(Arc::new(SpriteSheet::from_png(fdoom::assets::SPRITES_PNG))),
            frame: vec![0; (VIEW_W * VIEW_H) as usize],
        };
        s.build_swatches();
        s
    }

    /* ------------------------------ geometry & access ------------------------------ */

    /// The edited rect: the whole image when it fits in 16x16, else a stepped block
    /// (clamped at the image edge, so strips of any size work).
    fn block_rect(&self) -> (i32, i32, i32, i32) {
        if self.img.w <= 16 && self.img.h <= 16 {
            (0, 0, self.img.w, self.img.h)
        } else {
            let bw = self.block.min(self.img.w - self.bx).max(1);
            let bh = self.block.min(self.img.h - self.by).max(1);
            (self.bx, self.by, bw, bh)
        }
    }

    fn zoom(&self) -> i32 {
        let (_, _, bw, bh) = self.block_rect();
        (CANVAS_MAX / bw.max(bh)).clamp(1, 40)
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

    fn move_block(&mut self, dx: i32, dy: i32) {
        let step = self.block;
        let nx = (self.bx / step + dx) * step;
        let ny = (self.by / step + dy) * step;
        if nx >= 0 && nx < self.img.w {
            self.bx = nx;
        }
        if ny >= 0 && ny < self.img.h {
            self.by = ny;
        }
        self.hover = None;
    }

    fn set_block_size(&mut self, size: i32) {
        self.block = size;
        self.bx -= self.bx % size;
        self.by -= self.by % size;
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

    fn paint_at(&mut self, px: i32, py: i32) {
        let (bx, by, bw, bh) = self.block_rect();
        if (0..bw).contains(&px) && (0..bh).contains(&py) {
            let v = self.paint_rgba();
            self.put(bx + px, by + py, v);
        }
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
        let v = self.paint_rgba();
        if target == key(v) {
            return;
        }
        self.push_undo();
        let mut stack = vec![(px, py)];
        while let Some((x, y)) = stack.pop() {
            if !(0..bw).contains(&x) || !(0..bh).contains(&y) {
                continue;
            }
            if key(self.get(bx + x, by + y)) != target {
                continue;
            }
            self.put(bx + x, by + y, v);
            stack.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]);
        }
    }

    fn flip(&mut self, horizontal: bool) {
        self.push_undo();
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

    /* ------------------------------------ undo ------------------------------------ */

    fn snapshot(&self) -> Snap {
        let (bx, by, bw, bh) = self.block_rect();
        let mut px = Vec::with_capacity((bw * bh) as usize);
        for y in 0..bh {
            for x in 0..bw {
                px.push(self.get(bx + x, by + y));
            }
        }
        Snap { bx, by, bw, bh, px }
    }

    fn push_undo(&mut self) {
        let snap = self.snapshot();
        self.undo.push_back(snap);
        if self.undo.len() > 64 {
            self.undo.pop_front();
        }
    }

    fn undo_pop(&mut self) {
        let Some(s) = self.undo.pop_back() else {
            self.status = "NOTHING TO UNDO".into();
            return;
        };
        self.bx = s.bx;
        self.by = s.by;
        for y in 0..s.bh {
            for x in 0..s.bw {
                self.put(s.bx + x, s.by + y, s.px[(x + y * s.bw) as usize]);
            }
        }
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

    /// Dir mode: open the file entry at `idx` (blocked while dirty unless `force`).
    fn open_entry(&mut self, idx: usize, force: bool) {
        let Source::Tree { entries, sel, .. } = &mut self.source else {
            return;
        };
        if idx >= entries.len() || entries[idx].is_dir || idx == *sel {
            return;
        }
        if self.dirty && !force {
            self.status = "UNSAVED EDITS: S TO SAVE, SHIFT+MOVE TO DISCARD".into();
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
                self.undo.clear();
                self.dirty = false;
                self.hover = None;
                self.esc_armed = false;
                self.status = String::new();
            }
            Err(e) => self.status = format!("OPEN FAILED: {e}"),
        }
    }

    /// Dir mode: move the file selection up/down, skipping folder headers.
    fn move_file_sel(&mut self, dir: i32, force: bool) {
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
        self.open_entry(i as usize, force);
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

    /// True when any single 8x8 cell of the block mixes palette grays with saturated
    /// colors — usually a mistake (the gray half recolors at draw time, the rest not).
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

    fn region_label(&self) -> String {
        match &self.source {
            Source::Sheet if self.img.w == 256 => artgen_region(self.by / CELL).to_string(),
            Source::Sheet => "SHEET".into(),
            Source::Tree { entries, sel, .. } => {
                let rel = &entries[*sel].rel;
                match rel.rfind('/') {
                    Some(i) => rel[..i + 1].to_uppercase(),
                    None => "/".into(),
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
            "pixel studio — {}{} | {}x{} at ({}, {}) | {}",
            self.file_label(),
            if self.dirty { " *" } else { "" },
            bw,
            bh,
            bx,
            by,
            self.region_label()
        )
    }

    /* ---------------------------------- rendering ---------------------------------- */

    fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, col: u32) {
        let (x0, y0) = (x.max(0), y.max(0));
        let (x1, y1) = ((x + w).min(VIEW_W), (y + h).min(VIEW_H));
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

    /// Rasterize `s` with the game font into the frame (same trick as worldview:
    /// draw on a scratch 288-wide screen in 32-char chunks, then copy lit pixels).
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
    }

    fn draw_header(&mut self) {
        let (bx, by, bw, bh) = self.block_rect();
        let line1 = format!(
            "PIXEL STUDIO  {}{}  {}X{} AT {},{}  {}",
            self.file_label(),
            if self.dirty { "*" } else { "" },
            bw,
            bh,
            bx,
            by,
            self.region_label()
        );
        self.draw_text(PANE_X, 8, &line1, TXT);
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
        let line2 = format!("PAINT: {}{hover}", self.paint_desc());
        self.draw_text(PANE_X, 20, &line2, TXT);
        let status = std::mem::take(&mut self.status);
        self.draw_text(PANE_X, 32, &status, TXT_DIM);
        self.status = status;
        if self.block_mixes_modes() {
            self.draw_text(RX + 100, 32, "! GRAY + COLOR MIXED IN CELL", TXT_WARN);
        }
        self.fill_rect(0, 46, VIEW_W, 1, GRID);
    }

    /// Sheet mode left pane: the whole sheet at 2x, selected block outlined.
    fn draw_sheet_pane(&mut self) {
        self.fill_rect(PANE_X - 1, PANE_Y - 1, PANE_W + 2, PANE_H + 2, GRID);
        let view = PANE_W / 2; // sheet px shown per axis
        let off_x = clamp_scroll(self.bx, self.block, self.img.w, view);
        let off_y = clamp_scroll(self.by, self.block, self.img.h, view);
        let vw = self.img.w.min(view);
        let vh = self.img.h.min(view);
        for sy in 0..vh {
            for sx in 0..vw {
                let p = self.get(off_x + sx, off_y + sy);
                let col = if p[3] < 128 {
                    checker(sx / 4, sy / 4)
                } else {
                    ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32
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
                "SHEET: CLICK OR ARROWS SELECT CELL - TAB 8/16",
                "CANVAS: L-PAINT R-PICK F FILL H/V FLIP E ERASE",
                "U/CTRL+Z UNDO - S SAVE (.BAK.PNG FIRST TIME)",
                "C CUSTOM COLOR: ARROWS = L/R CHANNEL, U/D VALUE",
                "ESC QUIT",
            ],
            Source::Tree { .. } => [
                "FILES: CLICK OR UP/DOWN - SHIFT+MOVE DISCARDS",
                "BLOCK: L/R ARROWS + I/K STEP IN STRIPS - TAB 8/16",
                "CANVAS: L-PAINT R-PICK F FILL H/V FLIP E ERASE",
                "U/CTRL+Z UNDO - S SAVE - C CUSTOM (ARROWS ADJUST)",
                "ESC QUIT",
            ],
        };
        for (i, l) in lines.iter().enumerate() {
            self.draw_text(PANE_X, PANE_Y + PANE_H + 8 + i as i32 * 12, l, TXT_DIM);
        }
    }

    fn draw_canvas(&mut self) {
        let (bx, by, bw, bh) = self.block_rect();
        let z = self.zoom();
        self.fill_rect(RX - 1, CANVAS_Y - 1, bw * z + 2, bh * z + 2, GRID_MAJOR);
        for y in 0..bh {
            for x in 0..bw {
                let p = self.get(bx + x, by + y);
                let col = if p[3] < 128 {
                    checker(x, y)
                } else {
                    ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32
                };
                self.fill_rect(RX + x * z, CANVAS_Y + y * z, z, z, col);
            }
        }
        // pixel grid; a brighter line on 8px cell boundaries
        for x in 1..bw {
            let col = if x % CELL == 0 { GRID_MAJOR } else { GRID };
            self.fill_rect(RX + x * z, CANVAS_Y, 1, bh * z, col);
        }
        for y in 1..bh {
            let col = if y % CELL == 0 { GRID_MAJOR } else { GRID };
            self.fill_rect(RX, CANVAS_Y + y * z, bw * z, 1, col);
        }
        if let Some((px, py)) = self.hover
            && px < bw
            && py < bh
        {
            self.outline(RX + px * z, CANVAS_Y + py * z, z + 1, z + 1, ACCENT);
        }
    }

    fn draw_palette(&mut self) {
        // bank A: the four legal palette shades + transparent
        self.draw_text(RX, PAL_A_Y + 6, "SHADES", TXT_DIM);
        for (i, g) in GRAYS.iter().enumerate() {
            let x = SWATCH_X + i as i32 * 26;
            let col = ((*g as u32) << 16) | ((*g as u32) << 8) | *g as u32;
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
            let col = ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32;
            self.fill_rect(x, y, 14, 14, col);
            if self.cur == Paint::Rgb(c) {
                self.outline(x - 1, y - 1, 16, 16, ACCENT);
            }
        }
        let cx = SWATCH_X + 12 * 17 + 10;
        let c = self.custom;
        let col = ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32;
        self.fill_rect(cx, PAL_B_Y, 31, 31, col);
        self.outline(cx - 1, PAL_B_Y - 1, 33, 33, GRID_MAJOR);
        if self.cur == Paint::Custom {
            self.outline(cx - 2, PAL_B_Y - 2, 35, 35, ACCENT);
        }
        let c = self.custom;
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

    fn draw_preview(&mut self) {
        let (bx, by, bw, bh) = self.block_rect();
        self.draw_text(
            RX,
            PREVIEW_Y - 12,
            "PREVIEW 1X 2X 4X + TILED 3X3 (2X)",
            TXT_DIM,
        );
        let mut x = RX;
        for scale in [1, 2, 4] {
            self.fill_rect(x - 1, PREVIEW_Y - 1, bw * scale + 2, bh * scale + 2, PANEL);
            for y in 0..bh {
                for px in 0..bw {
                    let p = self.get(bx + px, by + y);
                    if p[3] >= 128 {
                        let col = ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32;
                        self.fill_rect(x + px * scale, PREVIEW_Y + y * scale, scale, scale, col);
                    }
                }
            }
            x += bw * scale + 10;
        }
        // tiled 3x3 at 2x: judge seamless tiling
        self.fill_rect(x - 1, PREVIEW_Y - 1, bw * 6 + 2, bh * 6 + 2, PANEL);
        for ty in 0..3 {
            for tx in 0..3 {
                for y in 0..bh {
                    for px in 0..bw {
                        let p = self.get(bx + px, by + y);
                        if p[3] >= 128 {
                            let col = ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32;
                            self.fill_rect(
                                x + (tx * bw + px) * 2,
                                PREVIEW_Y + (ty * bh + y) * 2,
                                2,
                                2,
                                col,
                            );
                        }
                    }
                }
            }
        }
    }
}

fn mark(active: bool) -> &'static str {
    if active { ">" } else { " " }
}

/// Sheet-pane scroll for sheets larger than the 256px view: keep the block visible.
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
    cursor: Option<(i32, i32)>, // frame coords
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
        if fx >= RX && fy >= CANVAS_Y && fx < RX + bw * z && fy < CANVAS_Y + bh * z {
            return Hit::Canvas((fx - RX) / z, (fy - CANVAS_Y) / z);
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
        if (PANE_X..PANE_X + PANE_W).contains(&fx) && (PANE_Y..PANE_Y + PANE_H).contains(&fy) {
            match &st.source {
                Source::Sheet => {
                    let view = PANE_W / 2;
                    let off_x = clamp_scroll(st.bx, st.block, st.img.w, view);
                    let off_y = clamp_scroll(st.by, st.block, st.img.h, view);
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
        match (self.hit(fx, fy), button) {
            (Hit::Canvas(px, py), MouseButton::Left) => {
                self.st.push_undo();
                self.st.paint_at(px, py);
                self.left_down = true;
            }
            (Hit::Canvas(px, py), MouseButton::Right) => self.st.eyedrop(px, py),
            (Hit::SheetPane(sx, sy), MouseButton::Left) => {
                self.st.bx = sx - sx % self.st.block;
                self.st.by = sy - sy % self.st.block;
                self.st.hover = None;
            }
            (Hit::TreeRow(i), MouseButton::Left) => {
                self.st.open_entry(i, self.mods.shift_key());
            }
            (Hit::ShadeSwatch(4), MouseButton::Left) => self.st.cur = Paint::Erase,
            (Hit::ShadeSwatch(i), MouseButton::Left) => self.st.cur = Paint::Shade(i as u8),
            (Hit::ColorSwatch(i), MouseButton::Left) => {
                self.st.cur = Paint::Rgb(self.st.swatches[i]);
            }
            (Hit::CustomSwatch, MouseButton::Left) => self.st.cur = Paint::Custom,
            _ => return,
        }
        self.refresh();
    }

    fn on_cursor(&mut self, fx: i32, fy: i32) {
        self.cursor = Some((fx, fy));
        let hover = match self.hit(fx, fy) {
            Hit::Canvas(px, py) => Some((px, py)),
            _ => None,
        };
        if self.left_down
            && let Some((px, py)) = hover
        {
            self.st.paint_at(px, py);
        }
        if hover != self.st.hover || self.left_down {
            self.st.hover = hover;
            self.refresh();
        }
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
            // arrows otherwise: navigate (files in dir mode, cells in sheet mode)
            KeyCode::ArrowUp => match st.source {
                Source::Tree { .. } => st.move_file_sel(-1, shift),
                Source::Sheet => st.move_block(0, -1),
            },
            KeyCode::ArrowDown => match st.source {
                Source::Tree { .. } => st.move_file_sel(1, shift),
                Source::Sheet => st.move_block(0, 1),
            },
            KeyCode::ArrowLeft => st.move_block(-1, 0),
            KeyCode::ArrowRight => st.move_block(1, 0),
            // vertical block stepping inside tall images (dir mode strips)
            KeyCode::KeyI => st.move_block(0, -1),
            KeyCode::KeyK => st.move_block(0, 1),
            KeyCode::Tab => st.set_block_size(if st.block == 16 { 8 } else { 16 }),
            KeyCode::KeyE => st.cur = Paint::Erase,
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
            KeyCode::KeyH => st.flip(true),
            KeyCode::KeyV => st.flip(false),
            KeyCode::KeyU => st.undo_pop(),
            KeyCode::KeyZ if ctrl => st.undo_pop(),
            KeyCode::KeyS => st.save(),
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
            .with_min_inner_size(LogicalSize::new(480.0, 328.0));
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
            WindowEvent::MouseInput { state, button, .. } => match state {
                ElementState::Pressed => self.on_mouse_press(button),
                ElementState::Released => {
                    if button == MouseButton::Left {
                        self.left_down = false;
                    }
                }
            },
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed
                    && let PhysicalKey::Code(code) = event.physical_key
                {
                    if code == KeyCode::Escape {
                        if self.st.dirty && !self.st.esc_armed {
                            self.st.esc_armed = true;
                            self.st.status = "UNSAVED EDITS: ESC AGAIN TO QUIT, S TO SAVE".into();
                            self.refresh();
                        } else {
                            event_loop.exit();
                        }
                        return;
                    }
                    self.on_key(code);
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
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
        "usage: pixel_studio [<dir> | <sheet.png>] [--sheet <png>] [--cell X Y] [--size 8|16]\n       \
         pixel_studio <sheet.png> [--set X Y (RRGGBB[AA]|t)]...          # headless edit\n       \
         pixel_studio <dir> --file <rel.png> [--set X Y ...]...          # headless edit\n\n\
         default target: assets/sprites (directory) if it exists, else assets/sprites.png"
    );
    std::process::exit(2);
}

fn arg_i32(args: &[String], i: usize) -> i32 {
    args.get(i)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| usage())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut target: Option<PathBuf> = None;
    let mut force_sheet = false;
    let mut cell: Option<(i32, i32)> = None;
    let mut size = 16i32;
    let mut file_rel: Option<String> = None;
    let mut sets: Vec<(i32, i32, Rgba)> = Vec::new();

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
            "--file" => {
                file_rel = Some(args.get(i + 1).cloned().unwrap_or_else(|| usage()));
                i += 1;
            }
            "--set" => {
                let (x, y) = (arg_i32(&args, i + 1), arg_i32(&args, i + 2));
                let c = args
                    .get(i + 3)
                    .and_then(|s| parse_set_color(s))
                    .unwrap_or_else(|| usage());
                sets.push((x, y, c));
                i += 3;
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
            PathBuf::from("assets/sprites.png")
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

    // headless batch mode: apply --set edits, back up, save, exit — no window
    if !sets.is_empty() {
        for &(x, y, c) in &sets {
            if !(0..img.w).contains(&x) || !(0..img.h).contains(&y) {
                eprintln!("--set {x} {y}: out of bounds ({}x{})", img.w, img.h);
                std::process::exit(2);
            }
            img.px[(x + y * img.w) as usize] = c;
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
            "wrote {} ({} pixel{}), backup at {}",
            path.display(),
            sets.len(),
            if sets.len() == 1 { "" } else { "s" },
            bak.display()
        );
        return;
    }

    let source = match entries {
        Some(entries) => Source::Tree {
            entries,
            sel: open_idx,
            scroll: 0,
        },
        None => Source::Sheet,
    };
    let mut st = Studio::new(source, path, img, size);
    if let Some((cx, cy)) = cell {
        st.bx = (cx * CELL).clamp(0, (st.img.w - 1).max(0));
        st.by = (cy * CELL).clamp(0, (st.img.h - 1).max(0));
        st.bx -= st.bx % st.block;
        st.by -= st.by % st.block;
    }

    println!("{}", st.title());
    println!("controls: see the panel under the left pane (or the doc header of this file)");

    let event_loop = EventLoop::new().expect("could not create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App {
        st,
        window: None,
        surface: None,
        needs_render: true,
        mods: ModifiersState::empty(),
        left_down: false,
        cursor: None,
    };
    event_loop.run_app(&mut app).expect("event loop error");
}
