//! Visual check: teleport across biomes in an infinite world and dump rendered frames.

use std::sync::Arc;

use fdoom::core::renderer::Renderer;
use fdoom::core::{game::Game, world};
use fdoom::gfx::SpriteSheet;
use fdoom::level::infinite_gen::{Biome, biome_at};

fn find_biome(seed: i64, want: Biome) -> Option<(i32, i32)> {
    for r in 0i32..600 {
        let ring = r * 8;
        for dy in (-ring..=ring).step_by(8) {
            for dx in (-ring..=ring).step_by(8) {
                if (dx.abs() == ring || dy.abs() == ring) && biome_at(seed, dx, dy) == want {
                    return Some((dx, dy));
                }
            }
        }
    }
    None
}

#[test]
fn frames_across_biomes() {
    let tmp = std::env::temp_dir().join("fdoom_biome_frames");
    let _ = std::fs::remove_dir_all(&tmp);
    let mut g = Game::new(false, false, tmp);
    world::reset_game(&mut g, true);
    g.settings.set("worldtype", "Infinite");
    g.world_name = "biomes".into();
    g.world_seed = 20260707;
    world::init_world(&mut g);
    g.tick();
    g.has_gui = true; // let the renderer draw in headless mode

    let mut r = Renderer::new(Arc::new(SpriteSheet::from_png(fdoom::assets::SPRITES_PNG)));
    let seed = g.world_seed;

    for (biome, name) in [
        (Biome::Tundra, "tundra"),
        (Biome::Desert, "desert"),
        (Biome::Marsh, "marsh"),
        (Biome::Forest, "forest"),
        (Biome::Mountains, "mountains"),
        (Biome::Beach, "beach"),
    ] {
        let Some((tx, ty)) = find_biome(seed, biome) else {
            panic!("no {name} within range");
        };
        {
            let p = g.player_mut();
            p.c.x = tx * 16 + 8;
            p.c.y = ty * 16 + 8;
        }
        for _ in 0..6 {
            g.tick(); // stream chunks + settle
        }
        r.render(&mut g);
        let dir = std::path::Path::new("target/verify");
        std::fs::create_dir_all(dir).unwrap();
        let file = std::fs::File::create(dir.join(format!("biome_{name}.png"))).unwrap();
        let mut enc = png::Encoder::new(
            std::io::BufWriter::new(file),
            fdoom::gfx::screen::W as u32,
            fdoom::gfx::screen::H as u32,
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
}
