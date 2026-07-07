//! Water tides: the Tidal Flat band between ocean and beach, the day-clock tide
//! curve, submerged/exposed flips, and beachcombing drops.

use fdoom::core::updater::DAY_LENGTH;
use fdoom::entity::EntityKind;
use fdoom::entity::behavior::is_swimming;
use fdoom::level::chunk::CHUNK_SIZE;
use fdoom::level::infinite_gen::{Biome, biome_at, generate_chunk, land_at};
use fdoom::level::tile::tidal::{BAND_HIGH, BAND_LOW, is_submerged, tide_level};
use fdoom::level::tile::{Tiles, dispatch};
use fdoom::testutil::{TestWorld, bare_game};

const SEED: i64 = 20260707; // the TestWorld default, spelled out for the pure-gen tests
const SURFACE: usize = 3; // lvl_idx(0)

/// Find a Tidal Flat tile near a coast by pure generation (no Game needed).
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

/* --------------------------------- generation --------------------------------- */

/// The tidal band generates on coasts, deterministically, and every flat tile's
/// elevation sits inside [BAND_LOW, BAND_HIGH) — the invariant the whole tide
/// mechanic (submersion = land vs tide level) rests on.
#[test]
fn tidal_band_generates_on_coasts_within_band() {
    let tiles = Tiles::new();
    let tidal_id = tiles.get("Tidal Flat").id;
    let (tx, ty) = find_tidal_tile(&tiles);

    // determinism: the coastal chunk regenerates byte-identically
    let (cx, cy) = (tx.div_euclid(CHUNK_SIZE), ty.div_euclid(CHUNK_SIZE));
    let a = generate_chunk(SEED, 0, cx, cy, &tiles);
    let b = generate_chunk(SEED, 0, cx, cy, &tiles);
    assert_eq!(a.tiles, b.tiles, "coastal chunk not deterministic");

    // band purity: every tidal tile in the chunk has land in [BAND_LOW, BAND_HIGH)
    let mut n = 0;
    for (i, &t) in a.tiles.iter().enumerate() {
        if t != tidal_id {
            continue;
        }
        n += 1;
        let x = cx * CHUNK_SIZE + i as i32 % CHUNK_SIZE;
        let y = cy * CHUNK_SIZE + i as i32 / CHUNK_SIZE;
        let land = land_at(SEED, x, y);
        assert!(
            (BAND_LOW..BAND_HIGH).contains(&land),
            "tidal flat at ({x},{y}) has land {land} outside the band"
        );
    }
    assert!(n > 0, "found chunk lost its tidal tiles on regeneration");
}

/* --------------------------------- tide curve --------------------------------- */

/// tide_level stays inside [BAND_LOW, BAND_HIGH], reaches both ends, and completes
/// exactly two full cycles per day (four crossings of the mid level).
#[test]
fn tide_cycles_twice_per_day_and_stays_in_range() {
    let mut g = bare_game("tides_curve");

    let mid = (BAND_LOW + BAND_HIGH) / 2.0;
    let (mut min, mut max) = (f64::MAX, f64::MIN);
    let mut crossings = 0;
    let mut last_side = tide_level(&g) >= mid; // t = 0: high tide
    assert!(last_side, "tide should start high at tick 0");

    for t in (0..DAY_LENGTH).step_by(27) {
        g.set_time(t);
        let tide = tide_level(&g);
        assert!(
            (BAND_LOW - 1e-9..=BAND_HIGH + 1e-9).contains(&tide),
            "tide {tide} out of range at tick {t}"
        );
        min = min.min(tide);
        max = max.max(tide);
        let side = tide >= mid;
        if side != last_side {
            crossings += 1;
            last_side = side;
        }
    }

    assert_eq!(crossings, 4, "two tides per day = four mid-level crossings");
    assert!(min < BAND_LOW + 1e-3, "low tide never reached: min {min}");
    assert!(max > BAND_HIGH - 1e-3, "high tide never reached: max {max}");
}

/* ----------------------------- submerged / exposed ----------------------------- */

/// A fixed tidal tile flips between submerged (high tide) and exposed (low tide)
/// across the day, and a player standing on it counts as swimming only while it's
/// under water.
#[test]
fn fixed_tile_flips_submerged_and_exposed_across_the_day() {
    let mut tw = TestWorld::infinite().build();
    let (tx, ty) = find_tidal_tile(&tw.tiles);

    // stream chunks in around the flat so tile_at sees the real tile
    tw.teleport(tx, ty);
    tw.tick_n(6);
    assert_eq!(
        tw.tile_at(SURFACE, tx, ty).name,
        "TIDAL FLAT",
        "teleport target is not a tidal flat in the live world"
    );

    // high tide (tick 0): every band tile is under water
    tw.set_time(0);
    assert!(is_submerged(&tw, tx, ty), "flat not submerged at high tide");
    {
        let player = tw.entities.get(tw.player_id).unwrap();
        assert!(is_swimming(&tw, player), "player not swimming at high tide");
    }

    // low tide (quarter day): the whole band is exposed and walkable
    tw.set_time(DAY_LENGTH / 4);
    assert!(!is_submerged(&tw, tx, ty), "flat not exposed at low tide");
    {
        let player = tw.entities.get(tw.player_id).unwrap();
        assert!(!is_swimming(&tw, player), "player swimming on dry flat");
    }
}

/* --------------------------------- beachcombing --------------------------------- */

/// Random tile ticks on an exposed flat wash up finds — but never litter: the drop
/// is throttled to at most 2 item entities within 8 tiles.
#[test]
fn beachcombing_drops_are_throttled() {
    let mut tw = TestWorld::infinite().build();
    let def = tw.tiles.get("Tidal Flat");

    // plant a flat on high ground near spawn (land above the band → always exposed,
    // so the test is independent of the clock)
    let (px, py) = tw.player_tile();
    let (tx, ty) = (3..60i32)
        .flat_map(|r| {
            (-r..=r).flat_map(move |d| {
                [
                    (px + d, py - r),
                    (px + d, py + r),
                    (px - r, d + py),
                    (px + r, d + py),
                ]
            })
        })
        .find(|&(x, y)| land_at(SEED, x, y) >= BAND_HIGH + 0.002)
        .expect("no high ground near spawn");
    tw.place_at("Tidal Flat", tx, ty);
    assert!(!is_submerged(&tw, tx, ty));

    let count_nearby = |tw: &TestWorld| {
        tw.entities
            .entities_on_level(SURFACE)
            .filter(|e| matches!(e.kind, EntityKind::ItemEntity(_)))
            .filter(|e| ((e.c.x >> 4) - tx).abs() <= 8 && ((e.c.y >> 4) - ty).abs() <= 8)
            .count()
    };

    let mut dropped_any = false;
    for round in 0..3 {
        for _ in 0..20_000 {
            dispatch::tick(&mut tw, &def, SURFACE, tx, ty);
        }
        tw.tick(); // drain entities_to_add into the arena
        let n = count_nearby(&tw);
        dropped_any |= n > 0;
        assert!(
            n <= 2,
            "beachcombing littered the shore: {n} items after round {round}"
        );
    }
    assert!(dropped_any, "no beach find in 60k exposed tile ticks");
}

/* ---------------------------------- visual dump ---------------------------------- */

/// Not an assertion test: renders the same shore at (near-)high and low tide to
/// target/verify/tide_{high,low}.png so the creeping waterline can be eyeballed.
/// Both moments sit in the Day window so daylight rendering doesn't hide the water —
/// exact high tide (tick 0 / half day) falls in the dark morning/evening.
#[test]
fn tide_frames() {
    let mut tw = TestWorld::infinite().build();
    let (tx, ty) = find_tidal_tile(&tw.tiles);
    tw.teleport(tx, ty);
    tw.tick_n(6);

    for (ticks, name) in [(DAY_LENGTH * 45 / 100, "high"), (DAY_LENGTH / 4, "low")] {
        tw.set_time(ticks);
        tw.screenshot(&format!("tide_{name}.png"));
    }
}
