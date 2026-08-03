//! Brush and tool operations: everything that changes pixels.
//!
//! Every entry point here works in *window-relative* coordinates — `(0, 0)` is the
//! top-left of the edit window, not of the image — because that is what the canvas
//! hands back from a click. [`Studio::stamp`] is the single write path: it clips to
//! the window, applies mirror-draw, and records the colour as recently used, so no
//! tool can paint outside the selection or forget the mirror.
//!
//! Undo is the caller's job: tools snapshot *before* their first write, which is why
//! a drag records one undo level for the whole stroke rather than one per pixel.

use crate::atlas::CELL;
use crate::canvas::nudge_rect;
use crate::color::{GRAYS, Kind, Rgba, classify, key};
use crate::image::nudge_image;
use crate::shapes::{line_points, rect_points};

use super::{Clip, Paint, Source, Studio, Tool};

impl Studio {
    /* ---------------------------------- the brush ---------------------------------- */

    pub(crate) fn paint_rgba(&self) -> Rgba {
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
    pub(crate) fn stamp(&mut self, pts: &[(i32, i32)]) {
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

    /// The points the active shape tool covers for a drag from `(ax, ay)` to
    /// `(hx, hy)`. Empty for the pencil, which paints as the cursor moves instead.
    /// Both the drag ghost and the committed stroke read this, so the preview is
    /// exactly what lands.
    pub(crate) fn shape_points(&self, ax: i32, ay: i32, hx: i32, hy: i32) -> Vec<(i32, i32)> {
        match self.tool {
            Tool::Line => line_points(ax, ay, hx, hy),
            Tool::Rect => rect_points(ax, ay, hx, hy, false),
            Tool::RectFill => rect_points(ax, ay, hx, hy, true),
            Tool::Pencil => Vec::new(),
        }
    }

    /// The shape currently being dragged, if any — anchor set and cursor on canvas.
    pub(crate) fn drag_shape(&self) -> Vec<(i32, i32)> {
        match (self.drag_anchor, self.hover) {
            (Some((ax, ay)), Some((hx, hy))) => self.shape_points(ax, ay, hx, hy),
            _ => Vec::new(),
        }
    }

    pub(crate) fn eyedrop(&mut self, px: i32, py: i32) {
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

    pub(crate) fn flood_fill(&mut self, px: i32, py: i32) {
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

    pub(crate) fn flip(&mut self, horizontal: bool) {
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
    pub(crate) fn shade_shift(&mut self, up: bool) {
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

    /* -------------------------------- copy & paste -------------------------------- */

    /// Ctrl+C: copy the current window. Ctrl+V arms paste; clicking stamps it.
    pub(crate) fn copy_block(&mut self) {
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

    pub(crate) fn paste_at(&mut self, px: i32, py: i32) {
        let Some(clip) = self.clipboard.take() else {
            return;
        };
        let (bx, by, _, _) = self.block_rect();
        let (dx, dy) = (bx + px, by + py);
        self.push_undo_rect(dx, dy, clip.w, clip.h);
        for y in 0..clip.h {
            for x in 0..clip.w {
                if self.img.contains(dx + x, dy + y) {
                    self.put(dx + x, dy + y, clip.px[(x + y * clip.w) as usize]);
                }
            }
        }
        self.status = format!("PASTED {}X{} AT {px},{py}", clip.w, clip.h);
        self.clipboard = Some(clip);
        self.paste_armed = false;
    }

    /* ----------------------------------- nudging ----------------------------------- */

    /// Shift+arrows: wrap-nudge the whole image — except in canvas mode, where the
    /// image is the whole atlas, so the nudge wraps only the file under the window.
    pub(crate) fn nudge(&mut self, dx: i32, dy: i32) {
        if let Source::Canvas { .. } = self.source {
            self.nudge_owned_file(dx, dy);
            return;
        }
        self.push_undo_rect(0, 0, self.img.w, self.img.h);
        nudge_image(&mut self.img, dx, dy);
        self.dirty = true;
        self.status = format!("NUDGED {dx},{dy} (WRAPS)");
    }

    /// Canvas mode: wrap-shift only the placement under the window origin.
    fn nudge_owned_file(&mut self, dx: i32, dy: i32) {
        let Some(i) = self.owner_at(self.bx, self.by) else {
            self.status = "NUDGE: NO FILE UNDER THE WINDOW".into();
            return;
        };
        let Source::Canvas { placements, .. } = &mut self.source else {
            unreachable!();
        };
        let (x, y, w, h, rel) = {
            let p = &mut placements[i];
            p.dirty = true;
            (p.x, p.y, p.w, p.h, p.rel.clone())
        };
        self.push_undo_rect(x, y, w, h);
        nudge_rect(&mut self.img, x, y, w, h, dx, dy);
        self.dirty = true;
        self.status = format!("NUDGED {rel} BY {dx},{dy} (WRAPS)");
    }

    /* ---------------------------------- analysis ---------------------------------- */

    /// True when any single 8x8 cell of the window mixes palette grays with
    /// saturated colors — usually a mistake.
    pub(crate) fn block_mixes_modes(&self) -> bool {
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
}
