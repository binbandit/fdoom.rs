//! HUD QOL checks: the held-tool durability bar (green -> yellow -> red) and the
//! smoked-glass notification backing. Renders real frames headlessly and asserts on
//! the framebuffer; PNGs land in target/verify for visual inspection.

use fdoom::gfx::screen;
use fdoom::item::ItemKind;
use fdoom::testutil::TestWorld;

/// Sample the durability bar's fill pixel (left end of the gauge, always filled while
/// dur > 0). Coordinates match render_gui's bar placement.
fn bar_pixel(pixels: &[i32]) -> (i32, i32, i32) {
    let p = pixels[(17 * screen::W + 96) as usize];
    ((p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF)
}

#[test]
fn durability_bar_traffic_lights() {
    let mut tw = TestWorld::infinite().name("hud").build();

    let tool = fdoom::item::registry::get(&tw, "Crude Pickaxe"); // max dur 28 * 1
    for (dur, name, check) in [
        (28, "hud_dur_full.png", "green"),
        (10, "hud_dur_mid.png", "yellow"),
        (3, "hud_dur_low.png", "red"),
    ] {
        let mut t = tool.clone();
        if let ItemKind::Tool { dur: d, .. } = &mut t.kind {
            *d = dur;
        }
        tw.player_mut().player_mut().active_item = Some(t);
        let pixels = tw.render();
        tw.screenshot(name);

        let (red, green, _blue) = bar_pixel(&pixels);
        match check {
            "green" => assert!(
                green > red,
                "full bar should be green, got rgb {red},{green}"
            ),
            "yellow" => assert!(
                green > 100 && red > 100,
                "mid bar should be yellow, got rgb {red},{green}"
            ),
            "red" => assert!(red > green, "low bar should be red, got rgb {red},{green}"),
            _ => unreachable!(),
        }
    }

    // no bar without a tool held: the exact fill colors must vanish from the gauge spot
    let fills = [
        fdoom::gfx::color::upgrade(fdoom::gfx::color::get_byte(140)),
        fdoom::gfx::color::upgrade(fdoom::gfx::color::get_byte(540)),
        fdoom::gfx::color::upgrade(fdoom::gfx::color::get_byte(500)),
    ];
    tw.player_mut().player_mut().active_item = None;
    let pixels = tw.render();
    let p = pixels[(17 * screen::W + 96) as usize];
    assert!(
        !fills.contains(&p),
        "no tool held: bar area should show the plain frame, got {p:#x}"
    );
}

#[test]
fn notifications_get_a_backing_band() {
    let mut tw = TestWorld::infinite().name("hud_note").build();

    // render once without a notification, once with; the band must darken the backdrop.
    // The paragraph is anchored at (W/2, H*2/5) with RelPos::Top, i.e. the text block
    // sits just above the anchor — sample inside the band, left of the text.
    let y = screen::H * 2 / 5 - 4;
    let x = screen::W / 2 - 44;
    let before = tw.render()[(y * screen::W + x) as usize];

    tw.notify_all("World Saved!");
    let pixels = tw.render();
    tw.screenshot("hud_notification.png");
    let after = pixels[(y * screen::W + x) as usize];

    // compare summed channels: the band multiplies every channel down unless the pixel
    // is covered by the (brighter) text itself — sample a spot between glyph rows
    let sum = |p: i32| ((p >> 16) & 0xFF) + ((p >> 8) & 0xFF) + (p & 0xFF);
    assert!(
        sum(after) != sum(before),
        "notification band should change the backdrop (before={before:#x} after={after:#x})"
    );
}
