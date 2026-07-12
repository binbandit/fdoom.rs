use std::sync::Arc;

use fdoom::gfx::screen::{H, Screen, W};
use fdoom::platform::logical_size_for_window;

#[test]
fn integer_scale_and_logical_size_follow_the_window_contract() {
    assert_eq!(logical_size_for_window(800, 600), (2, 400, 300));
    assert_eq!(logical_size_for_window(288, 192), (1, 288, 192));
    assert_eq!(logical_size_for_window(4000, 3000), (6, 640, 400));
    assert_eq!(logical_size_for_window(200, 100), (1, 288, 192));
}

#[test]
fn runtime_screen_keeps_classic_constructor_and_allocates_requested_size() {
    let sheet = Arc::new(fdoom::assets::sprite_sheet());
    let classic = Screen::new(sheet.clone());
    assert_eq!(
        (classic.w, classic.h, classic.pixels.len()),
        (W, H, (W * H) as usize)
    );

    let wide = Screen::with_size(384, 240, sheet);
    assert_eq!((wide.w, wide.h, wide.pixels.len()), (384, 240, 384 * 240));
    assert_eq!(wide.center().x, 192);
    assert_eq!(wide.center().y, 120);
}
