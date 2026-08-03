//! Drawing the UI frame: primitives, the header, the two left panes, the edit
//! canvas, and the palette banks.
//!
//! The frame is a plain `Vec<u32>` of 960x720 pixels that `app` blits to the window;
//! everything here is filled rects and game-font text, no widgets and no layout
//! engine. Rendering is a pure function of studio state — nothing in this module
//! changes what is being edited — so a frame can be rendered headlessly (`--shot`)
//! and look exactly like the live window.

use fdoom::gfx::{color, font};

use crate::atlas::CELL;
use crate::color::{ACCENT_BLEND, blend, checker, rgb24};
use crate::layout::*;

use super::inspect::mark;
use super::{Paint, Source, Studio, Tool};

/// The edit canvas's derived geometry for one frame: which image pixels are on
/// screen, at what zoom, offset by how much pan.
struct CanvasView {
    bx: i32,
    by: i32,
    bw: i32,
    bh: i32,
    z: i32,
    pan_x: i32,
    pan_y: i32,
    vis_w: i32,
    vis_h: i32,
    clip: (i32, i32, i32, i32),
}

impl CanvasView {
    /// Frame coordinates of the top-left corner of window pixel `(x, y)`.
    fn screen(&self, x: i32, y: i32) -> (i32, i32) {
        (
            RX + x * self.z - self.pan_x,
            CANVAS_Y + y * self.z - self.pan_y,
        )
    }
}

impl Studio {
    /* --------------------------------- primitives --------------------------------- */

    pub(crate) fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, col: u32) {
        self.fill_clip(x, y, w, h, col, (0, 0, VIEW_W, VIEW_H));
    }

    pub(crate) fn fill_clip(
        &mut self,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        col: u32,
        clip: (i32, i32, i32, i32),
    ) {
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

    pub(crate) fn outline(&mut self, x: i32, y: i32, w: i32, h: i32, col: u32) {
        self.fill_rect(x, y, w, 1, col);
        self.fill_rect(x, y + h - 1, w, 1, col);
        self.fill_rect(x, y, 1, h, col);
        self.fill_rect(x + w - 1, y, 1, h, col);
    }

    /// Rasterize `s` with the game font into the frame (worldview's trick: draw on
    /// a scratch 288-wide screen in 32-char chunks, then copy lit pixels).
    pub(crate) fn draw_text(&mut self, x: i32, y: i32, s: &str, readable: i32) {
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

    /* ----------------------------------- the frame ----------------------------------- */

    pub(crate) fn render(&mut self) {
        self.frame.fill(BG);
        self.draw_header();
        match &self.source {
            Source::Tree { .. } => self.draw_tree_pane(),
            _ => self.draw_sheet_pane(),
        }
        self.draw_legend();
        self.draw_canvas();
        self.draw_palette();
        self.draw_preview();
        if self.help_on {
            self.draw_help();
        }
        if self.new_sprite.is_some() {
            self.draw_new_sprite();
        }
    }

    /* ----------------------------------- header ----------------------------------- */

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
        let line2 = format!(
            "PAINT: {}{}{}",
            self.paint_desc(),
            self.header_hover(),
            self.header_flags()
        );
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

    /// The hovered pixel's coordinates and value, for the second header line.
    fn header_hover(&self) -> String {
        use crate::color::{Kind, classify};
        let (bx, by, bw, bh) = self.block_rect();
        match self.hover {
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
        }
    }

    /// The active-mode badges: tool, mirror, armed paste, onion, hovered file.
    fn header_flags(&self) -> String {
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
        // canvas mode: name the file under the sheet-pane cursor (secondary slot)
        if let Some((sx, sy)) = self.sheet_hover
            && let Some(i) = self.owner_at(sx, sy)
            && let Source::Canvas { placements, .. } = &self.source
        {
            flags += &format!("  HOVER: {}", placements[i].rel);
        }
        flags
    }

    /* ---------------------------------- left panes ---------------------------------- */

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
        self.draw_placement_boxes(off_x, off_y, vw, vh);
        let (bx, by, bw, bh) = self.block_rect();
        self.outline(
            PANE_X + (bx - off_x) * 2 - 1,
            PANE_Y + (by - off_y) * 2 - 1,
            bw * 2 + 2,
            bh * 2 + 2,
            ACCENT,
        );
    }

    /// Canvas mode: outline dirty files (red) and the hovered file (dim).
    fn draw_placement_boxes(&mut self, off_x: i32, off_y: i32, vw: i32, vh: i32) {
        let mut boxes: Vec<(i32, i32, i32, i32, u32)> = Vec::new();
        if let Source::Canvas { placements, .. } = &self.source {
            let hovered = self.sheet_hover.and_then(|(sx, sy)| self.owner_at(sx, sy));
            for (i, p) in placements.iter().enumerate() {
                let col = if p.dirty {
                    0xC05050
                } else if Some(i) == hovered {
                    0x5A6674
                } else {
                    continue;
                };
                // only boxes fully inside the visible view (outline doesn't pane-clip)
                if p.x >= off_x
                    && p.y >= off_y
                    && p.x + p.w <= off_x + vw
                    && p.y + p.h <= off_y + vh
                {
                    boxes.push((p.x - off_x, p.y - off_y, p.w, p.h, col));
                }
            }
        }
        for (x, y, w, h, col) in boxes {
            self.outline(
                PANE_X + x * 2 - 1,
                PANE_Y + y * 2 - 1,
                w * 2 + 2,
                h * 2 + 2,
                col,
            );
        }
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
                "EDIT: L-PAINT R-PICK F FILL L/R TOOLS H/V FLIP",
                "WHEEL ZOOM - MID-DRAG PAN - P PALETTE - D BACKDROP",
                "U UNDO Y REDO - S SAVE - B/O ONION - CTRL+C/V COPY/PASTE",
                "? FULL KEY LIST - ESC QUIT",
            ],
            Source::Tree { .. } => [
                "FILES: CLICK/UP+DOWN - / FIND - N NEW - W WHOLE-SHEET",
                "EDIT: L-PAINT R-PICK F FILL L/R TOOLS H/V FLIP",
                "WHEEL ZOOM - MID-DRAG PAN - P PALETTE - D BACKDROP",
                "U UNDO Y REDO - S SAVE - B/O ONION - CTRL+C/V COPY/PASTE",
                "? FULL KEY LIST - ESC QUIT",
            ],
            Source::Canvas { .. } => [
                "WHOLE SHEET: EVERY FILE - W BACK TO FILES - N NEW",
                "CLICK/ARROWS MOVE - G SNAP TO FILE - RED BOX = DIRTY",
                "PAINTS ROUTE TO THEIR FILE - S SAVES ONLY DIRTY FILES",
                "SHIFT+ARROWS NUDGE THE FILE UNDER THE WINDOW (WRAPS)",
                "? FULL KEY LIST - ESC QUIT",
            ],
        };
        for (i, l) in lines.iter().enumerate() {
            self.draw_text(PANE_X, PANE_Y + PANE_H + 8 + i as i32 * 12, l, TXT_DIM);
        }
    }

    /* ---------------------------------- edit canvas ---------------------------------- */

    fn draw_canvas(&mut self) {
        let (bx, by, bw, bh) = self.block_rect();
        let z = self.zoom();
        let (pan_x, pan_y) = self.pan;
        let v = CanvasView {
            bx,
            by,
            bw,
            bh,
            z,
            pan_x,
            pan_y,
            vis_w: (bw * z - pan_x).min(CANVAS_MAX),
            vis_h: (bh * z - pan_y).min(CANVAS_MAX),
            clip: (RX, CANVAS_Y, CANVAS_MAX, CANVAS_MAX),
        };
        self.fill_rect(RX - 1, CANVAS_Y - 1, v.vis_w + 2, v.vis_h + 2, GRID_MAJOR);
        self.draw_canvas_pixels(&v);
        self.draw_paste_ghost(&v);
        self.draw_pixel_grid(&v);
        self.draw_mirror_axis(&v);
        self.draw_hover_outline(&v);
    }

    /// The window's pixels, with transparency showing the checker (ghosted with the
    /// onion reference when one is on) and the in-progress shape tinted over the top.
    fn draw_canvas_pixels(&mut self, v: &CanvasView) {
        let ghost_pts = self.drag_shape();
        // taken out for the duration so the reference can be read while `self` is
        // borrowed mutably to draw; put back below
        let onion = if self.onion_on {
            self.onion.take()
        } else {
            None
        };
        for y in 0..v.bh {
            for x in 0..v.bw {
                let (sx, sy) = v.screen(x, y);
                if sx + v.z <= RX
                    || sy + v.z <= CANVAS_Y
                    || sx >= RX + CANVAS_MAX
                    || sy >= CANVAS_Y + CANVAS_MAX
                {
                    continue;
                }
                let p = self.get(v.bx + x, v.by + y);
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
                    col = blend(ACCENT, col, ACCENT_BLEND);
                }
                self.fill_clip(sx, sy, v.z, v.z, col, v.clip);
            }
        }
        if self.onion.is_none() {
            self.onion = onion;
        }
    }

    /// Armed paste: the clipboard follows the cursor, half-blended over the canvas.
    fn draw_paste_ghost(&mut self, v: &CanvasView) {
        if !self.paste_armed {
            return;
        }
        let (Some(clipb), Some((hx, hy))) = (&self.clipboard, self.hover) else {
            return;
        };
        let mut ghost = Vec::new();
        for y in 0..clipb.h {
            for x in 0..clipb.w {
                ghost.push((hx + x, hy + y, clipb.px[(x + y * clipb.w) as usize]));
            }
        }
        for (x, y, p) in ghost {
            if x < v.bw && y < v.bh {
                let (sx, sy) = v.screen(x, y);
                let under = checker(x, y);
                let col = blend(self.shown(p).unwrap_or(under), under, 150);
                self.fill_clip(sx, sy, v.z, v.z, col, v.clip);
            }
        }
    }

    /// Pixel grid, with a brighter line on 8px cell boundaries. Only worth drawing
    /// once a pixel is big enough to see the lines between.
    fn draw_pixel_grid(&mut self, v: &CanvasView) {
        if v.z < 4 {
            return;
        }
        for x in 1..v.bw {
            let col = if x % CELL == 0 { GRID_MAJOR } else { GRID };
            self.fill_clip(RX + x * v.z - v.pan_x, CANVAS_Y, 1, v.vis_h, col, v.clip);
        }
        for y in 1..v.bh {
            let col = if y % CELL == 0 { GRID_MAJOR } else { GRID };
            self.fill_clip(RX, CANVAS_Y + y * v.z - v.pan_y, v.vis_w, 1, col, v.clip);
        }
    }

    fn draw_mirror_axis(&mut self, v: &CanvasView) {
        if !self.mirror {
            return;
        }
        let mx = RX + (v.bw * v.z) / 2 - v.pan_x;
        self.fill_clip(mx, CANVAS_Y, 1, v.vis_h, 0xE06060, v.clip);
    }

    fn draw_hover_outline(&mut self, v: &CanvasView) {
        if let Some((px, py)) = self.hover
            && px < v.bw
            && py < v.bh
        {
            let (sx, sy) = v.screen(px, py);
            self.outline(sx, sy, v.z + 1, v.z + 1, ACCENT);
        }
    }

    /* ------------------------------- the palette banks ------------------------------- */

    fn draw_palette(&mut self) {
        self.draw_shade_bank();
        self.draw_color_bank();
        self.draw_recent_bank();
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

    /// Bank A: the four legal palette shades + transparent.
    fn draw_shade_bank(&mut self) {
        use crate::color::GRAYS;
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
    }

    /// Bank B: sampled true colors, 2 rows of 12, plus the custom swatch.
    fn draw_color_bank(&mut self) {
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
    }

    /// The last 8 painted colors, for reaching back without hunting the bank.
    fn draw_recent_bank(&mut self) {
        self.draw_text(RX, RECENT_Y + 3, "RECENT", TXT_DIM);
        let recent: Vec<[u8; 3]> = self.recent.iter().copied().collect();
        for (i, c) in recent.iter().enumerate() {
            let x = SWATCH_X + i as i32 * 17;
            self.fill_rect(x, RECENT_Y, 14, 14, rgb24([c[0], c[1], c[2], 255]));
            self.outline(x - 1, RECENT_Y - 1, 16, 16, GRID_MAJOR);
        }
    }
}
