//! Forest canopy connectors: trees standing together render merged canopy pieces
//! (per-quarter edge strips, inner corners, varied interior fill) instead of a grid
//! of identical lone trees — same-family neighbors only, render-side only.

use fdoom::gfx::screen;
use fdoom::level::infinite_gen::Biome;
use fdoom::testutil::TestWorld;

/// Framebuffer rect (16x16) of the tile at player + `(dx, dy)`. The camera centers
/// the player at (W/2, (H-8)/2), so with the player teleported to a tile center the
/// tile at offset `(dx, dy)` lands at exactly (dx*16 + W/2 - 8, dy*16 + (H-8)/2 - 8).
fn tile_rect(pixels: &[i32], dx: i32, dy: i32) -> Vec<i32> {
    let x0 = dx * 16 + screen::W / 2 - 8;
    let y0 = dy * 16 + (screen::H - 8) / 2 - 8;
    let mut out = Vec::with_capacity(256);
    for y in y0..y0 + 16 {
        for x in x0..x0 + 16 {
            out.push(pixels[(x + y * screen::W) as usize]);
        }
    }
    out
}

/// Clear a grass apron around the player so staged patches sit on known ground.
fn grass_apron(tw: &mut TestWorld) {
    let (px, py) = tw.player_tile();
    for dy in -6..=6 {
        for dx in -9..=9 {
            tw.place_at("grass", px + dx, py + dy);
        }
    }
}

fn daylight_world() -> TestWorld {
    let mut tw = TestWorld::infinite().seed(42).build();
    tw.change_time_of_day(fdoom::core::updater::Time::Day);
    tw.goto_biome(Biome::Plains);
    grass_apron(&mut tw);
    tw
}

/// Place a rectangle of trees with its top-left at player + `(dx, dy)`.
fn tree_block(tw: &mut TestWorld, dx: i32, dy: i32, w: i32, h: i32) {
    let (px, py) = tw.player_tile();
    for y in 0..h {
        for x in 0..w {
            tw.place_at("tree", px + dx + x, py + dy + y);
        }
    }
}

#[test]
fn interior_canopy_differs_from_lone_tree() {
    let mut tw = daylight_world();
    tw.place("tree", -6, -3); // lone
    tree_block(&mut tw, 2, -4, 3, 3); // interior tile at (3, -3)
    let px = tw.render();

    let lone = tile_rect(&px, -6, -3);
    let interior = tile_rect(&px, 3, -3);
    let grass = tile_rect(&px, 0, 3);
    assert_ne!(lone, grass, "lone tree did not render");
    assert_ne!(interior, grass, "canopy interior did not render");
    assert_ne!(
        lone, interior,
        "a fully surrounded tree must render dense canopy, not the lone silhouette"
    );
}

#[test]
fn boundary_edges_differ_from_interior() {
    let mut tw = daylight_world();
    tree_block(&mut tw, -1, -4, 3, 3); // center/interior tile at (0, -3)
    let px = tw.render();

    let interior = tile_rect(&px, 0, -3);
    let west_edge = tile_rect(&px, -1, -3);
    let corner = tile_rect(&px, -1, -4);
    assert_ne!(
        west_edge, interior,
        "a canopy border tile must keep an outer silhouette edge"
    );
    assert_ne!(
        corner, interior,
        "a canopy corner must differ from the interior"
    );
    assert_ne!(
        corner, west_edge,
        "corner and straight edge read differently"
    );
}

#[test]
fn south_edge_keeps_trunks() {
    let mut tw = daylight_world();
    tree_block(&mut tw, -1, -3, 2, 1); // horizontal pair
    let px = tw.render();
    // lit trunk brown from the traced classic cells, (122, 85, 54) — matched with a
    // small tolerance because ambient contact shadows nudge framebuffer values
    let is_trunk = |p: i32| {
        let (r, g, b) = (p >> 16 & 0xff, p >> 8 & 0xff, p & 0xff);
        (r - 122).abs() <= 6 && (g - 85).abs() <= 6 && (b - 54).abs() <= 6
    };
    for dx in [-1, 0] {
        let rect = tile_rect(&px, dx, -3);
        assert!(
            rect.iter().any(|&p| is_trunk(p)),
            "tree at dx={dx} in a merged pair lost its south-face trunk"
        );
    }
}

#[test]
fn mixed_species_do_not_connect() {
    let mut tw = daylight_world();
    tw.place("tree", 1, -3);
    let before = tile_rect(&tw.render(), 1, -3);

    // A willow next door must not change the classic tree's render. (Willow rather
    // than pine: it shares the grass ground base, so the pre-existing ground seam
    // blending stays out of the comparison.)
    tw.place("Willow", 2, -3);
    let with_willow = tile_rect(&tw.render(), 1, -3);
    assert_eq!(
        before, with_willow,
        "a willow neighbor must not connect to a classic tree"
    );

    // ...while a fellow classic tree does
    tw.place("tree", 2, -3);
    let with_tree = tile_rect(&tw.render(), 1, -3);
    assert_ne!(
        before, with_tree,
        "same-family neighbors must merge canopies"
    );
}

#[test]
fn canopy_render_is_deterministic() {
    let stage = |tw: &mut TestWorld| {
        grass_apron(tw);
        tree_block(tw, -2, -4, 4, 3);
        tw.render()
    };
    let mut a = daylight_world();
    let mut b = daylight_world();
    let pa = stage(&mut a);
    let pb = stage(&mut b);
    for dy in -4..=-2 {
        for dx in -2..=1 {
            assert_eq!(
                tile_rect(&pa, dx, dy),
                tile_rect(&pb, dx, dy),
                "canopy cell choice must be pure f(seed, x, y) — tile ({dx}, {dy})"
            );
        }
    }
}

/// Screenshot gallery for the art review: staged clumps plus the densest natural
/// forest and pine-wood patches near the spawn biome anchors (`target/verify/`).
#[test]
fn canopy_showcase_screenshots() {
    let mut tw = TestWorld::infinite().seed(42).build();
    tw.change_time_of_day(fdoom::core::updater::Time::Day);
    tw.goto_biome(Biome::Plains);
    grass_apron(&mut tw);

    // lone / pair / L-trio / 5x4 block on one staged screen
    let (px, py) = tw.player_tile();
    tw.place_at("tree", px - 8, py - 3);
    tw.place_at("tree", px - 5, py - 3);
    tw.place_at("tree", px - 4, py - 3);
    tw.place_at("tree", px - 1, py - 3);
    tw.place_at("tree", px, py - 3);
    tw.place_at("tree", px, py - 2);
    tree_block(&mut tw, 3, -5, 5, 4);
    tw.clear_notifications();
    tw.screenshot("canopy_clumps.png");

    // staged pine grove on snow: lone, pair, and a 3x3 stand
    let (px, py) = tw.player_tile();
    for dy in -6..=6 {
        for dx in -9..=9 {
            tw.place_at("snow", px + dx, py + dy);
        }
    }
    tw.place_at("Pine Tree", px - 7, py - 3);
    tw.place_at("Pine Tree", px - 3, py - 3);
    tw.place_at("Pine Tree", px - 2, py - 3);
    for dy in 0..3 {
        for dx in 0..3 {
            tw.place_at("Pine Tree", px + 2 + dx, py - 5 + dy);
        }
    }
    tw.clear_notifications();
    tw.screenshot("canopy_pine_grove.png");

    // densest natural patches: classic forest and the pine fringe
    let hunt = |tw: &mut TestWorld, name: &str, want: Biome| {
        let (ax, ay) = fdoom::testutil::find_biome(tw.world_seed, want);
        let (mut bx, mut by, mut best) = (ax, ay, -1);
        for cy in (ay - 200..=ay + 200).step_by(20) {
            for cx in (ax - 200..=ax + 200).step_by(20) {
                tw.teleport(cx, cy);
                tw.tick_n(4);
                let lvl = tw.current_level;
                let count = (-5..=5)
                    .flat_map(|dy| (-8..=8).map(move |dx| (dx, dy)))
                    .filter(|&(dx, dy)| tw.tile_at(lvl, cx + dx, cy + dy).name == name)
                    .count() as i32;
                if count > best {
                    (bx, by, best) = (cx, cy, count);
                }
            }
        }
        tw.teleport(bx, by);
        tw.tick_n(8);
        best
    };

    let n = hunt(&mut tw, "TREE", Biome::Forest);
    assert!(n > 8, "no dense classic-forest patch found near the anchor");
    tw.clear_notifications();
    tw.screenshot("canopy_forest.png");

    // pine woods grow where forest meets the cold climate gate, so the best pine
    // stands sit around the tundra anchor's fringe
    let n = hunt(&mut tw, "PINE TREE", Biome::Tundra);
    tw.clear_notifications();
    tw.screenshot("canopy_pines.png");
    assert!(n > 2, "no pine stand found near the tundra anchor");
}
