//! The preview strip, and the animation that plays inside it.
//!
//! The point of this strip is to answer "how will this look in the game?" without
//! booting the game. So the backdrops are not decorative: each texel is sampled from
//! the real terrain texture cells of the loaded sheet and recoloured through the exact
//! `get4` palette word the tile code uses, with the same pseudo-random per-cell
//! variant the game picks. Judging terrain taste — "calm base, sparse detail" —
//! happens here.
//!
//! Left to right: the window at raw 1x/2x/4x, the same sprite composited over the
//! backdrop at 1x/2x/4x, the animation frame if playing, and a 3x3 tiling for judging
//! seamless edges. Anything that does not fit is dropped, biggest first.

use std::path::Path;

use fdoom::gfx::sprite_sheet::SheetPixel;

use crate::atlas::CELL;
use crate::color::{PREVIEW_PALS, checker, night, palette_base, palette_shade};
use crate::image::load_png;
use crate::layout::{PANEL, PREVIEW_Y, RX, TXT_DIM, VIEW_H, VIEW_W};
use crate::library::Entry;

use super::{Onion, Source, Studio};

impl Studio {
    /* --------------------------------- backdrops --------------------------------- */

    /// In-context backdrop texel at texture px `(tx, ty)`: the real terrain texture
    /// cell (variant picked pseudo-randomly per 8px tile cell, like the game's
    /// `Sprite::dots_at`) recolored through the tile's actual `get4` palette.
    fn backdrop_texel(&self, tx: i32, ty: i32) -> u32 {
        let b = &self.backdrops[self.backdrop_idx];
        let (tcx, tcy) = (tx.div_euclid(CELL), ty.div_euclid(CELL));
        let variant = (tcx * 7 + tcy * 13 + tcx * tcy).rem_euclid(4);
        let sx = (b.cell_x + variant) * CELL + tx.rem_euclid(CELL);
        let sy = b.cell_y * CELL + ty.rem_euclid(CELL);
        let idx = (sx + sy * self.sheet.width) as usize;
        let col = match self.sheet.pixels.get(idx) {
            Some(&SheetPixel::Rgb(c)) => c as u32,
            // shade 0 is the tile's base colour: it fills wherever the texture
            // sprite draws nothing, so the ground is never see-through
            Some(&SheetPixel::Palette(s)) => {
                palette_shade(b.pal, s as i32).unwrap_or_else(|| palette_base(b.pal))
            }
            Some(&SheetPixel::Transparent) => palette_base(b.pal),
            None => checker(tx / 4, ty / 4), // sheet without these cells (odd targets)
        };
        if b.night { night(col) } else { col }
    }

    /// Fill a `w x h` rect of backdrop at `(x, y)`, magnified `scale` times.
    fn fill_backdrop(&mut self, x: i32, y: i32, w: i32, h: i32, scale: i32) {
        for yy in 0..h {
            for xx in 0..w {
                let c = self.backdrop_texel(xx / scale, yy / scale);
                self.fill_rect(x + xx, y + yy, 1, 1, c);
            }
        }
    }

    /* ------------------------------- the preview strip ------------------------------- */

    pub(crate) fn draw_preview(&mut self) {
        let (bx, by, bw, bh) = self.block_rect();
        let (pal_name, _) = PREVIEW_PALS[self.pal_idx];
        let bd_name = self.backdrops[self.backdrop_idx].name;
        let label = format!(
            "RAW 1X 2X 4X | IN GAME  D:{bd_name}  P:{pal_name}{}",
            if self.anim_on { " | ANIM" } else { "" }
        );
        self.draw_text(RX, PREVIEW_Y - 12, &label, TXT_DIM);

        let mut x = RX;
        x = self.draw_raw_scales(x, bx, by, bw, bh);
        x += 4;
        x = self.draw_in_context(x, bx, by, bw, bh);
        if self.anim_on {
            x = self.draw_anim_frame(x, bx, by, bw, bh);
        }
        self.draw_tiled_3x3(x, bx, by, bw, bh);
        self.draw_onion_note();
    }

    /// The window itself at 1x, 2x and 4x on a flat panel.
    fn draw_raw_scales(&mut self, mut x: i32, bx: i32, by: i32, bw: i32, bh: i32) -> i32 {
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
        x
    }

    /// The sprite over the real terrain texture at 1x, 2x and 4x, with a half-tile
    /// apron all around. Whatever does not fit is dropped, biggest first.
    fn draw_in_context(&mut self, mut x: i32, bx: i32, by: i32, bw: i32, bh: i32) -> i32 {
        let is_night = self.backdrops[self.backdrop_idx].night;
        for scale in [1, 2, 4] {
            let (sw, sh) = ((bw + 8) * scale, (bh + 8) * scale);
            if x + sw > VIEW_W - 4 || PREVIEW_Y + sh > VIEW_H {
                break;
            }
            self.fill_backdrop(x, PREVIEW_Y, sw, sh, scale);
            for y in 0..bh {
                for px in 0..bw {
                    if let Some(mut c) = self.shown(self.get(bx + px, by + y)) {
                        if is_night {
                            c = night(c);
                        }
                        self.fill_rect(
                            x + (4 + px) * scale,
                            PREVIEW_Y + (4 + y) * scale,
                            scale,
                            scale,
                            c,
                        );
                    }
                }
            }
            x += sw + 8;
        }
        x
    }

    /// The animation frame at 2x over the backdrop: sibling files when the folder has
    /// them, otherwise the next window along the strip.
    fn draw_anim_frame(&mut self, x: i32, bx: i32, by: i32, bw: i32, bh: i32) -> i32 {
        let (fw, fh) = if self.anim_files.is_empty() {
            (bw, bh)
        } else {
            let f = &self.anim_files[self.anim_i.min(self.anim_files.len() - 1)];
            (f.w.min(24), f.h.min(24))
        };
        let (w2, h2) = (fw * 2 + 8, fh * 2 + 8);
        self.fill_backdrop(x, PREVIEW_Y, w2, h2, 2);
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
        x + w2 + 8
    }

    /// A 3x3 tiling at 2x for judging seamless edges (16px windows only, space
    /// allowing).
    fn draw_tiled_3x3(&mut self, x: i32, bx: i32, by: i32, bw: i32, bh: i32) {
        if bw > 16 || bh > 16 || x + bw * 6 + 2 > VIEW_W - 4 {
            return;
        }
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

    fn draw_onion_note(&mut self) {
        if let Some(o) = &self.onion {
            let s = format!(
                "ONION REF: {} ({})",
                o.label,
                if self.onion_on { "ON" } else { "OFF - O" }
            );
            self.draw_text(RX, VIEW_H - 16, &s, TXT_DIM);
        }
    }

    /* --------------------------------- animation --------------------------------- */

    /// `A`: dir mode plays the sibling files of the open file's folder (walk frames
    /// as files, at the game's walk cadence); with no siblings — or in sheet mode —
    /// it flips the window between two side-by-side frames instead (2-frame flames,
    /// mob frame strips on the atlas).
    pub(crate) fn toggle_anim(&mut self) {
        if self.anim_on {
            self.anim_on = false;
            self.anim_files.clear();
            return;
        }
        self.anim_files.clear();
        self.load_sibling_frames();
        let (_, _, bw, _) = self.block_rect();
        if self.anim_files.is_empty() && self.bx + bw * 2 > self.img.w {
            self.status = "ANIM: NO SIBLING FRAMES / NO ROOM TO FLIP".into();
            return;
        }
        self.anim_i = 0;
        self.anim_on = true;
        self.status = format!("ANIM: {} FRAMES AT WALK CADENCE", self.anim_frame_count());
    }

    /// Dir mode: load every PNG sitting next to the open file as an animation frame.
    fn load_sibling_frames(&mut self) {
        let Source::Tree { entries, sel, .. } = &self.source else {
            return;
        };
        let Some(dir) = entries[*sel].path.parent().map(Path::to_path_buf) else {
            return;
        };
        let sib: Vec<&Entry> = entries
            .iter()
            .filter(|e| !e.is_dir && e.path.parent() == Some(dir.as_path()))
            .collect();
        if sib.len() <= 1 {
            return;
        }
        let paths: Vec<_> = sib.iter().map(|e| e.path.clone()).collect();
        for p in paths {
            if let Ok(img) = load_png(&p) {
                self.anim_files.push(img);
            }
        }
    }

    /// Frames in the current animation: the sibling files, or the 2-frame strip flip.
    fn anim_frame_count(&self) -> usize {
        if self.anim_files.is_empty() {
            2
        } else {
            self.anim_files.len()
        }
    }

    pub(crate) fn anim_advance(&mut self) {
        self.anim_i = (self.anim_i + 1) % self.anim_frame_count();
    }

    /// `B`: capture the current window as the onion-skin reference.
    pub(crate) fn capture_onion(&mut self) {
        let (bx, by, bw, bh) = self.block_rect();
        let mut px = Vec::with_capacity((bw * bh) as usize);
        for y in 0..bh {
            for x in 0..bw {
                px.push(self.get(bx + x, by + y));
            }
        }
        self.onion = Some(Onion {
            w: bw,
            h: bh,
            px,
            label: self.file_label(),
        });
        self.onion_on = true;
        self.status = "ONION REFERENCE CAPTURED (O TOGGLES)".into();
    }

    /// `O`: toggle the onion skin, refusing when there is nothing to ghost.
    pub(crate) fn toggle_onion(&mut self) {
        self.onion_on = !self.onion_on;
        if self.onion.is_none() {
            self.status = "ONION: CAPTURE A REFERENCE FIRST (B)".into();
            self.onion_on = false;
        }
    }
}
