//! Playtest bug #6 / improvement #10: flora must sit on its biome's real ground.
//! The ground-blend pass used to classify every species tree as grass, so seam
//! blending stippled meadow-green squares into snowfields (under pines) and dunes
//! (under dead trees). These tests render real scenes and assert the green is gone.
//!
//! Set `FLORA_SHOT_DIR=/some/dir` to also dump the rendered frames as PNGs.

use fdoom::core::updater::Time;
use fdoom::gfx::screen;
use fdoom::level::infinite_gen::{Biome, biome_at};
use fdoom::testutil::{TestWorld, find_biome, save_png};

/// Find a `tile` in `biome` whose eight neighbors are all `ground` (a lone prop on
/// open ground), streaming chunks as we search. Returns its tile coordinates.
fn find_lone(tw: &mut TestWorld, seed: i64, biome: Biome, tile: &str, ground: &str) -> (i32, i32) {
    let (bx, by) = find_biome(seed, biome);
    let want = tw.g.tiles.get(tile).id;
    let bed = tw.g.tiles.get(ground).id;
    for r in 0..300i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue;
                }
                let (x, y) = (bx + dx, by + dy);
                if biome_at(seed, x, y) != biome {
                    continue;
                }
                tw.teleport(x, y);
                tw.tick_n(2);
                let lvl = tw.g.current_level;
                if tw.g.tile_at(lvl, x, y).id != want {
                    continue;
                }
                let lone = (-1..=1).all(|ny| {
                    (-1..=1).all(|nx| {
                        (nx == 0 && ny == 0) || tw.g.tile_at(lvl, x + nx, y + ny).id == bed
                    })
                });
                if lone {
                    return (x, y);
                }
            }
        }
    }
    panic!("no lone {tile} on {ground} found in {biome:?} for seed {seed}");
}

/// Render the scene with the player standing two tiles below `(px, py)` and return
/// (pixels, screen x of the tile's left edge, screen y of its top edge).
fn render_at(tw: &mut TestWorld, px: i32, py: i32, shot: &str) -> (Vec<i32>, i32, i32) {
    tw.teleport(px, py + 2);
    tw.tick_n(4);
    let pixels = tw.render();
    if let Ok(dir) = std::env::var("FLORA_SHOT_DIR") {
        let path = std::path::Path::new(&dir).join(shot);
        save_png(&path, &pixels, screen::W as usize, screen::H as usize, 1);
    }
    let (plx, ply) = tw.player_pos();
    let sx = px * 16 - (plx - screen::W / 2);
    let sy = py * 16 - (ply - screen::H / 2);
    (pixels, sx, sy)
}

/// Count pixels in the tile +/- 8 px region matching `bad`.
fn count_region(pixels: &[i32], sx: i32, sy: i32, bad: impl Fn(i32, i32, i32) -> bool) -> usize {
    let (w, h) = (screen::W, screen::H);
    let mut n = 0;
    for dy in -8..24i32 {
        for dx in -8..24i32 {
            let (x, y) = (sx + dx, sy + dy);
            if x < 0 || y < 0 || x >= w || y >= h {
                continue;
            }
            let p = pixels[(y * w + x) as usize];
            let (r, g, b) = ((p >> 16) & 0xff, (p >> 8) & 0xff, p & 0xff);
            if bad(r, g, b) {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn tundra_pine_sits_on_snow_not_grass() {
    let seed = 9;
    let mut tw = TestWorld::infinite().seed(seed).build();
    let (px, py) = find_lone(&mut tw, seed, Biome::Tundra, "Pine Tree", "Snow");
    tw.g.change_time_of_day(Time::Day);
    let (pixels, sx, sy) = render_at(&mut tw, px, py, "tundra_pine.png");

    // grass-green carried into snow reads as a light green (g dominant, but with
    // snow's blue floor still present); pine canopy greens are far darker (b < 100)
    let seam_green = count_region(&pixels, sx, sy, |r, g, b| {
        g > r + 25 && g > b + 25 && b >= 115
    });
    assert_eq!(
        seam_green, 0,
        "tundra pine at ({px},{py}) still bleeds {seam_green} grass-green pixels into the snow"
    );
    // sanity: the pine itself did render (canopy pixels present)
    let canopy = count_region(&pixels, sx, sy, |r, g, b| g > r && g > b && b < 100);
    assert!(canopy > 20, "no pine canopy found at ({px},{py})");
}

#[test]
fn desert_dead_tree_sits_on_sand_not_grass() {
    let seed = 9;
    let mut tw = TestWorld::infinite().seed(seed).build();
    let (px, py) = find_lone(&mut tw, seed, Biome::Desert, "Dead Tree", "Sand");
    tw.g.change_time_of_day(Time::Day);
    let (pixels, sx, sy) = render_at(&mut tw, px, py, "desert_dead_tree.png");

    // a dead tree is bare wood on sand: nothing in this scene may be green at all
    let green = count_region(&pixels, sx, sy, |r, g, b| g > r + 25 && g > b + 25);
    assert_eq!(
        green, 0,
        "desert dead tree at ({px},{py}) still has {green} green pixels around it"
    );
}
