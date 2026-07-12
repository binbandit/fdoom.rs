//! Biome ground-identity guards (2026-07 playtest follow-up): the Mountains belt
//! interior carries its own highland ground instead of plain grass, and Marsh
//! interiors actually read marshy (pools, mud, reeds, scraggly willows). Bands are
//! deliberately loose — they guard identity, not exact noise statistics.

use std::collections::HashMap;

use fdoom::level::chunk::CHUNK_SIZE;
use fdoom::level::infinite_gen::{self, Biome};
use fdoom::level::tile::Tiles;

/// Tile-name histogram over every tile of `biome` (blended lookup, i.e. what the
/// renderer sees) within the chunk square [-r, r) x [-r, r).
fn composition(seed: i64, biome: Biome, r: i32) -> (u32, HashMap<String, u32>) {
    let tiles = Tiles::new();
    let mut counts = HashMap::new();
    let mut n = 0u32;
    for cy in -r..r {
        for cx in -r..r {
            let chunk = infinite_gen::generate_chunk(seed, 0, cx, cy, &tiles);
            for ly in 0..CHUNK_SIZE {
                for lx in 0..CHUNK_SIZE {
                    let (x, y) = (cx * CHUNK_SIZE + lx, cy * CHUNK_SIZE + ly);
                    if infinite_gen::biome_at_blended(seed, x, y) != biome {
                        continue;
                    }
                    n += 1;
                    let id = chunk.tiles[(lx + ly * CHUNK_SIZE) as usize];
                    *counts
                        .entry(tiles.get_id(id as i32).name.clone())
                        .or_insert(0) += 1;
                }
            }
        }
    }
    (n, counts)
}

fn frac(counts: &HashMap<String, u32>, n: u32, name: &str) -> f64 {
    f64::from(counts.get(name).copied().unwrap_or(0)) / f64::from(n)
}

/// Marsh interiors must read marshy: a healthy wet fraction (pools + mud), reed
/// banks, and the odd scraggly willow — never a plain grass field. Seed 9 is the
/// playtest seed.
#[test]
fn marsh_interior_reads_marshy() {
    for seed in [9i64, 1234, 42] {
        let (n, c) = composition(seed, Biome::Marsh, 16);
        assert!(n > 5_000, "seed {seed}: too few marsh tiles sampled ({n})");
        let wet = frac(&c, n, "WATER") + frac(&c, n, "MUD");
        assert!(
            (0.15..0.50).contains(&wet),
            "seed {seed}: marsh wet fraction {wet:.3} outside [0.15, 0.50)"
        );
        let reeds = frac(&c, n, "REEDS");
        assert!(
            reeds > 0.03,
            "seed {seed}: marsh reed fraction {reeds:.3} <= 0.03"
        );
        assert!(
            c.get("WILLOW").copied().unwrap_or(0) > 0,
            "seed {seed}: no willows in marsh"
        );
        // never a plain grass field: grass must not dominate the biome
        let grass = frac(&c, n, "GRASS");
        assert!(
            grass < 0.62,
            "seed {seed}: marsh is {grass:.3} plain grass — reads like plains"
        );
    }
}

/// Mountains must carry their own ground: the belt interior is heath (with rock
/// crags and snow summits), and plain grass never leaks into the biome — the
/// playtest's "green grass + boulder blobs" failure mode.
#[test]
fn mountains_interior_has_highland_ground() {
    for seed in [9i64, 1234, 42] {
        let (n, c) = composition(seed, Biome::Mountains, 16);
        assert!(
            n > 5_000,
            "seed {seed}: too few mountain tiles sampled ({n})"
        );
        let heath = frac(&c, n, "HEATH");
        assert!(
            heath > 0.20,
            "seed {seed}: heath fraction {heath:.3} <= 0.20 — no highland ground"
        );
        let rock = frac(&c, n, "ROCK");
        assert!(
            (0.05..0.75).contains(&rock),
            "seed {seed}: rock fraction {rock:.3} outside [0.05, 0.75) — crags gone or wall-to-wall"
        );
        let grass = frac(&c, n, "GRASS");
        assert!(
            grass < 0.02,
            "seed {seed}: {grass:.3} plain grass inside Mountains"
        );
    }
}

/// The playtest's exact miss: seed 9 (-280, 40) is Forest, not Marsh — the marsh on
/// that row genuinely exists further west. Pins the diagnosis so the map-color fix
/// (tests/biome_frames.rs) has a documented reason.
#[test]
fn playtest_marsh_coordinates_were_forest() {
    assert_eq!(infinite_gen::biome_at(9, -280, 40), Biome::Forest);
    let marsh_on_row = (-400..-300).any(|x| infinite_gen::biome_at(9, x, 40) == Biome::Marsh);
    assert!(
        marsh_on_row,
        "expected real marsh west of the playtest spot"
    );
}

/// Daytime verification screenshots (run with `--ignored`): a mountain heath
/// hillside and a marsh interior with pools in frame, at seed 9.
#[test]
#[ignore]
fn shots_marsh_and_mountains() {
    use fdoom::core::updater::Time;
    use fdoom::testutil::TestWorld;

    // The first tile `goto_biome`'s ring scan hits is an edge tile by construction;
    // walk from it to a point whose ±16-tile box is entirely in-biome (an interior).
    fn interior_of(seed: i64, (fx, fy): (i32, i32), want: Biome) -> (i32, i32) {
        let all_in = |x: i32, y: i32| {
            [(0, 0), (-16, -16), (16, -16), (-16, 16), (16, 16)]
                .iter()
                .all(|(dx, dy)| infinite_gen::biome_at(seed, x + dx, y + dy) == want)
        };
        for r in 0..60 {
            for dy in (-r..=r).step_by(2) {
                for dx in (-r..=r).step_by(2) {
                    if all_in(fx + dx * 4, fy + dy * 4) {
                        return (fx + dx * 4, fy + dy * 4);
                    }
                }
            }
        }
        (fx, fy)
    }
    // Center the frame on a tile of `name` near (tx, ty), on the surface level.
    fn center_on(tw: &mut TestWorld, (tx, ty): (i32, i32), name: &str) {
        let lvl = tw.g.current_level;
        for r in 0..30 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if tw.g.tile_at(lvl, tx + dx, ty + dy).name == name {
                        tw.teleport(tx + dx, ty + dy);
                        return;
                    }
                }
            }
        }
    }

    let mut tw = TestWorld::infinite().seed(9).name("biome_identity").build();
    tw.g.change_time_of_day(Time::Day);
    let seed = tw.g.world_seed;

    // mountains: an interior hillside, frame centered on open heath
    let found = tw.goto_biome(Biome::Mountains);
    let (mx, my) = interior_of(seed, found, Biome::Mountains);
    tw.teleport(mx, my);
    tw.tick_n(8);
    center_on(&mut tw, (mx, my), "HEATH");
    tw.tick_n(10);
    tw.screenshot("identity_mountain_heath.png");

    // marsh: an interior, frame centered on a pool rim
    let found = tw.goto_biome(Biome::Marsh);
    let (sx, sy) = interior_of(seed, found, Biome::Marsh);
    tw.teleport(sx, sy);
    tw.tick_n(8);
    center_on(&mut tw, (sx, sy), "MUD");
    tw.tick_n(10);
    tw.screenshot("identity_marsh_interior.png");
}

/// Diagnostic sweep (run with `--ignored --nocapture`): biome frequencies and
/// full marsh/mountains compositions for a few seeds.
#[test]
#[ignore]
fn diag_marsh_and_mountains() {
    for seed in [9i64, 1234, 42] {
        let mut counts = HashMap::new();
        let n = 1024;
        for y in (-n..n).step_by(4) {
            for x in (-n..n).step_by(4) {
                *counts
                    .entry(format!("{:?}", infinite_gen::biome_at(seed, x, y)))
                    .or_insert(0u32) += 1;
            }
        }
        println!("seed {seed} biome frequencies: {counts:?}");
        for biome in [Biome::Marsh, Biome::Mountains] {
            let (n, c) = composition(seed, biome, 16);
            println!("seed {seed} {biome:?}: {n} tiles, composition {c:?}");
        }
    }
}
