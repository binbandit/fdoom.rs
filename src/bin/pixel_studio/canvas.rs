//! Canvas mode's document model: the whole sprite tree stitched into one editable
//! image, plus the routing that gets edits back out to the files they belong to.
//!
//! The stitch uses the game's own `sprite_sheet::stitch`, so what canvas mode shows
//! *is* the real atlas layout — manifest pins on the base grid, new art on the
//! auto-allocated rows below. Every canvas pixel belongs to at most one file: the
//! per-8px-cell `owner` index answers "who owns this pixel", [`Placement`] tracks
//! each file's rectangle and dirty flag, and [`canvas_extract`] cuts a file back out
//! for saving. Gaps between placements are owned by nobody and never save.

use std::path::{Path, PathBuf};

use fdoom::gfx::sprite_sheet;

use crate::atlas::CELL;
use crate::color::Rgba;
use crate::image::Image;
use crate::library::walk;

/// One source file's rectangle on the stitched canvas. Canvas mode edits the whole
/// tree as a single image; every paint routes back to its owning file via these.
pub(crate) struct Placement {
    pub(crate) rel: String, // manifest-relative path, e.g. "tiles/grass.png"
    pub(crate) path: PathBuf,
    pub(crate) x: i32, // px on the canvas
    pub(crate) y: i32,
    pub(crate) w: i32,
    pub(crate) h: i32,
    pub(crate) dirty: bool,
}

/// The stitched canvas: one image, the per-file placements, and the owner index.
pub(crate) struct Stitched {
    pub(crate) img: Image,
    pub(crate) placements: Vec<Placement>,
    /// Placement index per 8px canvas cell, -1 for gaps (row-major, `img.w/8` wide).
    pub(crate) owner: Vec<i32>,
}

/// Read every `*.png` under `root` as stitcher input, in browser order.
fn collect_parts(root: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut owned: Vec<(String, Vec<u8>)> = Vec::new();
    for e in walk(root) {
        if e.is_dir {
            continue;
        }
        let bytes = std::fs::read(&e.path).map_err(|err| format!("{}: {err}", e.rel))?;
        owned.push((e.rel, bytes));
    }
    if owned.is_empty() {
        return Err(format!("no *.png files under {}", root.display()));
    }
    Ok(owned)
}

/// Map every 8px cell of the canvas to the placement that owns it (-1 = gap).
fn owner_index(img: &Image, placements: &[Placement]) -> Vec<i32> {
    let cols = img.w / CELL;
    let mut owner = vec![-1i32; (cols * (img.h / CELL)) as usize];
    for (i, p) in placements.iter().enumerate() {
        for cy in p.y / CELL..(p.y + p.h) / CELL {
            for cx in p.x / CELL..(p.x + p.w) / CELL {
                owner[(cx + cy * cols) as usize] = i as i32;
            }
        }
    }
    owner
}

/// Stitch every `*.png` under `root` into one canvas using the game's own stitcher
/// (`sprite_sheet::stitch`), so canvas mode shows the **real atlas layout**: pinned
/// files at their manifest cells, new art on the auto-allocated rows below.
pub(crate) fn build_canvas(root: &Path) -> Result<Stitched, String> {
    let manifest = std::fs::read_to_string(root.join("manifest.txt")).unwrap_or_default();
    let owned = collect_parts(root)?;
    let parts: Vec<(&str, &[u8])> = owned
        .iter()
        .map(|(p, b)| (p.as_str(), b.as_slice()))
        .collect();
    let st = sprite_sheet::stitch(&manifest, &parts)?;
    let px: Vec<Rgba> = st
        .rgba
        .chunks_exact(4)
        .map(|c| [c[0], c[1], c[2], c[3]])
        .collect();
    let img = Image {
        w: st.width,
        h: st.height,
        px,
    };
    let mut placements: Vec<Placement> = st
        .cells
        .iter()
        .map(|(name, r)| {
            let rel = format!("{name}.png");
            Placement {
                path: root.join(&rel),
                rel,
                x: r.x * CELL,
                y: r.y * CELL,
                w: r.w * CELL,
                h: r.h * CELL,
                dirty: false,
            }
        })
        .collect();
    placements.sort_by(|a, b| (a.y, a.x, &a.rel).cmp(&(b.y, b.x, &b.rel)));
    let owner = owner_index(&img, &placements);
    Ok(Stitched {
        img,
        placements,
        owner,
    })
}

/// The placement owning canvas pixel `(x, y)`, if any. Coordinates outside the
/// canvas own nothing — without that check an x past the right edge would wrap onto
/// the next cell row and report the wrong file.
pub(crate) fn owner_of(owner: &[i32], img: &Image, x: i32, y: i32) -> Option<usize> {
    if !img.contains(x, y) {
        return None;
    }
    let cols = img.w / CELL;
    match owner[(x / CELL + (y / CELL) * cols) as usize] {
        -1 => None,
        o => Some(o as usize),
    }
}

/// Cut a placement's rectangle back out of the canvas as a standalone image.
pub(crate) fn canvas_extract(img: &Image, p: &Placement) -> Image {
    let mut px = Vec::with_capacity((p.w * p.h) as usize);
    for y in p.y..p.y + p.h {
        for x in p.x..p.x + p.w {
            px.push(img.px[(x + y * img.w) as usize]);
        }
    }
    Image { w: p.w, h: p.h, px }
}

/// Wrap-shift only the `(x, y, w, h)` rect of `img` (canvas-mode nudge: one file).
pub(crate) fn nudge_rect(img: &mut Image, x: i32, y: i32, w: i32, h: i32, dx: i32, dy: i32) {
    let mut buf = vec![[0u8; 4]; (w * h) as usize];
    for yy in 0..h {
        for xx in 0..w {
            buf[(xx + yy * w) as usize] = img.px[(x + xx + (y + yy) * img.w) as usize];
        }
    }
    for yy in 0..h {
        for xx in 0..w {
            let (nx, ny) = ((xx + dx).rem_euclid(w), (yy + dy).rem_euclid(h));
            img.px[(x + nx + (y + ny) * img.w) as usize] = buf[(xx + yy * w) as usize];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(x: i32, y: i32, w: i32, h: i32) -> Placement {
        Placement {
            rel: format!("f{x}_{y}.png"),
            path: PathBuf::from("f.png"),
            x,
            y,
            w,
            h,
            dirty: false,
        }
    }

    fn ramp(w: i32, h: i32) -> Image {
        let mut img = Image::blank(w, h);
        for y in 0..h {
            for x in 0..w {
                img.px[(x + y * w) as usize] = [x as u8, y as u8, 0, 255];
            }
        }
        img
    }

    /// Extraction cuts exactly the placement's rectangle, at its own local origin.
    #[test]
    fn extract_cuts_the_placement_rect() {
        let img = ramp(32, 32);
        let p = placement(8, 16, 16, 8);
        let out = canvas_extract(&img, &p);
        assert_eq!((out.w, out.h), (16, 8));
        assert_eq!(out.pixel(0, 0), [8, 16, 0, 255], "local 0,0 is canvas 8,16");
        assert_eq!(out.pixel(15, 7), [23, 23, 0, 255]);
    }

    /// The owner index resolves every pixel of a placement to it, and gaps to nobody.
    #[test]
    fn owner_index_covers_placements_and_leaves_gaps() {
        let img = Image::blank(32, 32);
        let ps = vec![placement(0, 0, 16, 16), placement(16, 0, 8, 8)];
        let owner = owner_index(&img, &ps);

        assert_eq!(owner_of(&owner, &img, 0, 0), Some(0));
        assert_eq!(owner_of(&owner, &img, 15, 15), Some(0), "last px of file 0");
        assert_eq!(owner_of(&owner, &img, 16, 0), Some(1));
        assert_eq!(owner_of(&owner, &img, 23, 7), Some(1));
        assert_eq!(owner_of(&owner, &img, 24, 0), None, "gap to the right");
        assert_eq!(owner_of(&owner, &img, 0, 16), None, "gap below");
        assert_eq!(owner_of(&owner, &img, 999, 999), None, "off the canvas");
        assert_eq!(owner_of(&owner, &img, -1, 0), None, "negative");
        // past the right edge must not wrap onto the next cell row
        assert_eq!(owner_of(&owner, &img, 40, 0), None, "past the right edge");
    }

    /// A per-file nudge wraps inside its own rect and never touches a neighbour.
    #[test]
    fn nudge_rect_wraps_within_one_file_only() {
        let mut img = ramp(32, 16);
        let untouched: Vec<Rgba> = (0..16).map(|x| img.pixel(16 + x, 0)).collect();

        nudge_rect(&mut img, 0, 0, 16, 16, 1, 0);
        assert_eq!(img.pixel(1, 0), [0, 0, 0, 255], "column 0 moved right");
        assert_eq!(img.pixel(0, 0), [15, 0, 0, 255], "column 15 wrapped around");

        let after: Vec<Rgba> = (0..16).map(|x| img.pixel(16 + x, 0)).collect();
        assert_eq!(after, untouched, "the neighbouring file is untouched");
    }

    /// A full-period per-file nudge is the identity, in both axes at once.
    #[test]
    fn nudge_rect_full_period_is_identity() {
        let mut img = ramp(32, 32);
        let before = img.px.clone();
        nudge_rect(&mut img, 8, 8, 16, 16, 16, 16);
        assert_eq!(img.px, before);
    }
}
