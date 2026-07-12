//! Screenshot scenes for the HANDOFF 2d art follow-ups: tiny mushroom clusters,
//! flower species variety, dedicated wet-sand tidal cells, the timber-prop frame,
//! and the new item icons (pan / window / prop / big fish / cave eel).
//!
//! These are verification dumps (target/verify/art2d_*.png) plus cheap presence
//! asserts — the real acceptance is a human reading the shots.

use fdoom::core::updater::DAY_LENGTH;
use fdoom::level::chunk::CHUNK_SIZE;
use fdoom::level::infinite_gen::{Biome, biome_at, generate_chunk};
use fdoom::level::tile::Tiles;
use fdoom::level::tile::tidal::is_submerged;
use fdoom::testutil::TestWorld;

const SEED: i64 = 20260711;
const SURFACE: usize = 3; // lvl_idx(0)

/// Pin the clock to mid-day so shots are lit neutrally (settling advances it).
fn pin_noon(tw: &mut TestWorld) {
    tw.set_time(DAY_LENGTH * 3 / 8);
    tw.tick_n(2);
    tw.set_time(DAY_LENGTH * 3 / 8);
}

/// Find a Tidal Flat tile near a coast by pure generation (as in tests/tides.rs).
fn find_tidal_tile(tiles: &Tiles) -> (i32, i32) {
    let tidal_id = tiles.get("Tidal Flat").id;
    for radius in 0..140i32 {
        for cy in -radius..=radius {
            for cx in -radius..=radius {
                if cx.abs() != radius && cy.abs() != radius {
                    continue; // ring only
                }
                let (x, y) = (
                    cx * CHUNK_SIZE + CHUNK_SIZE / 2,
                    cy * CHUNK_SIZE + CHUNK_SIZE / 2,
                );
                if !matches!(biome_at(SEED, x, y), Biome::Ocean | Biome::Beach) {
                    continue;
                }
                let chunk = generate_chunk(SEED, 0, cx, cy, tiles);
                for (i, &t) in chunk.tiles.iter().enumerate() {
                    if t == tidal_id {
                        let lx = i as i32 % CHUNK_SIZE;
                        let ly = i as i32 / CHUNK_SIZE;
                        return (cx * CHUNK_SIZE + lx, cy * CHUNK_SIZE + ly);
                    }
                }
            }
        }
    }
    panic!("no tidal flat found on any coast within the ring sweep");
}

/// A patch of mushroom tiles: several tiny caps per tile, mirror-varied per tile.
#[test]
fn shot_mushroom_patch() {
    let mut tw = TestWorld::infinite()
        .seed(20260711)
        .name("art2d_shroom")
        .build();
    tw.tick_n(6);
    pin_noon(&mut tw);
    let (px, py) = tw.player_tile();
    for (dx, dy) in [
        (-2, -1),
        (-1, -1),
        (-1, 0),
        (0, -2),
        (1, -1),
        (2, 0),
        (0, 1),
    ] {
        tw.place_at("Mushroom", px + dx, py + dy);
    }
    tw.screenshot("art2d_mushrooms.png");
}

/// A flower meadow: three species (daisy / poppy / cornflower) mixed by position.
#[test]
fn shot_flower_meadow() {
    let mut tw = TestWorld::infinite()
        .seed(20260711)
        .name("art2d_flower")
        .build();
    tw.tick_n(6);
    pin_noon(&mut tw);
    let (px, py) = tw.player_tile();
    for dx in -4i32..=4 {
        for dy in -3i32..=2 {
            if (dx, dy) == (0, 0) || (dx + dy).rem_euclid(2) == 0 {
                continue;
            }
            tw.place_at("flower", px + dx, py + dy);
        }
    }
    tw.screenshot("art2d_flowers.png");
}

/// Timber props over dirt: header beam + open middle, not a solid crate.
#[test]
fn shot_timber_props() {
    let mut tw = TestWorld::infinite()
        .seed(20260711)
        .name("art2d_prop")
        .build();
    tw.tick_n(6);
    pin_noon(&mut tw);
    let (px, py) = tw.player_tile();
    for (dx, dy) in [(-2, -1), (0, -2), (2, -1)] {
        tw.place_at("dirt", px + dx, py + dy);
        tw.place_at("Timber Prop", px + dx, py + dy);
    }
    tw.screenshot("art2d_timber_props.png");
}

/// The tidal flat at low tide: exposed wet sand with the dedicated texture.
#[test]
fn shot_wet_sand_low_tide() {
    let mut tw = TestWorld::infinite().seed(SEED).name("art2d_tidal").build();
    let (fx, fy) = find_tidal_tile(&tw.tiles);
    // stand on the *dry* beach strip above the band, so the frame shows the
    // dry-sand -> wet-sand -> water progression side by side
    let (mut sx, mut sy) = (fx, fy);
    'dry: for r in 1..24i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs().max(dy.abs()) != r {
                    continue;
                }
                let land = fdoom::level::infinite_gen::land_at(SEED, fx + dx, fy + dy);
                if (0.437..0.443).contains(&land) {
                    (sx, sy) = (fx + dx, fy + dy);
                    break 'dry;
                }
            }
        }
    }
    tw.teleport(sx, sy);
    tw.tick_n(6);
    // A/B pair: high tide (band under water) vs low tide (band exposed as wet
    // sand) — the diff shows exactly which tiles carry the new wet-sand cells.
    tw.set_time(DAY_LENGTH / 2);
    tw.tick_n(1);
    tw.set_time(DAY_LENGTH / 2);
    assert!(
        is_submerged(&tw, fx, fy),
        "flat should be submerged at high tide"
    );
    tw.screenshot("art2d_wet_sand_hightide.png");
    tw.set_time(DAY_LENGTH / 4);
    tw.tick_n(1);
    tw.set_time(DAY_LENGTH / 4);
    assert!(
        !is_submerged(&tw, fx, fy),
        "flat should be exposed at low tide"
    );
    assert_eq!(tw.tile_at(SURFACE, fx, fy).name, "TIDAL FLAT");
    tw.screenshot("art2d_wet_sand.png");
    // and the hero angle: noon mid-tide, waterline mid-band — dry beach, wet
    // band and open water in one frame
    pin_noon(&mut tw);
    tw.screenshot("art2d_wet_sand_midtide.png");
}

/// The new item icons in the inventory list.
#[test]
fn shot_new_item_icons() {
    let mut tw = TestWorld::infinite()
        .seed(20260711)
        .name("art2d_icons")
        .build();
    tw.tick_n(6);
    pin_noon(&mut tw);
    for item in [
        "Prospector's Pan",
        "Window",
        "Timber Prop",
        "Big Fish",
        "Cooked Big Fish",
        "Cave Eel",
        "Cooked Cave Eel",
    ] {
        tw.give(item, 1);
    }
    // open the inventory like the I key does (see tests/display_flow.rs)
    let pid = tw.player_id;
    let player = tw.entities.take(pid).unwrap();
    let inv_display = fdoom::screen::player_inv_display::PlayerInvDisplay::new(&tw, &player);
    tw.entities.put_back(player);
    tw.set_menu(inv_display);
    tw.tick();
    assert!(tw.display.menu_active(), "inventory should be open");
    tw.screenshot("art2d_icons.png");
}
