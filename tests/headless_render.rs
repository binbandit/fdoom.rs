//! Headless rendering smoke tests: draw into the software framebuffer with the real
//! sprite sheet and dump PNGs to target/test-frames for visual inspection.

use std::sync::Arc;

use fdoom::gfx::{
    SpriteSheet, color, font,
    screen::{self, Screen},
};

fn dump_png(name: &str, s: &Screen) {
    let dir = std::path::Path::new("target/test-frames");
    std::fs::create_dir_all(dir).unwrap();
    let file = std::fs::File::create(dir.join(name)).unwrap();
    let mut enc = png::Encoder::new(
        std::io::BufWriter::new(file),
        screen::W as u32,
        screen::H as u32,
    );
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().unwrap();
    let mut data = Vec::with_capacity((screen::W * screen::H * 3) as usize);
    for &p in &s.pixels {
        data.push(((p >> 16) & 0xff) as u8);
        data.push(((p >> 8) & 0xff) as u8);
        data.push((p & 0xff) as u8);
    }
    writer.write_image_data(&data).unwrap();
}

#[test]
fn sheet_loads_and_text_renders() {
    let sheet = Arc::new(SpriteSheet::from_png(fdoom::assets::SPRITES_PNG));
    assert!(
        sheet.width >= 256 && sheet.height >= 256,
        "unexpected sheet size {}x{}",
        sheet.width,
        sheet.height
    );

    let mut s = Screen::new(sheet);
    s.clear(0);
    font::draw("HELLO FOSSICKER 0123!?", &mut s, 8, 8, color::WHITE);
    font::draw_centered("CENTERED YELLOW", &mut s, 100, color::YELLOW);
    font::render_frame(&mut s, "TITLE", 2, 4, 20, 10);

    // The buffer must contain non-black pixels where the text was drawn.
    let lit = s.pixels.iter().filter(|&&p| p != 0).count();
    assert!(lit > 500, "expected rendered pixels, got {lit}");

    dump_png("font_smoke.png", &s);
}
