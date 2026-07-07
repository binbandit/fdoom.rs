//! Headless rendering smoke tests: draw into the software framebuffer with the real
//! sprite sheet and dump PNGs to target/verify for visual inspection.

use std::sync::Arc;

use fdoom::gfx::{
    color, font,
    screen::{self, Screen},
};
use fdoom::testutil::{save_png, verify_path};

#[test]
fn sheet_loads_and_text_renders() {
    let sheet = Arc::new(fdoom::assets::sprite_sheet());
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

    save_png(
        verify_path("font_smoke.png"),
        &s.pixels,
        screen::W as usize,
        screen::H as usize,
        1,
    );
}
