//! Read-only questions about the open art: what colours it uses, whether it obeys
//! its declared palette mode, how it will look under a game palette, and the strings
//! the chrome puts on screen.
//!
//! Nothing here mutates pixels. The mode warning is the part that earns its keep:
//! a palette pixel that should have been true colour silently changes colour in
//! game, which is the single easiest mistake to make in this editor, so a file with a
//! declared `pal`/`rgb` mode is checked against it and everything else is checked for
//! the tell-tale gray-and-colour mix inside one 8x8 cell.

use std::collections::HashMap;

use crate::atlas::{CELL, artgen_region, sprite_at};
use crate::color::{GRAYS, Kind, PREVIEW_PALS, Rgba, classify, key, palette_shade, rgb24};
use crate::image::{Image, load_png};

use super::{Paint, Source, Studio};

/// What a scan of some pixels found, in the terms a mode check cares about.
#[derive(Default, Clone, Copy)]
struct ModeScan {
    color: bool,
    gray: bool,
    off_ladder: bool,
}

impl ModeScan {
    fn add(&mut self, p: Rgba) {
        match classify(p) {
            Kind::Color => self.color = true,
            Kind::Gray(v) => {
                self.gray = true;
                self.off_ladder |= !GRAYS.contains(&v);
            }
            Kind::Transparent => {}
        }
    }

    /// The warning these pixels earn for a file declared `pal` (or not) — a palette
    /// file may only hold ladder grays, an rgb file may hold no grays at all.
    fn verdict(self, is_pal: bool) -> Option<String> {
        if is_pal && self.color {
            Some("! PAL FILE CONTAINS COLOR PIXELS".into())
        } else if is_pal && self.off_ladder {
            Some("! PAL FILE HAS OFF-LADDER GRAYS".into())
        } else if !is_pal && self.gray {
            Some("! RGB FILE CONTAINS GRAY (PAL) PIXELS".into())
        } else {
            None
        }
    }
}

impl Studio {
    /* ----------------------------- swatches & analysis ----------------------------- */

    /// The true-color bank: the ~24 most-used saturated colors across the source
    /// (whole sheet in sheet mode; every file in the tree, capped, in dir mode).
    pub(crate) fn build_swatches(&mut self) {
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
    /// contract; without one, warn when a single 8x8 cell of the window mixes grays
    /// with colors.
    pub(crate) fn art_warning(&self) -> Option<String> {
        if let Some(is_pal) = self.tree_declared_mode() {
            let mut scan = ModeScan::default();
            for &p in &self.img.px {
                scan.add(p);
            }
            return scan.verdict(is_pal);
        }
        if let Some((is_pal, (x, y, w, h))) = self.canvas_declared_mode() {
            let mut scan = ModeScan::default();
            for yy in y..y + h {
                for xx in x..x + w {
                    scan.add(self.get(xx, yy));
                }
            }
            return scan.verdict(is_pal);
        }
        if self.block_mixes_modes() {
            Some("! GRAY + COLOR MIXED IN CELL".into())
        } else {
            None
        }
    }

    /// Dir mode: the manifest-declared mode of the selected file, when it is pinned.
    fn tree_declared_mode(&self) -> Option<bool> {
        let Source::Tree { entries, sel, .. } = &self.source else {
            return None;
        };
        self.manifest.get(&entries[*sel].rel).copied()
    }

    /// Canvas mode: the declared mode and canvas rect of the file under the window.
    fn canvas_declared_mode(&self) -> Option<(bool, (i32, i32, i32, i32))> {
        let i = self.owner_at(self.bx, self.by)?;
        let Source::Canvas { placements, .. } = &self.source else {
            return None;
        };
        let p = &placements[i];
        let is_pal = *self.manifest.get(&p.rel)?;
        Some((is_pal, (p.x, p.y, p.w, p.h)))
    }

    /* ------------------------------ palette preview ------------------------------ */

    /// How a sheet pixel displays under the active preview palette: palette grays go
    /// through the packed `get4` word exactly like `Screen::render` (byte 255 =
    /// transparent, else `color::upgrade`); true colors and RAW mode pass through.
    pub(crate) fn shown(&self, p: Rgba) -> Option<u32> {
        match classify(p) {
            Kind::Transparent => None,
            Kind::Color => Some(rgb24(p)),
            Kind::Gray(v) => {
                if self.pal_idx == 0 {
                    return Some(rgb24(p));
                }
                let (_, pal) = PREVIEW_PALS[self.pal_idx];
                palette_shade(pal, (v / 64) as i32)
            }
        }
    }

    pub(crate) fn shown_img(&self, img: &Image, x: i32, y: i32) -> Option<u32> {
        self.shown(img.pixel(x, y))
    }

    /* ---------------------------------- labels ---------------------------------- */

    pub(crate) fn file_label(&self) -> String {
        match &self.source {
            Source::Sheet => self
                .path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            Source::Tree { entries, sel, .. } => entries[*sel].rel.clone(),
            Source::Canvas { placements, .. } => {
                let dirty = placements.iter().filter(|p| p.dirty).count();
                match dirty {
                    0 => format!("CANVAS ({} FILES)", placements.len()),
                    n => format!("CANVAS ({} FILES, {n} DIRTY)", placements.len()),
                }
            }
        }
    }

    /// Sheet mode: the sprite-map name for the window origin (region as fallback).
    /// Dir mode: the folder plus the manifest-declared pal/rgb mode.
    pub(crate) fn sprite_label(&self) -> String {
        match &self.source {
            Source::Sheet if self.img.w == 256 => {
                let (ccx, ccy) = (self.bx / CELL, self.by / CELL);
                match sprite_at(ccx, ccy) {
                    Some(&(.., name)) => name.to_string(),
                    None => artgen_region(ccy).to_string(),
                }
            }
            Source::Sheet => "SHEET".into(),
            Source::Canvas { placements, .. } => match self.owner_at(self.bx, self.by) {
                Some(i) => {
                    let p = &placements[i];
                    let mode = match self.manifest.get(&p.rel) {
                        Some(true) => " PAL",
                        Some(false) => " RGB",
                        None => " (UNPINNED)",
                    };
                    format!("{}{}{}", p.rel, mode, if p.dirty { " *" } else { "" })
                }
                None => "GAP (NO FILE)".into(),
            },
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

    pub(crate) fn paint_desc(&self) -> String {
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

    pub(crate) fn title(&self) -> String {
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
}

/// The `>` cursor the RGB stepper puts against the channel it is editing.
pub(crate) fn mark(active: bool) -> &'static str {
    if active { ">" } else { " " }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(pixels: &[Rgba]) -> ModeScan {
        let mut s = ModeScan::default();
        for &p in pixels {
            s.add(p);
        }
        s
    }

    const WHITE: Rgba = [255, 255, 255, 255];
    const OFF_LADDER: Rgba = [28, 28, 28, 255];
    const TRUE_COLOR: Rgba = [31, 27, 24, 255];
    const CLEAR: Rgba = [0, 0, 0, 0];

    /// A palette file may hold ladder grays and nothing else.
    #[test]
    fn pal_files_reject_colour_and_off_ladder_grays() {
        assert_eq!(scan(&[WHITE, CLEAR]).verdict(true), None, "clean pal file");
        assert_eq!(
            scan(&[WHITE, TRUE_COLOR]).verdict(true).as_deref(),
            Some("! PAL FILE CONTAINS COLOR PIXELS")
        );
        assert_eq!(
            scan(&[WHITE, OFF_LADDER]).verdict(true).as_deref(),
            Some("! PAL FILE HAS OFF-LADDER GRAYS")
        );
    }

    /// An rgb file may hold no palette grays at all — that is the "my gray pixel
    /// changed colour in game" bug, caught before it ships.
    #[test]
    fn rgb_files_reject_any_gray() {
        assert_eq!(scan(&[TRUE_COLOR, CLEAR]).verdict(false), None);
        assert_eq!(
            scan(&[TRUE_COLOR, WHITE]).verdict(false).as_deref(),
            Some("! RGB FILE CONTAINS GRAY (PAL) PIXELS")
        );
        // an off-ladder gray is still a gray, so it fails the rgb check too
        assert_eq!(
            scan(&[OFF_LADDER]).verdict(false).as_deref(),
            Some("! RGB FILE CONTAINS GRAY (PAL) PIXELS")
        );
    }

    /// Transparency is mode-neutral: an empty file never warns either way.
    #[test]
    fn transparency_is_mode_neutral() {
        let empty = scan(&[CLEAR, CLEAR]);
        assert_eq!(empty.verdict(true), None);
        assert_eq!(empty.verdict(false), None);
    }

    /// The RGB stepper marks exactly the channel it is editing.
    #[test]
    fn the_channel_marker_points_at_one_channel() {
        assert_eq!(mark(true), ">");
        assert_eq!(mark(false), " ");
    }
}
