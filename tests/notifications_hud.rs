//! Playtest #2 + #9: notification tiers (ambient ticker vs centered warning band vs
//! bottom-right save toast), menu suppression, and the held-item panel (name clipping
//! and the empty-hands dash). Renders real frames headlessly and asserts on the
//! framebuffer; PNGs land in target/verify for visual inspection.

use fdoom::gfx::screen;
use fdoom::screen::pause_display::PauseDisplay;
use fdoom::testutil::TestWorld;

fn px(pixels: &[i32], x: i32, y: i32) -> i32 {
    pixels[(y * screen::W + x) as usize]
}

/// Inside the ambient ticker's first-line backing (renderer: TICKER_X=4, TICKER_Y=3 —
/// flush to the top-left edge since the corner-HUD relayout removed the frame boxes
/// the ticker used to dock under; UI_REDESIGN §2 L1).
const TICKER_PROBE: (i32, i32) = (6, 4);
/// Inside the centered warning band for a ~20-char message (same spot the old
/// hud_qol band test sampled: within the darkened band, left of the glyph column).
fn band_probe() -> (i32, i32) {
    (screen::W / 2 - 44, screen::H * 2 / 5 - 4)
}

#[test]
fn ambient_and_warning_route_to_different_places() {
    let mut tw = TestWorld::infinite().name("nh_route").build();
    let base = tw.render();
    let (bx, by) = band_probe();
    let (tx, ty) = TICKER_PROBE;

    // Ambient: draws the top-left ticker, leaves the band area untouched.
    tw.notify_all("Gave 5 x Wood");
    let amb = tw.render();
    tw.screenshot("nh_ambient.png");
    assert_ne!(
        px(&amb, tx, ty),
        px(&base, tx, ty),
        "ambient note should paint the top-left ticker"
    );
    assert_eq!(
        px(&amb, bx, by),
        px(&base, bx, by),
        "ambient note must not paint the centered band"
    );

    // Warning: draws the centered band, leaves the ticker area untouched.
    tw.clear_notifications();
    tw.push_warning("The ceiling groans...");
    let warn = tw.render();
    tw.screenshot("nh_warning.png");
    assert_ne!(
        px(&warn, bx, by),
        px(&base, bx, by),
        "warning should paint the centered band"
    );
    assert_eq!(
        px(&warn, tx, ty),
        px(&base, tx, ty),
        "warning must not paint the ambient ticker"
    );
}

#[test]
fn notifications_hold_while_a_menu_is_open() {
    let mut tw = TestWorld::infinite().name("nh_menu").build();
    let base = tw.render();
    let (bx, by) = band_probe();

    let pause = PauseDisplay::new(&tw.g);
    tw.g.display.stack.push(Box::new(pause));
    let menu_plain = tw.render();

    tw.push_warning("The ceiling groans...");
    tw.notify_all("Gave 5 x Wood");
    let menu_noted = tw.render();
    tw.screenshot("nh_menu_suppressed.png");
    assert_eq!(
        menu_plain, menu_noted,
        "notifications must not render (or age) while a menu is open"
    );

    // After the menu closes, the held warning resumes on the band.
    tw.g.display.stack.clear();
    let resumed = tw.render();
    assert_ne!(
        px(&resumed, bx, by),
        px(&base, bx, by),
        "held warning should resume after the menu closes"
    );
}

#[test]
fn save_toast_sits_bottom_right() {
    let mut tw = TestWorld::infinite().name("nh_toast").build();
    let base = tw.render();
    let (bx, by) = band_probe();

    // Live progress while saving...
    tw.g.saving = true;
    tw.g.loading_percentage = 62.0;
    let saving = tw.render();
    tw.screenshot("nh_saving.png");
    tw.g.saving = false;

    // ...then the toast pushed by the save path.
    tw.push_toast("World Saved!");
    let toast = tw.render();
    tw.screenshot("nh_toast.png");

    // Both live in the bottom-right corner strip, not on the centered band.
    // "World Saved!" is 12 chars: text from W-100 to W-5, backing rows H-13..H-3.
    let (cx, cy) = (screen::W - 30, screen::H - 8);
    for (name, frame) in [("saving progress", &saving), ("toast", &toast)] {
        assert_ne!(
            px(frame, cx, cy),
            px(&base, cx, cy),
            "{name} should paint the bottom-right corner"
        );
        assert_eq!(
            px(frame, bx, by),
            px(&base, bx, by),
            "{name} must not paint the centered band"
        );
    }
}

#[test]
fn held_item_name_never_bleeds_out_of_its_corner() {
    let mut tw = TestWorld::infinite().name("nh_clip").build();

    // Corner HUD: the held item is an icon plate bottom-right plus a transient
    // right-aligned name label (UI_REDESIGN §2 L1). Both test items are stackables
    // (same plate furniture: badge but no durability bar), so any pixel difference
    // outside the plate/badge/label corner could only come from name overflow.
    let pan = fdoom::item::registry::get(&tw, "Prospector's Pan");
    tw.player_mut().player_mut().active_item = Some(pan);
    let long = tw.render();
    tw.screenshot("nh_pan.png");

    let wood = fdoom::item::registry::get(&tw, "Wood");
    tw.player_mut().player_mut().active_item = Some(wood);
    let short = tw.render();

    // The label band starts at y=145 (backing top); everything above it, and
    // everything left of the plate column below it, must not depend on the name.
    for y in 0..145 {
        for x in 0..screen::W {
            assert_eq!(
                px(&long, x, y),
                px(&short, x, y),
                "held-item name bled out of the label band at ({x},{y})"
            );
        }
    }
    for y in 145..screen::H {
        for x in 0..160 {
            assert_eq!(
                px(&long, x, y),
                px(&short, x, y),
                "held-item name bled left of its corner at ({x},{y})"
            );
        }
    }
}

#[test]
fn empty_hands_show_a_dim_fist() {
    let mut tw = TestWorld::infinite().name("nh_empty").build();
    tw.player_mut().player_mut().active_item = None;
    let pixels = tw.render();
    tw.screenshot("nh_empty.png");

    // The plate interior (267..283, 171..187) must hold the fist glyph: without it
    // the interior is only the smoked-glass darken, which never yields the fist's
    // two flat grays.
    let cell: Vec<i32> = (171..187)
        .flat_map(|y| (267..283).map(move |x| (x, y)))
        .map(|(x, y)| px(&pixels, x, y))
        .collect();
    assert!(
        cell.contains(&0x3A3A3A) && cell.contains(&0x5A5A5A),
        "empty-hands fist missing from the held plate"
    );
}

/// Stages the review screenshots: a 3-line ambient ticker mid-play plus the pieces
/// covered above (band, toast, pan, empty hands write their own PNGs).
#[test]
fn ticker_stacks_newest_on_top() {
    let mut tw = TestWorld::infinite().name("nh_ticker").build();
    let base = tw.render();

    tw.notify_all("The campfire dies to embers");
    tw.notify_all("Gave 5 x Wood");
    tw.notify_all("Dig a hole first!");
    let ticker = tw.render();
    tw.screenshot("nh_ticker3.png");

    // Three stacked 9px rows at the top-left edge: each row's backing must darken
    // the frame.
    for row in 0..3 {
        let (x, y) = (6, 4 + row * 9);
        assert_ne!(
            px(&ticker, x, y),
            px(&base, x, y),
            "ticker row {row} missing"
        );
    }

    // Only 3 lines ever survive: pushing a fourth drops the oldest.
    tw.notify_all("A fourth message");
    tw.render();
    assert_eq!(tw.g.notifications.len(), 3, "ticker must cap at 3 lines");
    assert_eq!(
        tw.g.notifications[0], "Gave 5 x Wood",
        "oldest line should have been dropped"
    );
}
