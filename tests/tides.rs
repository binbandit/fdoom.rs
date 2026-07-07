//! Water tides: the Tidal Flat band between ocean and beach, the day-clock tide
//! curve, submerged/exposed flips, and beachcombing drops.

use std::sync::Arc;

use fdoom::core::renderer::Renderer;
use fdoom::core::updater::DAY_LENGTH;
use fdoom::core::{game::Game, world};
use fdoom::entity::EntityKind;
use fdoom::entity::behavior::is_swimming;
use fdoom::gfx::SpriteSheet;
use fdoom::level::chunk::CHUNK_SIZE;
use fdoom::level::infinite_gen::{Biome, biome_at, generate_chunk, land_at};
use fdoom::level::tile::tidal::{BAND_HIGH, BAND_LOW, is_submerged, tide_level};
use fdoom::level::tile::{Tiles, dispatch};

const SEED: i64 = 20260707;
const SURFACE: usize = 3; // lvl_idx(0)

fn new_infinite_world(name: &str) -> Game {
    let tmp = std::env::temp_dir().join(format!("fdoom_tides_{name}"));
    let _ = std::fs::remove_dir_all(&tmp);
    let mut g = Game::new(false, false, tmp);
    world::reset_game(&mut g, true);
    g.settings.set("worldtype", "Infinite");
    g.world_name = name.into();
    g.world_seed = SEED;
    world::init_world(&mut g);
    g.tick();
    g
}

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
    let tmp = std::env::temp_dir().join("fdoom_tides_curve");
    let mut g = Game::new(false, false, tmp);

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
    let mut g = new_infinite_world("flip");
    let (tx, ty) = find_tidal_tile(&g.tiles);

    // stream chunks in around the flat so tile_at sees the real tile
    {
        let p = g.player_mut();
        p.c.x = tx * 16 + 8;
        p.c.y = ty * 16 + 8;
    }
    for _ in 0..6 {
        g.tick();
    }
    assert_eq!(
        g.tile_at(SURFACE, tx, ty).name,
        "TIDAL FLAT",
        "teleport target is not a tidal flat in the live world"
    );

    // high tide (tick 0): every band tile is under water
    g.set_time(0);
    assert!(is_submerged(&g, tx, ty), "flat not submerged at high tide");
    {
        let player = g.entities.get(g.player_id).unwrap();
        assert!(is_swimming(&g, player), "player not swimming at high tide");
    }

    // low tide (quarter day): the whole band is exposed and walkable
    g.set_time(DAY_LENGTH / 4);
    assert!(!is_submerged(&g, tx, ty), "flat not exposed at low tide");
    {
        let player = g.entities.get(g.player_id).unwrap();
        assert!(!is_swimming(&g, player), "player swimming on dry flat");
    }
}

/* --------------------------------- beachcombing --------------------------------- */

/// Random tile ticks on an exposed flat wash up finds — but never litter: the drop
/// is throttled to at most 2 item entities within 8 tiles.
#[test]
fn beachcombing_drops_are_throttled() {
    let mut g = new_infinite_world("combing");
    let def = g.tiles.get("Tidal Flat");

    // plant a flat on high ground near spawn (land above the band → always exposed,
    // so the test is independent of the clock)
    let (px, py) = {
        let p = g.player();
        (p.c.x >> 4, p.c.y >> 4)
    };
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
    g.set_tile_default(SURFACE, tx, ty, &def);
    assert!(!is_submerged(&g, tx, ty));

    let count_nearby = |g: &Game| {
        g.entities
            .entities_on_level(SURFACE)
            .filter(|e| matches!(e.kind, EntityKind::ItemEntity(_)))
            .filter(|e| ((e.c.x >> 4) - tx).abs() <= 8 && ((e.c.y >> 4) - ty).abs() <= 8)
            .count()
    };

    let mut dropped_any = false;
    for round in 0..3 {
        for _ in 0..20_000 {
            dispatch::tick(&mut g, &def, SURFACE, tx, ty);
        }
        g.tick(); // drain entities_to_add into the arena
        let n = count_nearby(&g);
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
    let mut g = new_infinite_world("frames");
    g.has_gui = true;
    let (tx, ty) = find_tidal_tile(&g.tiles);
    {
        let p = g.player_mut();
        p.c.x = tx * 16 + 8;
        p.c.y = ty * 16 + 8;
    }
    for _ in 0..6 {
        g.tick();
    }

    let mut r = Renderer::new(Arc::new(SpriteSheet::from_png(fdoom::assets::SPRITES_PNG)));
    for (ticks, name) in [(DAY_LENGTH * 45 / 100, "high"), (DAY_LENGTH / 4, "low")] {
        g.set_time(ticks);
        r.render(&mut g);
        let dir = std::path::Path::new("target/verify");
        std::fs::create_dir_all(dir).unwrap();
        let file = std::fs::File::create(dir.join(format!("tide_{name}.png"))).unwrap();
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
