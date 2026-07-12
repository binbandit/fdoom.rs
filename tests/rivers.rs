//! Rivers: the winding mid-contour band of the river course field
//! (`infinite_gen::river_zone_at`) — river water, its pannable banks, desert
//! fade-out, and the plank footbridges where trails cross the channel.

use fdoom::level::chunk::CHUNK_SIZE;
use fdoom::level::infinite_gen::{
    Biome, RiverZone, biome_at, biome_at_blended, climate_at, generate_chunk, land_at,
    river_zone_at,
};
use fdoom::level::structures_gen::{trail_writes, trails_in_rect};
use fdoom::level::tile::Tiles;
use fdoom::testutil::TestWorld;

const SEEDS: [i64; 4] = [1, 424242, 20260707, -987654321];

/// Is this a biome a walker could be standing in when they "find a river"?
fn land_biome(b: Biome) -> bool {
    !matches!(b, Biome::Ocean | Biome::DeepOcean | Biome::Beach)
}

/// Tuning/stat dump: run with
/// `cargo test --test rivers -- --ignored --nocapture stats`
#[test]
#[ignore]
fn stats() {
    for seed in SEEDS {
        // crossings along straight walks (rivers per tile of travel)
        let mut crossings = 0usize;
        let mut walked = 0usize;
        let mut channel_tiles = 0usize;
        let mut land_tiles = 0usize;
        for line in -4..=4i32 {
            let y = line * 1500;
            let mut in_channel = false;
            for x in -6000..6000 {
                if !land_biome(biome_at(seed, x, y)) {
                    in_channel = false;
                    continue;
                }
                walked += 1;
                land_tiles += 1;
                let ch = matches!(river_zone_at(seed, x, y), Some(RiverZone::Channel));
                if ch {
                    channel_tiles += 1;
                }
                if ch && !in_channel {
                    crossings += 1;
                }
                in_channel = ch;
            }
        }
        println!(
            "seed {seed}: {crossings} crossings over {walked} land tiles walked \
             (1 per {} tiles); channel fraction {:.4}",
            walked / crossings.max(1),
            channel_tiles as f64 / land_tiles as f64
        );
    }

    // ASCII map of a river region for shape judgement
    let seed = SEEDS[2];
    'hunt: for cy in -30..30i32 {
        for cx in -30..30i32 {
            let (x0, y0) = (cx * 128, cy * 128);
            let n = (0..128)
                .flat_map(|dy| (0..128).map(move |dx| (x0 + dx, y0 + dy)))
                .filter(|&(x, y)| matches!(river_zone_at(seed, x, y), Some(RiverZone::Channel)))
                .count();
            if n > 400 {
                println!("map at ({x0},{y0}), {n} channel tiles:");
                for y in y0..y0 + 96 {
                    let row: String = (x0..x0 + 192)
                        .map(|x| match river_zone_at(seed, x, y) {
                            Some(RiverZone::Channel) => '~',
                            Some(RiverZone::Bank) => ':',
                            None if !land_biome(biome_at(seed, x, y)) => 'o',
                            None => '.',
                        })
                        .collect();
                    println!("{row}");
                }
                break 'hunt;
            }
        }
    }
}

/// Rivers show up at a "notable find" frequency: crossing the world in straight
/// lines meets a river roughly every 500-3000 land tiles — present on every seed,
/// never so dense the country turns to canals.
#[test]
fn rivers_exist_at_plausible_frequency() {
    for seed in SEEDS {
        let mut crossings = 0usize;
        let mut walked = 0usize;
        for line in -4..=4i32 {
            let y = line * 1500;
            let mut in_channel = false;
            for x in -6000..6000 {
                if !land_biome(biome_at(seed, x, y)) {
                    in_channel = false;
                    continue;
                }
                walked += 1;
                let ch = matches!(river_zone_at(seed, x, y), Some(RiverZone::Channel));
                if ch && !in_channel {
                    crossings += 1;
                }
                in_channel = ch;
            }
        }
        let per = walked / crossings.max(1);
        assert!(
            (500..=3000).contains(&per),
            "seed {seed}: a river crossing every {per} land tiles \
             ({crossings} crossings / {walked} walked) — outside the notable-find band"
        );
    }
}

/// Channel tiles generate as water; bank-zone tiles generate as a soft margin
/// (mud/sand/snow, never trees or rock), so the water's edge is always workable.
#[test]
fn river_tiles_are_water_with_soft_banks() {
    let tiles = Tiles::new();
    let water = tiles.get("water").id;
    let banks = [
        tiles.get("Mud").id,
        tiles.get("sand").id,
        tiles.get("snow").id,
    ];
    let planks = tiles.get("Wood Planks").id;
    for seed in SEEDS {
        let (mut w, mut b) = (0usize, 0usize);
        'chunks: for cy in -40..40i32 {
            for cx in -40..40i32 {
                // probe the chunk center cheaply before generating the whole chunk
                let (px, py) = (cx * CHUNK_SIZE + 32, cy * CHUNK_SIZE + 32);
                if river_zone_at(seed, px, py).is_none() {
                    continue;
                }
                if !land_biome(biome_at_blended(seed, px, py)) {
                    continue;
                }
                let chunk = generate_chunk(seed, 0, cx, cy, &tiles);
                for ly in 0..CHUNK_SIZE {
                    for lx in 0..CHUNK_SIZE {
                        let (x, y) = (cx * CHUNK_SIZE + lx, cy * CHUNK_SIZE + ly);
                        if !land_biome(biome_at_blended(seed, x, y)) {
                            continue;
                        }
                        let t = chunk.tiles[(lx + ly * CHUNK_SIZE) as usize];
                        match river_zone_at(seed, x, y) {
                            // structures stamp over everything (rare) and a trail
                            // bridge lays planks — tolerate both, count the rest
                            Some(RiverZone::Channel) if t == water || t == planks => w += 1,
                            Some(RiverZone::Bank) if banks.contains(&t) => b += 1,
                            _ => {}
                        }
                    }
                }
                if w > 200 && b > 100 {
                    break 'chunks;
                }
            }
        }
        assert!(
            w > 200 && b > 100,
            "seed {seed}: only {w} river-water and {b} bank tiles found"
        );
    }
}

/// Two generations of the same river chunk are identical (chunk-border purity is
/// covered by the existing determinism suite; this pins the river arm specifically).
#[test]
fn river_generation_is_deterministic() {
    let tiles = Tiles::new();
    let seed = SEEDS[2];
    // find a chunk with river in it
    for cy in -40..40i32 {
        for cx in -40..40i32 {
            let (px, py) = (cx * CHUNK_SIZE + 32, cy * CHUNK_SIZE + 32);
            if river_zone_at(seed, px, py) != Some(RiverZone::Channel) {
                continue;
            }
            let a = generate_chunk(seed, 0, cx, cy, &tiles);
            let b = generate_chunk(seed, 0, cx, cy, &tiles);
            assert_eq!(
                a.tiles, b.tiles,
                "river chunk ({cx},{cy}) not deterministic"
            );
            return;
        }
    }
    panic!("no river chunk found for seed {seed}");
}

/// Rivers fade out before the deep desert: no channel anywhere the climate is well
/// past the Desert gate on dry ground (the strength fade hits zero at climate 0.70;
/// deep-desert country is hotter still).
#[test]
fn rivers_fade_out_in_deep_desert() {
    for seed in SEEDS {
        for y in (-6000..6000).step_by(7) {
            for x in (-6000..6000).step_by(7) {
                if biome_at(seed, x, y) == Biome::Desert && climate_at(seed, x, y) > 0.72 {
                    assert_eq!(
                        river_zone_at(seed, x, y),
                        None,
                        "seed {seed}: river at ({x},{y}) deep in hot-dry country"
                    );
                }
            }
        }
    }
}

/// Rivers never intrude on the coast machinery: no channel or bank tile ever sits
/// in or below the tidal band (`land < 0.448`), so tides keep their exact strip.
#[test]
fn rivers_stay_above_the_tidal_band() {
    for seed in SEEDS {
        for y in (-6000..6000).step_by(5) {
            for x in (-6000..6000).step_by(5) {
                if river_zone_at(seed, x, y).is_some() {
                    assert!(
                        land_at(seed, x, y) >= 0.448,
                        "seed {seed}: river zone at ({x},{y}) inside the coast band"
                    );
                }
            }
        }
    }
}

/// Panning a real river bank works through the real gate (water_adjacent + try_pan)
/// and eventually pays: boot a world, walk to a generated river, pan the bank.
#[test]
fn panning_a_river_bank_pays() {
    let seed = SEEDS[2];
    // find a bank tile whose 4-neighborhood holds a channel tile, on land
    let mut found = None;
    'hunt: for y in -2000..2000i32 {
        for x in -2000..2000i32 {
            if river_zone_at(seed, x, y) != Some(RiverZone::Bank) {
                continue;
            }
            if !land_biome(biome_at_blended(seed, x, y)) {
                continue;
            }
            // temperate country only, so the bank rim is mud (mud always pans;
            // snow banks in the cold don't, by design)
            if !(0.40..0.60).contains(&climate_at(seed, x, y)) {
                continue;
            }
            let channel_adjacent = [(0, -1), (0, 1), (-1, 0), (1, 0)].iter().any(|(dx, dy)| {
                river_zone_at(seed, x + dx, y + dy) == Some(RiverZone::Channel)
                    && land_biome(biome_at_blended(seed, x + dx, y + dy))
            });
            if channel_adjacent {
                found = Some((x, y));
                break 'hunt;
            }
        }
    }
    let (bx, by) = found.expect("no temperate river bank tile within 4000x4000 of origin");

    let mut tw = TestWorld::infinite().seed(seed).build();
    tw.teleport(bx, by - 1); // stand beside the bank, pan the tile below
    tw.tick_n(10); // let the chunks around the player load
    let bank = tw.g.tile_at(tw.g.current_level, bx, by).name.clone();
    assert!(
        bank.eq_ignore_ascii_case("mud") || bank.eq_ignore_ascii_case("sand"),
        "river bank at ({bx},{by}) generated as {bank:?}, not a pannable margin"
    );
    let mut panned = 0;
    for _ in 0..60 {
        tw.g.player_mut().player_mut().stamina = 10;
        if tw.interact_with("Prospector's Pan", 0, 1) {
            panned += 1;
        }
    }
    assert!(
        panned >= 40,
        "river bank at ({bx},{by}) only panned {panned}/60"
    );
    let drops = tw.dropped_items();
    let paying = [
        "Stone",
        "Coal",
        "Iron Ore",
        "Gold Ore",
        "gem",
        "Seed Potato",
    ];
    assert!(
        drops
            .iter()
            .any(|d| paying.iter().any(|p| d.eq_ignore_ascii_case(p))),
        "{panned} pans of river gravel turned up nothing: {drops:?}"
    );
}

/// Where a trail crosses the river channel, the generated chunk carries walkable
/// plank-bridge tiles instead of open water (searched across seeds until found).
#[test]
fn trail_river_crossings_are_bridged() {
    let tiles = Tiles::new();
    let planks = tiles.get("Wood Planks").id;
    let mut bridges = 0usize;
    for seed in SEEDS {
        for (a, b) in trails_in_rect(seed, -3000, -3000, 3000, 3000) {
            for (x, y, _) in trail_writes(seed, a, b, &tiles) {
                if river_zone_at(seed, x, y) != Some(RiverZone::Channel) {
                    continue;
                }
                if !land_biome(biome_at_blended(seed, x, y)) {
                    continue;
                }
                let (cx, cy) = (x.div_euclid(CHUNK_SIZE), y.div_euclid(CHUNK_SIZE));
                let chunk = generate_chunk(seed, 0, cx, cy, &tiles);
                let i = (x.rem_euclid(CHUNK_SIZE) + y.rem_euclid(CHUNK_SIZE) * CHUNK_SIZE) as usize;
                if chunk.tiles[i] == planks {
                    bridges += 1;
                    if bridges >= 3 {
                        return;
                    }
                }
            }
        }
    }
    panic!("only {bridges} plank-bridge tiles found at trail-river crossings across seeds");
}

/// Screenshot harness (run with `--ignored`): the shots land in `target/verify/`.
#[test]
#[ignore]
fn shots() {
    use fdoom::core::updater::Time;
    use fdoom::testutil::{save_png, verify_path};

    let seed = SEEDS[2];

    // how many channel tiles fill the screen-sized window around (x, y)?
    let density = |x: i32, y: i32, r: i32| {
        (-r..=r)
            .flat_map(|dy| (-r..=r).map(move |dx| (x + dx, y + dy)))
            .filter(|&(x, y)| matches!(river_zone_at(seed, x, y), Some(RiverZone::Channel)))
            .count()
    };
    // best channel tile matching `keep` in a coarse sweep. Ribbon-like windows
    // only (a bend crossing the frame): density-maximizing would beeline to the
    // rare lake blobs instead of a winding channel.
    let hunt = |keep: &dyn Fn(i32, i32) -> bool| -> Option<(i32, i32)> {
        let mut best = None;
        let mut best_n = 0;
        for y in (-3000..3000i32).step_by(3) {
            for x in (-3000..3000i32).step_by(3) {
                if !matches!(river_zone_at(seed, x, y), Some(RiverZone::Channel)) || !keep(x, y) {
                    continue;
                }
                let n = density(x, y, 9);
                if n > best_n && n <= 120 {
                    best_n = n;
                    best = Some((x, y));
                }
            }
        }
        best
    };

    let mut tw = TestWorld::infinite().seed(seed).build();
    tw.g.change_time_of_day(Time::Day);
    let shoot = |tw: &mut TestWorld, (x, y): (i32, i32), name: &str| {
        tw.teleport(x, y);
        tw.tick_n(12);
        let p = tw.screenshot(name);
        println!("{name}: centered ({x},{y}) -> {}", p.display());
    };

    // 1. money shot: the river winding through forest
    let spot = hunt(&|x, y| biome_at(seed, x, y) == Biome::Forest).expect("no forest river");
    shoot(&mut tw, spot, "river_forest.png");

    // 2. banks up close: stand on a temperate bank tile
    let bank = hunt(&|x, y| {
        (0.40..0.60).contains(&climate_at(seed, x, y))
            && [(0, 1), (1, 0)]
                .iter()
                .any(|(dx, dy)| river_zone_at(seed, x + dx, y + dy) == Some(RiverZone::Bank))
    })
    .expect("no temperate bank");
    shoot(&mut tw, (bank.0, bank.1 + 2), "river_banks.png");

    // 3. a trail bridge: first bridged crossing found across the region
    let tiles = Tiles::new();
    // prefer a clean perpendicular crossing: a trail whose channel overlap is a
    // short run (a trail riding along the river planks a long boardwalk instead)
    let mut bridge = None;
    let mut bridge_n = 0;
    for (a, b) in trails_in_rect(seed, -3000, -3000, 3000, 3000) {
        let over: Vec<(i32, i32)> = trail_writes(seed, a, b, &tiles)
            .into_iter()
            .filter(|&(x, y, _)| {
                river_zone_at(seed, x, y) == Some(RiverZone::Channel)
                    && land_biome(biome_at_blended(seed, x, y))
            })
            .map(|(x, y, _)| (x, y))
            .collect();
        if over.is_empty() || over.len() > 9 {
            continue;
        }
        let (x, y) = over[over.len() / 2];
        // a structure stamped over the crossing wins the tile — skip those
        let chunk = generate_chunk(
            seed,
            0,
            x.div_euclid(CHUNK_SIZE),
            y.div_euclid(CHUNK_SIZE),
            &tiles,
        );
        let i = (x.rem_euclid(CHUNK_SIZE) + y.rem_euclid(CHUNK_SIZE) * CHUNK_SIZE) as usize;
        if chunk.tiles[i] != tiles.get("Wood Planks").id {
            continue;
        }
        let n = density(x, y, 4);
        if n > bridge_n {
            bridge_n = n;
            bridge = Some((x, y));
        }
    }
    if let Some(spot) = bridge {
        shoot(&mut tw, spot, "river_bridge.png");
    } else {
        println!("river_bridge.png: no crossing on this seed (test covers it across seeds)");
    }

    // 4. river meeting the sea: the channel tile closest above the coast fade
    let mouth = hunt(&|x, y| {
        land_at(seed, x, y) < 0.462
            && (-14..=14).any(|d| {
                !land_biome(biome_at(seed, x + d, y)) || !land_biome(biome_at(seed, x, y + d))
            })
    });
    if let Some(spot) = mouth {
        shoot(&mut tw, spot, "river_mouth.png");
    }

    // 5. overview map: 1 px per tile over 768x768, real generated chunks
    let (cx0, cy0) = {
        let (x, y) = hunt(&|_, _| true).expect("no river at all");
        (x.div_euclid(CHUNK_SIZE) - 6, y.div_euclid(CHUNK_SIZE) - 6)
    };
    let side = 12 * CHUNK_SIZE as usize;
    let mut px = vec![0i32; side * side];
    let water = tiles.get("water").id;
    let deep = tiles.get("Deep Water").id;
    let mud = tiles.get("Mud").id;
    let sand = tiles.get("sand").id;
    let rock = tiles.get("rock").id;
    let snow = tiles.get("snow").id;
    let planks = tiles.get("Wood Planks").id;
    for cy in 0..12 {
        for cx in 0..12 {
            let chunk = generate_chunk(seed, 0, cx0 + cx, cy0 + cy, &tiles);
            for ly in 0..CHUNK_SIZE as usize {
                for lx in 0..CHUNK_SIZE as usize {
                    let t = chunk.tiles[lx + ly * CHUNK_SIZE as usize];
                    let c = if t == water {
                        0x2F6FD0
                    } else if t == deep {
                        0x1E4A8F
                    } else if t == mud {
                        0x6B4A2B
                    } else if t == sand {
                        0xD8C878
                    } else if t == rock {
                        0x8A8A8A
                    } else if t == snow {
                        0xE8ECF2
                    } else if t == planks {
                        0xC08840
                    } else {
                        0x3F8F3F
                    };
                    px[(cx as usize * CHUNK_SIZE as usize + lx)
                        + (cy as usize * CHUNK_SIZE as usize + ly) * side] = c;
                }
            }
        }
    }
    let path = verify_path("river_overview.png");
    save_png(&path, &px, side, side, 1);
    println!(
        "river_overview.png: tiles ({},{}) .. +768 -> {}",
        cx0 * CHUNK_SIZE,
        cy0 * CHUNK_SIZE,
        path.display()
    );
}
