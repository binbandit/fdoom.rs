//! Contract tests for the split sprite tree (`assets/sprites/**`) and the atlas
//! stitcher (`SpriteSheet::from_parts`). Successor to the artgen-era `artgen_sheet`
//! test: instead of auditing a generated monolith, it locks in
//!
//! - the **golden atlas**: stitching the tree reproduces `assets/golden_atlas.png`
//!   byte-for-byte, so cell-addressed render call sites keep seeing the exact art
//!   they saw before the sheet was decomposed;
//! - **manifest integrity**: every pin has a file of the pinned size, pins never
//!   overlap, paths are unique, no file is empty;
//! - **pixel-mode rules**: `pal` files contain only the legal grays 0/85/170/255
//!   (+ transparent) — see docs/ART_GUIDE.md;
//! - **auto-allocation**: unpinned files land on appended rows and are reachable by
//!   name via `sheet.cell("items/berry")`-style lookup.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use fdoom::gfx::SpriteSheet;
use fdoom::gfx::sprite_sheet::{
    CellRect, PartMode, SHEET_CELLS, decode_rgba, parse_manifest, stitch,
};

fn sprites_dir() -> PathBuf {
    fdoom::assets::sprites_dir().expect("assets/sprites/ not found (run from the repo)")
}

fn manifest_text() -> String {
    fs::read_to_string(sprites_dir().join("manifest.txt")).unwrap()
}

fn disk_parts() -> Vec<(String, Vec<u8>)> {
    fdoom::assets::read_sprite_parts(&sprites_dir())
}

fn as_refs(owned: &[(String, Vec<u8>)]) -> Vec<(&str, &[u8])> {
    owned
        .iter()
        .map(|(p, b)| (p.as_str(), b.as_slice()))
        .collect()
}

/// Stitching the sprite tree reproduces the golden atlas byte-for-byte, so every
/// cell-addressed draw call renders exactly the art it did pre-decomposition.
#[test]
fn stitched_atlas_matches_golden() {
    let owned = disk_parts();
    let s = stitch(&manifest_text(), &as_refs(&owned)).unwrap();
    assert_eq!((s.width, s.height), (256, 256), "unexpected atlas size");

    let golden_png = fs::read(sprites_dir().parent().unwrap().join("golden_atlas.png")).unwrap();
    let (gw, gh, golden) = decode_rgba(&golden_png).unwrap();
    assert_eq!((gw, gh), (256, 256), "golden fixture size");
    assert!(
        s.rgba == golden,
        "stitched atlas differs from assets/golden_atlas.png — if an art change is \
         intentional, regenerate the golden (see docs/ART_GUIDE.md)"
    );
}

/// Every manifest pin has a file of the pinned size; pins are unique, in-bounds
/// (parse_manifest enforces bounds) and never overlap; no sprite file is empty.
#[test]
fn manifest_integrity() {
    let entries = parse_manifest(&manifest_text()).unwrap();
    let owned = disk_parts();
    let files: HashMap<&str, &[u8]> = owned
        .iter()
        .map(|(p, b)| (p.as_str(), b.as_slice()))
        .collect();

    let mut owner: HashMap<(i32, i32), &str> = HashMap::new();
    let mut seen = HashMap::new();
    for e in &entries {
        assert!(
            seen.insert(e.path.clone(), ()).is_none(),
            "duplicate manifest entry {}",
            e.path
        );
        let bytes = files
            .get(e.path.as_str())
            .unwrap_or_else(|| panic!("manifest entry {} has no file", e.path));
        let (w, h, rgba) = decode_rgba(bytes).unwrap();
        assert_eq!(
            (w, h),
            (e.rect.w * 8, e.rect.h * 8),
            "{}: file size does not match its manifest pin",
            e.path
        );
        assert!(
            rgba.chunks_exact(4).any(|p| p[3] >= 128),
            "{}: sprite file is fully transparent",
            e.path
        );
        for dy in 0..e.rect.h {
            for dx in 0..e.rect.w {
                if let Some(other) = owner.insert((e.rect.x + dx, e.rect.y + dy), &e.path) {
                    panic!(
                        "pin overlap at cell ({},{}): {} vs {}",
                        e.rect.x + dx,
                        e.rect.y + dy,
                        other,
                        e.path
                    );
                }
            }
        }
    }
}

/// `pal` files hold palette-mode art: every opaque pixel must be one of the four
/// legal grays (0/85/170/255) so the `/64` shade quantization stays exact. `rgb`
/// files are unconstrained (they may even mix in palette grays deliberately).
#[test]
fn palette_files_use_legal_grays() {
    let entries = parse_manifest(&manifest_text()).unwrap();
    let dir = sprites_dir();
    for e in entries.iter().filter(|e| e.mode == PartMode::Palette) {
        let (w, _, rgba) = decode_rgba(&fs::read(dir.join(&e.path)).unwrap()).unwrap();
        for (i, p) in rgba.chunks_exact(4).enumerate() {
            if p[3] < 128 {
                continue;
            }
            assert!(
                p[0] == p[1] && p[1] == p[2] && matches!(p[0], 0 | 85 | 170 | 255),
                "{}: pixel ({},{}) is {:?} — pal files may only use grays 0/85/170/255",
                e.path,
                i as i32 % w,
                i as i32 / w,
                &p[..3]
            );
        }
    }
}

/// Pinned sprites are reachable by name, and the name -> pos mapping matches the
/// historical cell addresses render call sites use.
#[test]
fn name_lookup_resolves_pins() {
    let owned = disk_parts();
    let sheet = SpriteSheet::from_parts(&manifest_text(), &as_refs(&owned));
    let berry = sheet.cell("items/berry").expect("items/berry");
    assert_eq!(
        berry,
        CellRect {
            x: 11,
            y: 10,
            w: 1,
            h: 1
        }
    );
    assert_eq!(berry.pos(), 11 + 10 * 32);
    let walk = sheet.cell("mobs/player/walk").expect("mobs/player/walk");
    assert_eq!((walk.x, walk.y, walk.w, walk.h), (0, 14, 8, 2));
    assert!(sheet.cell("font/a").is_some());
    assert!(sheet.cell("no/such/sprite").is_none());
}

/// New art needs no manifest edit: a file that is not pinned auto-allocates onto an
/// appended row (>= row 32), grows the atlas, and resolves by name.
#[test]
fn unpinned_files_auto_allocate_appended_rows() {
    // a 16x8 all-true-color part, built in memory
    let mut png_bytes = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png_bytes, 16, 8);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let px: Vec<u8> = std::iter::repeat_n([200u8, 40, 120, 255], 16 * 8)
            .flatten()
            .collect();
        enc.write_header().unwrap().write_image_data(&px).unwrap();
    }
    let mut owned = disk_parts();
    owned.push(("items/zzz_atlas_test.png".to_string(), png_bytes));

    let sheet = SpriteSheet::from_parts(&manifest_text(), &as_refs(&owned));
    let rect = sheet
        .cell("items/zzz_atlas_test")
        .expect("auto-allocated sprite must be resolvable by name");
    assert!(
        rect.y >= SHEET_CELLS,
        "auto-allocated art must land below the pinned 32-row grid, got {rect:?}"
    );
    assert_eq!((rect.w, rect.h), (2, 1));
    assert!(
        sheet.height >= (rect.y + rect.h) * 8,
        "atlas height must grow to fit appended rows"
    );
    // and the pixels actually landed there
    use fdoom::gfx::sprite_sheet::SheetPixel;
    let px = sheet.pixels[(rect.y * 8 * sheet.width + rect.x * 8) as usize];
    assert_eq!(px, SheetPixel::Rgb(0xC82878));
}

/// The build-time embedded copy (release / out-of-repo fallback) tracks the folder.
#[test]
fn embedded_copy_matches_disk() {
    assert_eq!(
        fdoom::assets::EMBEDDED_SPRITE_MANIFEST,
        manifest_text(),
        "embedded manifest is stale — cargo should have rerun build.rs"
    );
    let owned = disk_parts();
    let embedded: HashMap<&str, &[u8]> = fdoom::assets::EMBEDDED_SPRITE_PARTS
        .iter()
        .copied()
        .collect();
    assert_eq!(embedded.len(), owned.len(), "embedded part count differs");
    for (path, bytes) in &owned {
        assert_eq!(
            embedded.get(path.as_str()),
            Some(&bytes.as_slice()),
            "embedded copy of {path} differs from disk"
        );
    }
}

/// Writes the stitched atlas to `target/verify/atlas.png` for eyeballing
/// (`just preview` opens it).
#[test]
fn write_atlas_preview() {
    let owned = disk_parts();
    let s = stitch(&manifest_text(), &as_refs(&owned)).unwrap();
    let path = fdoom::testutil::verify_path("atlas.png");
    let file = fs::File::create(&path).unwrap();
    let mut enc = png::Encoder::new(
        std::io::BufWriter::new(file),
        s.width as u32,
        s.height as u32,
    );
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .unwrap()
        .write_image_data(&s.rgba)
        .unwrap();
    println!("wrote {}", path.display());
}
