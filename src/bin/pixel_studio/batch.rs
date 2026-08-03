//! Headless editing: `--new`, `--set`, `--blit` and `--nudge` with no window.
//!
//! These take the same code paths as the interactive editor — same pixel writes, same
//! back-up-once-then-save, same canvas routing — so a scripted edit and a hand-drawn
//! one are indistinguishable on disk. That is what makes them safe to use from CI and
//! from other tools.
//!
//! Each runner reports what it wrote on stdout and exits; anything it cannot do is a
//! non-zero exit with a reason, never a silent no-op.

use std::path::Path;

use crate::atlas::CELL;
use crate::canvas::{Placement, build_canvas, canvas_extract};
use crate::cli::BatchOp;
use crate::image::{Image, bak_path, blit_rect, load_png, nudge_image, write_png};
use crate::library::is_legal_sprite_name;

/// `WxH` as `--new` spells it: whole 8px cells, no wider than the atlas.
fn parse_size_spec(s: &str) -> Option<(i32, i32)> {
    let (w, h) = s.split_once(['x', 'X'])?;
    let (w, h) = (w.parse::<i32>().ok()?, h.parse::<i32>().ok()?);
    (w > 0 && h > 0 && w % 8 == 0 && h % 8 == 0 && w <= 256).then_some((w, h))
}

/// `--new <rel-no-ext> WxH`: create a blank transparent sprite in the tree and exit.
pub(crate) fn run_new_sprite(target: &Path, rel: &str, size: &str) {
    let Some((w, h)) = parse_size_spec(size) else {
        eprintln!("--new size must be WxH, multiples of 8, width <= 256 (got {size:?})");
        std::process::exit(2);
    };
    let rel = rel.trim_matches('/').trim_end_matches(".png").to_string();
    if rel.is_empty() || !is_legal_sprite_name(&rel) {
        eprintln!("--new name: lowercase letters, digits, _ - and / only (got {rel:?})");
        std::process::exit(2);
    }
    let path = target.join(format!("{rel}.png"));
    if path.exists() {
        eprintln!("--new: {} already exists", path.display());
        std::process::exit(2);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create sprite folder");
    }
    if let Err(e) = write_png(&path, &Image::blank(w, h)) {
        eprintln!("--new: {e}");
        std::process::exit(1);
    }
    println!(
        "created {} ({w}x{h}, transparent) — remember to add it to the UNPINNED list in tests/sprite_atlas.rs",
        path.display()
    );
}

/// `--canvas --set/--blit`: edits in stitched-canvas coordinates route to their
/// owning files; only the touched files are rewritten (each with a `.bak`).
pub(crate) fn run_canvas_batch(target: &Path, batch: &[BatchOp]) {
    let st = match build_canvas(target) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("pixel_studio --canvas: {e}");
            std::process::exit(1);
        }
    };
    let (mut img, mut placements, owner) = (st.img, st.placements, st.owner);
    let cols = img.w / CELL;
    let mark = |placements: &mut Vec<Placement>, x: i32, y: i32| {
        if let Some(&o) = owner.get((x / CELL + (y / CELL) * cols) as usize)
            && o >= 0
        {
            placements[o as usize].dirty = true;
        }
    };
    for op in batch {
        match *op {
            BatchOp::Set(x, y, c) => {
                if !img.contains(x, y) {
                    eprintln!("--set {x} {y}: outside the {}x{} canvas", img.w, img.h);
                    std::process::exit(2);
                }
                img.px[(x + y * img.w) as usize] = c;
                mark(&mut placements, x, y);
            }
            BatchOp::Blit(sx, sy, w, h, dx, dy) => {
                blit_rect(&mut img, sx, sy, w, h, dx, dy);
                for y in dy.max(0)..(dy + h).min(img.h) {
                    for x in dx.max(0)..(dx + w).min(img.w) {
                        mark(&mut placements, x, y);
                    }
                }
            }
            BatchOp::Nudge(..) => {
                eprintln!("--nudge is per-file; run it on the file, not --canvas");
                std::process::exit(2);
            }
        }
    }
    let mut wrote = 0;
    for p in placements.iter().filter(|p| p.dirty) {
        let bak = bak_path(&p.path);
        if let Err(e) = std::fs::copy(&p.path, &bak) {
            eprintln!("backup failed for {}: {e}", p.rel);
            std::process::exit(1);
        }
        if let Err(e) = write_png(&p.path, &canvas_extract(&img, p)) {
            eprintln!("save failed for {}: {e}", p.rel);
            std::process::exit(1);
        }
        println!("wrote {} (backup at {})", p.path.display(), bak.display());
        wrote += 1;
    }
    println!("{wrote} file(s) written from canvas edits");
}

/// `--set/--blit/--nudge` on one PNG: apply in argument order, back up, save, exit.
pub(crate) fn run_file_batch(path: &Path, img: &mut Image, batch: &[BatchOp]) {
    for op in batch {
        match *op {
            BatchOp::Set(x, y, c) => {
                if !img.contains(x, y) {
                    eprintln!("--set {x} {y}: out of bounds ({}x{})", img.w, img.h);
                    std::process::exit(2);
                }
                img.px[(x + y * img.w) as usize] = c;
            }
            BatchOp::Blit(sx, sy, w, h, dx, dy) => blit_rect(img, sx, sy, w, h, dx, dy),
            BatchOp::Nudge(dx, dy) => nudge_image(img, dx, dy),
        }
    }
    let bak = bak_path(path);
    if let Err(e) = std::fs::copy(path, &bak) {
        eprintln!("backup failed: {e}");
        std::process::exit(1);
    }
    if let Err(e) = write_png(path, img) {
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
}

/// Load the PNG the session will edit, or exit with the reason.
pub(crate) fn open_or_exit(path: &Path) -> Image {
    match load_png(path) {
        Ok(img) => img,
        Err(e) => {
            eprintln!("pixel_studio: {e}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The documented sizes parse, in either case of the separator.
    #[test]
    fn size_specs_accept_whole_cells() {
        assert_eq!(parse_size_spec("8x8"), Some((8, 8)));
        assert_eq!(parse_size_spec("64X16"), Some((64, 16)));
        assert_eq!(parse_size_spec("256x256"), Some((256, 256)));
        assert_eq!(parse_size_spec("32x8"), Some((32, 8)));
    }

    /// Off-grid, over-wide and malformed sizes are refused — art whose dimensions
    /// are not multiples of 8 cannot be addressed on the atlas at all.
    #[test]
    fn size_specs_refuse_what_the_atlas_cannot_hold() {
        for bad in [
            "7x8",   // not a whole cell
            "8x7",   //
            "264x8", // wider than the atlas
            "0x8",   // empty
            "8x0",   //
            "-8x8",  // negative
            "8",     // no separator
            "8y8",   // wrong separator
            "axb",   // not numbers
            "",      //
            "8x8x8", // trailing junk
        ] {
            assert_eq!(parse_size_spec(bad), None, "{bad:?} should be refused");
        }
    }
}
