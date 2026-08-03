//! `Studio` — the editor's whole state, and the window geometry every other part
//! reads.
//!
//! One open document ([`Image`]) plus a [`Source`] saying where it came from: a
//! monolithic sheet, a browsed file tree, or the whole tree stitched into one canvas.
//! Everything else here answers geometric questions about the *edit window* — the
//! rect of the image currently under the brush: which pixels it covers, what zoom
//! fits it, where it moves to, and how `G` snaps it onto a whole sprite.
//!
//! The behaviour lives in the sibling modules: [`paint`] for brush operations,
//! [`history`] for undo/redo, [`files`] for open/save, [`inspect`] for the read-only
//! queries the chrome displays, and [`render`]/[`preview`]/[`modals`] for drawing.

pub(crate) mod files;
pub(crate) mod history;
pub(crate) mod inspect;
pub(crate) mod modals;
pub(crate) mod paint;
pub(crate) mod preview;
pub(crate) mod render;

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use fdoom::gfx::Screen;
use fdoom::gfx::sprite_sheet::SpriteSheet;

use crate::atlas::{CELL, sprite_at, unit_origin};
use crate::canvas::{Placement, owner_of};
use crate::color::{Backdrop, Rgba, backdrops};
use crate::image::Image;
use crate::layout::{CANVAS_MAX, VIEW_H, VIEW_W};
use crate::library::Entry;

use history::History;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Paint {
    Erase,
    Shade(u8),
    Rgb([u8; 3]),
    Custom,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Tool {
    Pencil,
    Line,
    Rect,
    RectFill,
}

impl Tool {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Tool::Pencil => "PENCIL",
            Tool::Line => "LINE",
            Tool::Rect => "RECT",
            Tool::RectFill => "RECT FILL",
        }
    }
}

pub(crate) struct Clip {
    pub(crate) w: i32,
    pub(crate) h: i32,
    pub(crate) px: Vec<Rgba>,
}

pub(crate) struct Onion {
    pub(crate) w: i32,
    pub(crate) h: i32,
    pub(crate) px: Vec<Rgba>,
    pub(crate) label: String,
}

pub(crate) enum Source {
    /// Monolithic atlas: the left pane is the sheet itself, browsed by cell.
    Sheet,
    /// Directory tree: the left pane is a file browser.
    Tree {
        entries: Vec<Entry>,
        sel: usize,
        scroll: i32,
    },
    /// The whole tree stitched into one editable canvas (`W` toggles from Tree).
    /// Paints route to their owning file; `S` saves only the dirty files.
    Canvas {
        placements: Vec<Placement>,
        /// Placement index per 8px canvas cell, -1 for gaps (row-major, `img.w/8` wide).
        owner: Vec<i32>,
    },
}

/// The `N` new-sprite modal: name the file, pick a size preset, create, edit.
pub(crate) struct NewSprite {
    pub(crate) name: String,
    pub(crate) preset: usize, // SIZE_PRESETS index; usize::MAX once the size is hand-adjusted
    pub(crate) w: i32,
    pub(crate) h: i32,
    pub(crate) pal: bool, // advisory only: which UNPINNED list to remind the artist about
}

/// Common sprite shapes (docs/ART_GUIDE.md pixel-budget conventions).
pub(crate) const SIZE_PRESETS: &[(i32, i32, &str)] = &[
    (8, 8, "ITEM ICON"),
    (16, 16, "TILE / FURNITURE / MOB FRAME"),
    (64, 16, "MOB WALK STRIP (4 FRAMES)"),
    (32, 8, "TEXTURE ROW (4 VARIANTS)"),
    (24, 24, "CONNECTOR SPARSE 3X3"),
    (16, 24, "TREE SPECIES 2X3"),
];

pub(crate) struct Studio {
    pub(crate) source: Source,
    pub(crate) path: PathBuf, // currently open PNG
    pub(crate) img: Image,
    pub(crate) root: Option<PathBuf>, // sprite-tree root (dir and canvas modes)
    pub(crate) tree_rel: Option<String>, // file to reselect when leaving canvas mode
    pub(crate) sheet: Arc<SpriteSheet>, // the real game sheet (font + backdrop cells)
    pub(crate) backdrops: [Backdrop; 5],
    pub(crate) backdrop_idx: usize, // in-context preview backdrop (D cycles)
    pub(crate) new_sprite: Option<NewSprite>, // the N modal, when open
    pub(crate) find: Option<String>, // the / file-finder, when active (dir mode)
    pub(crate) manifest: HashMap<String, bool>, // rel path -> is_palette (dir mode, may be empty)
    pub(crate) bx: i32, // edit-window origin in image px (free; keyboard steps one 8px cell)
    pub(crate) by: i32,
    pub(crate) view_w: i32, // selected window size (Tab: 8/16; G: sprite footprint)
    pub(crate) view_h: i32,
    pub(crate) zoom_ovr: Option<i32>, // wheel-zoom override of the fit zoom
    pub(crate) pan: (i32, i32),       // canvas pan (zoomed px) when the window outgrows the canvas
    pub(crate) cur: Paint,
    pub(crate) prev_paint: Paint, // what `C` toggles back to
    pub(crate) custom: [u8; 3],
    pub(crate) chan: usize,
    pub(crate) swatches: Vec<[u8; 3]>,
    pub(crate) recent: VecDeque<[u8; 3]>, // last 8 painted colors
    pub(crate) tool: Tool,
    pub(crate) drag_anchor: Option<(i32, i32)>, // line/rect start (block px)
    pub(crate) mirror: bool,
    pub(crate) clipboard: Option<Clip>,
    pub(crate) paste_armed: bool,
    pub(crate) pal_idx: usize, // PREVIEW_PALS index
    pub(crate) anim_on: bool,
    pub(crate) anim_files: Vec<Image>, // dir mode: sibling frames; empty = strip flip
    pub(crate) anim_i: usize,
    pub(crate) onion_on: bool,
    pub(crate) onion: Option<Onion>,
    pub(crate) help_on: bool,
    pub(crate) history: History,
    pub(crate) dirty: bool,
    pub(crate) backed_up: HashSet<PathBuf>,
    pub(crate) status: String,
    pub(crate) hover: Option<(i32, i32)>, // block-relative pixel under cursor
    pub(crate) sheet_hover: Option<(i32, i32)>, // sheet-pane px under cursor (sheet mode)
    pub(crate) esc_armed: bool,
    pub(crate) text: Screen, // scratch 288x192 screen to rasterize the game font
    pub(crate) frame: Vec<u32>,
}

impl Studio {
    pub(crate) fn new(source: Source, path: PathBuf, img: Image, size: i32) -> Studio {
        let sheet = Arc::new(fdoom::assets::sprite_sheet());
        let mut s = Studio {
            source,
            path,
            img,
            root: None,
            tree_rel: None,
            sheet: sheet.clone(),
            backdrops: backdrops(),
            backdrop_idx: 0,
            new_sprite: None,
            find: None,
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
            history: History::default(),
            dirty: false,
            backed_up: HashSet::new(),
            status: String::new(),
            hover: None,
            sheet_hover: None,
            esc_armed: false,
            text: Screen::new(sheet),
            frame: vec![0; (VIEW_W * VIEW_H) as usize],
        };
        s.build_swatches();
        s
    }

    /* ------------------------------ geometry & access ------------------------------ */

    /// The edited rect: the whole image when it fits in 16x16, else the selected
    /// window clamped at the image edge (strips of any size work).
    pub(crate) fn block_rect(&self) -> (i32, i32, i32, i32) {
        if self.img.w <= 16 && self.img.h <= 16 {
            (0, 0, self.img.w, self.img.h)
        } else {
            let bw = self.view_w.min(self.img.w - self.bx).max(1);
            let bh = self.view_h.min(self.img.h - self.by).max(1);
            (self.bx, self.by, bw, bh)
        }
    }

    pub(crate) fn zoom(&self) -> i32 {
        let (_, _, bw, bh) = self.block_rect();
        let fit = (CANVAS_MAX / bw.max(bh)).clamp(1, 40);
        self.zoom_ovr.unwrap_or(fit)
    }

    pub(crate) fn clamp_pan(&mut self) {
        let (_, _, bw, bh) = self.block_rect();
        let z = self.zoom();
        self.pan.0 = self.pan.0.clamp(0, (bw * z - CANVAS_MAX).max(0));
        self.pan.1 = self.pan.1.clamp(0, (bh * z - CANVAS_MAX).max(0));
    }

    pub(crate) fn get(&self, x: i32, y: i32) -> Rgba {
        self.img.pixel(x, y)
    }

    pub(crate) fn put(&mut self, x: i32, y: i32, v: Rgba) {
        let i = (x + y * self.img.w) as usize;
        if self.img.px[i] != v {
            self.img.px[i] = v;
            self.dirty = true;
            self.esc_armed = false;
            // canvas mode: mark the owning file dirty (gaps save nowhere — warn)
            let cols = self.img.w / CELL;
            if let Source::Canvas { placements, owner } = &mut self.source {
                match owner[(x / CELL + (y / CELL) * cols) as usize] {
                    -1 => self.status = "GAP PIXEL: NO FILE OWNS THIS (WON'T SAVE)".into(),
                    o => placements[o as usize].dirty = true,
                }
            }
        }
    }

    /// Canvas mode: the placement index owning canvas pixel `(x, y)`, if any.
    pub(crate) fn owner_at(&self, x: i32, y: i32) -> Option<usize> {
        let Source::Canvas { owner, .. } = &self.source else {
            return None;
        };
        owner_of(owner, &self.img, x, y)
    }

    /// Move the window origin by whole 8px cells. Any cell is a legal origin — no
    /// even-cell snapping (graves live at cell x 15/17/19, flora at (15,26), ...).
    pub(crate) fn move_block(&mut self, dx: i32, dy: i32) {
        let nx = (self.bx.div_euclid(CELL) + dx) * CELL;
        let ny = (self.by.div_euclid(CELL) + dy) * CELL;
        self.set_origin(nx, ny);
    }

    pub(crate) fn set_origin(&mut self, nx: i32, ny: i32) {
        self.bx = nx.clamp(0, (self.img.w - CELL).max(0));
        self.by = ny.clamp(0, (self.img.h - CELL).max(0));
        self.hover = None;
        self.drag_anchor = None;
        self.clamp_pan();
    }

    pub(crate) fn set_view(&mut self, w: i32, h: i32) {
        self.view_w = w;
        self.view_h = h;
        self.zoom_ovr = None;
        self.pan = (0, 0);
        self.drag_anchor = None;
    }

    /* ---------------------------------- snapping ---------------------------------- */

    /// `G`: jump the window to the sprite under the sheet-pane cursor with its true
    /// footprint (odd origins included). Sheet mode uses the built-in sprite map;
    /// canvas mode uses the real file placements.
    pub(crate) fn snap_to_sprite(&mut self) {
        if let Source::Tree { .. } = self.source {
            self.status = "G: SHEET/CANVAS ONLY (FILES ARE ALREADY PER-SPRITE)".into();
            return;
        }
        let Some((sx, sy)) = self.sheet_hover else {
            self.status = "G: HOVER THE SHEET PANE FIRST".into();
            return;
        };
        match self.source {
            Source::Canvas { .. } => self.snap_to_file(sx, sy),
            _ => self.snap_to_mapped_sprite(sx, sy),
        }
    }

    /// Canvas mode: select the whole file under the cursor, at its own origin.
    fn snap_to_file(&mut self, sx: i32, sy: i32) {
        let Some(i) = self.owner_at(sx, sy) else {
            self.set_view(CELL, CELL);
            self.set_origin(sx - sx % CELL, sy - sy % CELL);
            self.status = "SNAP: GAP CELL (8X8, NO FILE)".into();
            return;
        };
        let Source::Canvas { placements, .. } = &self.source else {
            unreachable!();
        };
        let (x, y, w, h, rel) = {
            let p = &placements[i];
            (p.x, p.y, p.w, p.h, p.rel.clone())
        };
        self.set_view(w, h);
        self.set_origin(x, y);
        self.status = format!("SNAP: {rel}");
    }

    /// Sheet mode: select the sprite unit under the cursor via the built-in map.
    fn snap_to_mapped_sprite(&mut self, sx: i32, sy: i32) {
        let (ccx, ccy) = (sx / CELL, sy / CELL);
        match sprite_at(ccx, ccy) {
            Some(span) => {
                let (ox, oy, uw, uh) = unit_origin(span, ccx, ccy);
                self.set_view(uw * CELL, uh * CELL);
                self.set_origin(ox * CELL, oy * CELL);
                self.status = format!("SNAP: {}", span.6);
            }
            None => {
                self.set_view(CELL, CELL);
                self.set_origin(ccx * CELL, ccy * CELL);
                self.status = "SNAP: UNMAPPED CELL (8X8)".into();
            }
        }
    }
}
