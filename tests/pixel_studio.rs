//! Round-trip tests for the pixel_studio headless surface: batch edits (`--set`,
//! `--blit`, `--nudge`), stitched-canvas multi-file edits (`--canvas`), the
//! new-sprite flow (`--new`), and the odd-origin selection report (`--snap`).
//!
//! Drives the real binary (`CARGO_BIN_EXE_pixel_studio`) against temp copies, then
//! re-decodes the result through `SpriteSheet::from_png` — the game's own loader —
//! so the assertions check the semantics the renderer will actually see
//! (palette gray / true color / transparent). Never points at the real
//! `assets/sprites` tree: every test builds its own temp folder.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fdoom::gfx::SpriteSheet;
use fdoom::gfx::sprite_sheet::SheetPixel;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pixel_studio_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_pixel_studio"))
        .args(args)
        .output()
        .expect("run pixel_studio");
    assert!(
        out.status.success(),
        "pixel_studio {args:?} failed:\n{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn run_expect_fail(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_pixel_studio"))
        .args(args)
        .output()
        .expect("run pixel_studio");
    assert!(
        !out.status.success(),
        "pixel_studio {args:?} unexpectedly succeeded"
    );
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// Write a small RGBA PNG filled with one color (an identifiable sprite stand-in).
fn write_filled_png(path: &Path, w: u32, h: u32, rgba: [u8; 4]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().unwrap();
    let data: Vec<u8> = rgba
        .iter()
        .copied()
        .cycle()
        .take((w * h * 4) as usize)
        .collect();
    writer.write_image_data(&data).unwrap();
}

/// Write a tiny all-transparent RGBA PNG (a stand-in for a split sprite file).
fn write_blank_png(path: &Path, w: u32, h: u32) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().unwrap();
    writer
        .write_image_data(&vec![0u8; (w * h * 4) as usize])
        .unwrap();
}

/// Sheet mode: `--set` edits exactly the requested pixels of a monolithic sheet,
/// leaves every other pixel untouched, and drops a byte-identical `.bak.png`.
#[test]
fn batch_set_roundtrips_on_sheet() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/golden_atlas.png");
    let dir = temp_dir("sheet");
    let sheet = dir.join("sheet.png");
    fs::copy(&src, &sheet).unwrap();

    // one true-color, one transparent, one palette-gray (85 -> shade 1) edit
    run(&[
        sheet.to_str().unwrap(),
        "--set",
        "3",
        "5",
        "FF00AA",
        "--set",
        "10",
        "5",
        "t",
        "--set",
        "11",
        "5",
        "555555",
    ]);

    let before = SpriteSheet::from_png(&fs::read(&src).unwrap());
    let after = SpriteSheet::from_png(&fs::read(&sheet).unwrap());
    assert_eq!((before.width, before.height), (after.width, after.height));

    let idx = |x: i32, y: i32| (x + y * after.width) as usize;
    assert_eq!(after.pixels[idx(3, 5)], SheetPixel::Rgb(0xFF00AA));
    assert_eq!(after.pixels[idx(10, 5)], SheetPixel::Transparent);
    assert_eq!(after.pixels[idx(11, 5)], SheetPixel::Palette(1));

    let edited = [idx(3, 5), idx(10, 5), idx(11, 5)];
    for i in 0..after.pixels.len() {
        if !edited.contains(&i) {
            assert_eq!(before.pixels[i], after.pixels[i], "pixel {i} changed");
        }
    }

    // first save of a session backs up the original, byte for byte
    assert_eq!(
        fs::read(dir.join("sheet.bak.png")).unwrap(),
        fs::read(&src).unwrap()
    );
    fs::remove_dir_all(&dir).ok();
}

/// Dir mode: the browser walk resolves `--file` inside a split-sprites tree
/// (the upcoming `assets/sprites/**` world), edits it, and backs it up in place.
#[test]
fn batch_set_roundtrips_in_sprite_tree() {
    let root = temp_dir("tree").join("sprites");
    write_blank_png(&root.join("tiles/grass.png"), 16, 16);
    write_blank_png(&root.join("items/sword.png"), 8, 8);
    write_blank_png(&root.join("mobs/zombie/walk.png"), 32, 16);

    run(&[
        root.to_str().unwrap(),
        "--file",
        "mobs/zombie/walk.png",
        "--set",
        "17",
        "9",
        "22AA55",
        "--set",
        "0",
        "0",
        "FFFFFF",
    ]);

    let after = SpriteSheet::from_png(&fs::read(root.join("mobs/zombie/walk.png")).unwrap());
    assert_eq!((after.width, after.height), (32, 16));
    assert_eq!(after.pixels[17 + 9 * 32], SheetPixel::Rgb(0x22AA55));
    assert_eq!(after.pixels[0], SheetPixel::Palette(3)); // white gray = shade 3
    // everything else still transparent
    let edited = [17 + 9 * 32, 0];
    for (i, p) in after.pixels.iter().enumerate() {
        if !edited.contains(&i) {
            assert_eq!(*p, SheetPixel::Transparent, "pixel {i} changed");
        }
    }

    // the untouched neighbors were not written, the edited file was backed up
    assert!(root.join("mobs/zombie/walk.bak.png").exists());
    assert!(!root.join("tiles/grass.bak.png").exists());
    let grass = SpriteSheet::from_png(&fs::read(root.join("tiles/grass.png")).unwrap());
    assert!(grass.pixels.iter().all(|p| *p == SheetPixel::Transparent));

    fs::remove_dir_all(root.parent().unwrap()).ok();
}

/// Build the canvas-mode fixture: three pinned files so the stitched-canvas
/// coordinates are known (left at cells 0,0 / right at 2,0 / spare at 0,2).
fn canvas_tree(name: &str) -> PathBuf {
    let root = temp_dir(name).join("sprites");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("manifest.txt"),
        "tiles/left.png 0 0 2 2 rgb\ntiles/right.png 2 0 2 2 rgb\nitems/spare.png 0 2 1 1 rgb\n",
    )
    .unwrap();
    write_blank_png(&root.join("tiles/left.png"), 16, 16);
    write_blank_png(&root.join("tiles/right.png"), 16, 16);
    write_filled_png(&root.join("items/spare.png"), 8, 8, [200, 40, 40, 255]);
    root
}

/// Canvas mode: one edit spanning two source files saves exactly each file's own
/// pixels; a file that was not touched is not rewritten (byte-identical on disk).
#[test]
fn canvas_edit_spanning_two_files_saves_each_and_leaves_rest_untouched() {
    let root = canvas_tree("canvas_span");
    let spare_before = fs::read(root.join("items/spare.png")).unwrap();

    // left's last column and right's first column: adjacent canvas pixels 15 and 16
    let out = run(&[
        root.to_str().unwrap(),
        "--canvas",
        "--set",
        "15",
        "3",
        "FF0055",
        "--set",
        "16",
        "3",
        "00FF66",
    ]);
    assert!(out.contains("2 file(s) written"), "stdout: {out}");

    let left = SpriteSheet::from_png(&fs::read(root.join("tiles/left.png")).unwrap());
    assert_eq!((left.width, left.height), (16, 16));
    assert_eq!(left.pixels[15 + 3 * 16], SheetPixel::Rgb(0xFF0055));
    assert_eq!(
        left.pixels
            .iter()
            .filter(|p| **p != SheetPixel::Transparent)
            .count(),
        1,
        "left got exactly its own pixel"
    );
    let right = SpriteSheet::from_png(&fs::read(root.join("tiles/right.png")).unwrap());
    assert_eq!(right.pixels[3 * 16], SheetPixel::Rgb(0x00FF66));
    assert_eq!(
        right
            .pixels
            .iter()
            .filter(|p| **p != SheetPixel::Transparent)
            .count(),
        1,
        "right got exactly its own pixel"
    );

    // untouched file: not rewritten (byte-identical), not backed up
    assert_eq!(
        fs::read(root.join("items/spare.png")).unwrap(),
        spare_before
    );
    assert!(!root.join("items/spare.bak.png").exists());
    // touched files were backed up as their originals
    assert!(root.join("tiles/left.bak.png").exists());
    assert!(root.join("tiles/right.bak.png").exists());

    fs::remove_dir_all(root.parent().unwrap()).ok();
}

/// Canvas copy-paste (`--blit` in canvas coords) crosses file boundaries: pixels
/// seeded in one file land in the other, routed to the correct local position.
#[test]
fn canvas_blit_copies_across_file_boundaries() {
    let root = canvas_tree("canvas_blit");

    // seed a 2x2 marker in left at (1,1)-(2,2), then blit that corner onto right
    let out = run(&[
        root.to_str().unwrap(),
        "--canvas",
        "--set",
        "1",
        "1",
        "112233",
        "--set",
        "2",
        "2",
        "445566",
        "--blit",
        "0",
        "0",
        "4",
        "4",
        "20",
        "8",
    ]);
    assert!(out.contains("2 file(s) written"), "stdout: {out}");

    let right = SpriteSheet::from_png(&fs::read(root.join("tiles/right.png")).unwrap());
    // canvas (20,8) = right local (4,8); marker offsets (1,1) and (2,2) follow
    assert_eq!(
        right.pixels[4 + 1 + (8 + 1) * 16],
        SheetPixel::Rgb(0x112233)
    );
    assert_eq!(
        right.pixels[4 + 2 + (8 + 2) * 16],
        SheetPixel::Rgb(0x445566)
    );

    fs::remove_dir_all(root.parent().unwrap()).ok();
}

/// Without a manifest every file is auto-allocated (row 32 and below, path order);
/// canvas edits still route to the right file at the allocated cells.
#[test]
fn canvas_routes_edits_on_auto_allocated_rows() {
    let root = temp_dir("canvas_auto").join("sprites");
    write_blank_png(&root.join("a.png"), 8, 8);
    write_blank_png(&root.join("b.png"), 8, 8);

    // shelf-packed in path order: a at cell (0,32) = px (0,256), b at (8,256)
    run(&[
        root.to_str().unwrap(),
        "--canvas",
        "--set",
        "0",
        "256",
        "FF0000",
        "--set",
        "8",
        "256",
        "00FF00",
    ]);
    let a = SpriteSheet::from_png(&fs::read(root.join("a.png")).unwrap());
    let b = SpriteSheet::from_png(&fs::read(root.join("b.png")).unwrap());
    assert_eq!(a.pixels[0], SheetPixel::Rgb(0xFF0000));
    assert_eq!(b.pixels[0], SheetPixel::Rgb(0x00FF00));

    fs::remove_dir_all(root.parent().unwrap()).ok();
}

/// `--nudge` wrap-shifts the whole image: a corner pixel wraps to the far corner,
/// and a full-period nudge is the identity.
#[test]
fn nudge_wraps_and_full_period_is_identity() {
    let dir = temp_dir("nudge");
    let png = dir.join("n.png");
    write_blank_png(&png, 8, 8);

    run(&[
        png.to_str().unwrap(),
        "--set",
        "0",
        "0",
        "AA00FF",
        "--nudge",
        "-1",
        "-1",
    ]);
    let after = SpriteSheet::from_png(&fs::read(&png).unwrap());
    assert_eq!(
        after.pixels[7 + 7 * 8],
        SheetPixel::Rgb(0xAA00FF),
        "wrapped to far corner"
    );
    assert_eq!(
        after
            .pixels
            .iter()
            .filter(|p| **p != SheetPixel::Transparent)
            .count(),
        1
    );

    let before = fs::read(&png).unwrap();
    run(&[png.to_str().unwrap(), "--nudge", "8", "8"]);
    let cycled = SpriteSheet::from_png(&fs::read(&png).unwrap());
    let orig = SpriteSheet::from_png(&before);
    assert_eq!(
        cycled.pixels, orig.pixels,
        "full-period nudge changed pixels"
    );

    fs::remove_dir_all(&dir).ok();
}

/// `--blit` copies a rect across an 8px cell boundary inside one file, exact stamp
/// semantics (transparency copied too).
#[test]
fn blit_copies_rects_across_cell_boundaries() {
    let dir = temp_dir("blit");
    let png = dir.join("b.png");
    write_blank_png(&png, 16, 16);

    // marker straddles nothing at source (cell 0), destination straddles all 4 cells
    run(&[
        png.to_str().unwrap(),
        "--set",
        "6",
        "6",
        "123456",
        "--set",
        "7",
        "7",
        "654321",
        "--blit",
        "5",
        "5",
        "4",
        "4",
        "9",
        "9",
    ]);
    let img = SpriteSheet::from_png(&fs::read(&png).unwrap());
    assert_eq!(img.pixels[10 + 10 * 16], SheetPixel::Rgb(0x123456)); // (6,6)+(4,4)
    assert_eq!(img.pixels[11 + 11 * 16], SheetPixel::Rgb(0x654321)); // (7,7)+(4,4)
    // originals still present (copy, not move)
    assert_eq!(img.pixels[6 + 6 * 16], SheetPixel::Rgb(0x123456));
    assert_eq!(img.pixels[7 + 7 * 16], SheetPixel::Rgb(0x654321));

    fs::remove_dir_all(&dir).ok();
}

/// Odd-origin selection (the "half tree half pumpkin" bug class): `G`-snapping a
/// multi-cell sprite that starts at an odd cell must select the whole sprite at its
/// true origin, never an even-snapped half. `--snap` prints the resulting window.
#[test]
fn snap_selects_whole_sprites_at_odd_origins() {
    let atlas = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/golden_atlas.png");
    let atlas = atlas.to_str().unwrap();

    // graves: 2x2 blocks at odd cell x (15,11) — hover the right half
    let out = run(&["--sheet", atlas, "--snap", "16", "11"]);
    assert!(out.contains("cell (15, 11)"), "snap drifted: {out}");
    assert!(out.contains("16x16"), "wrong footprint: {out}");
    assert!(out.contains("GRAVE"), "wrong sprite: {out}");

    // decor flora: 2x2 units in a strip starting at odd (15,26) — hover unit 1's
    // right-bottom cell; the window must cover the full unit, at the odd origin
    let out = run(&["--sheet", atlas, "--snap", "16", "27"]);
    assert!(out.contains("cell (15, 26)"), "snap drifted: {out}");
    assert!(out.contains("16x16"), "wrong footprint: {out}");

    // tree species: 2x3 units from (7,26) — hover inside the second unit
    let out = run(&["--sheet", atlas, "--snap", "10", "28"]);
    assert!(out.contains("cell (9, 26)"), "snap drifted: {out}");
    assert!(out.contains("16x24"), "wrong footprint: {out}");
}

/// Canvas-mode snap uses real file placements: a file pinned at an odd cell origin
/// is selected whole, wherever inside it the cursor sits.
#[test]
fn canvas_snap_selects_whole_file_at_odd_origin() {
    let root = temp_dir("canvas_snap").join("sprites");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("manifest.txt"), "tiles/odd.png 3 1 2 2 rgb\n").unwrap();
    write_blank_png(&root.join("tiles/odd.png"), 16, 16);

    // hover the bottom-right cell of the sprite: selection covers the whole file
    let out = run(&[root.to_str().unwrap(), "--canvas", "--snap", "4", "2"]);
    assert!(out.contains("cell (3, 1)"), "snap drifted: {out}");
    assert!(out.contains("16x16"), "wrong footprint: {out}");
    assert!(out.contains("tiles/odd.png"), "wrong file: {out}");

    fs::remove_dir_all(root.parent().unwrap()).ok();
}

/// `--new` creates a blank, transparent, correctly-sized sprite PNG — and refuses
/// bad sizes, bad names, and overwrites.
#[test]
fn new_sprite_creates_blank_png_and_validates() {
    let root = temp_dir("newsprite").join("sprites");
    fs::create_dir_all(&root).unwrap();

    let out = run(&[root.to_str().unwrap(), "--new", "items/moonfruit", "8x8"]);
    assert!(out.contains("created"), "stdout: {out}");
    let img = SpriteSheet::from_png(&fs::read(root.join("items/moonfruit.png")).unwrap());
    assert_eq!((img.width, img.height), (8, 8));
    assert!(img.pixels.iter().all(|p| *p == SheetPixel::Transparent));

    // duplicate, off-grid size, and illegal name all refuse
    run_expect_fail(&[root.to_str().unwrap(), "--new", "items/moonfruit", "8x8"]);
    run_expect_fail(&[root.to_str().unwrap(), "--new", "items/lopside", "7x8"]);
    run_expect_fail(&[root.to_str().unwrap(), "--new", "Items/Bad", "8x8"]);

    fs::remove_dir_all(root.parent().unwrap()).ok();
}
