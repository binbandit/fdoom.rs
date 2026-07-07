//! Round-trip tests for the pixel_studio headless batch editor (`--set`).
//!
//! Drives the real binary (`CARGO_BIN_EXE_pixel_studio`) against temp copies, then
//! re-decodes the result through `SpriteSheet::from_png` — the game's own loader —
//! so the assertions check the semantics the renderer will actually see
//! (palette gray / true color / transparent).

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

fn run(args: &[&str]) {
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
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/sprites.png");
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
