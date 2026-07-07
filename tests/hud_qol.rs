//! HUD QOL checks: the held-tool durability bar (green -> yellow -> red) and the
//! smoked-glass notification backing. Renders real frames headlessly and asserts on
//! the framebuffer; PNGs land in target/verify for visual inspection.

use std::sync::Arc;

use fdoom::core::renderer::Renderer;
use fdoom::core::{game::Game, world};
use fdoom::gfx::SpriteSheet;
use fdoom::gfx::screen;
use fdoom::item::ItemKind;

fn dump_png(name: &str, r: &Renderer) {
    let dir = std::path::Path::new("target/verify");
    std::fs::create_dir_all(dir).unwrap();
    let file = std::fs::File::create(dir.join(name)).unwrap();
    let mut enc = png::Encoder::new(
        std::io::BufWriter::new(file),
        screen::W as u32,
        screen::H as u32,
    );
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().unwrap();
    let mut data = Vec::new();
    for &p in &r.screen.pixels {
        data.extend_from_slice(&[
            ((p >> 16) & 0xff) as u8,
            ((p >> 8) & 0xff) as u8,
            (p & 0xff) as u8,
        ]);
    }
    w.write_image_data(&data).unwrap();
}

fn boot() -> Game {
    let tmp = std::env::temp_dir().join("fdoom_hud_qol");
    let _ = std::fs::remove_dir_all(&tmp);
    let mut g = Game::new(false, false, tmp);
    world::reset_game(&mut g, true);
    g.world_name = "hud".into();
    g.world_seed = 20260707;
    world::init_world(&mut g);
    g.tick();
    g.has_gui = true; // let the renderer draw in headless mode
    g
}

/// Sample the durability bar's fill pixel (left end of the gauge, always filled while
/// dur > 0). Coordinates match render_gui's bar placement.
fn bar_pixel(r: &Renderer) -> (i32, i32, i32) {
    let p = r.screen.pixels[(17 * screen::W + 96) as usize];
    ((p >> 16) & 0xFF, (p >> 8) & 0xFF, p & 0xFF)
}

#[test]
fn durability_bar_traffic_lights() {
    let mut g = boot();
    let mut r = Renderer::new(Arc::new(SpriteSheet::from_png(fdoom::assets::SPRITES_PNG)));

    let tool = fdoom::item::registry::get(&g, "Crude Pickaxe"); // max dur 28 * 1
    for (dur, name, check) in [
        (28, "hud_dur_full.png", "green"),
        (10, "hud_dur_mid.png", "yellow"),
        (3, "hud_dur_low.png", "red"),
    ] {
        let mut t = tool.clone();
        if let ItemKind::Tool { dur: d, .. } = &mut t.kind {
            *d = dur;
        }
        g.player_mut().player_mut().active_item = Some(t);
        r.render(&mut g);
        dump_png(name, &r);

        let (red, green, _blue) = bar_pixel(&r);
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
    g.player_mut().player_mut().active_item = None;
    r.render(&mut g);
    let p = r.screen.pixels[(17 * screen::W + 96) as usize];
    assert!(
        !fills.contains(&p),
        "no tool held: bar area should show the plain frame, got {p:#x}"
    );
}

#[test]
fn notifications_get_a_backing_band() {
    let mut g = boot();
    let mut r = Renderer::new(Arc::new(SpriteSheet::from_png(fdoom::assets::SPRITES_PNG)));

    // render once without a notification, once with; the band must darken the backdrop.
    // The paragraph is anchored at (W/2, H*2/5) with RelPos::Top, i.e. the text block
    // sits just above the anchor — sample inside the band, left of the text.
    r.render(&mut g);
    let y = screen::H * 2 / 5 - 4;
    let x = screen::W / 2 - 44;
    let before = r.screen.pixels[(y * screen::W + x) as usize];

    g.notify_all("World Saved!");
    r.render(&mut g);
    dump_png("hud_notification.png", &r);
    let after = r.screen.pixels[(y * screen::W + x) as usize];

    // compare summed channels: the band multiplies every channel down unless the pixel
    // is covered by the (brighter) text itself — sample a spot between glyph rows
    let sum = |p: i32| ((p >> 16) & 0xFF) + ((p >> 8) & 0xFF) + (p & 0xFF);
    assert!(
        sum(after) != sum(before),
        "notification band should change the backdrop (before={before:#x} after={after:#x})"
    );
}
