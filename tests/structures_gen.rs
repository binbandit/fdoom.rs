//! Surface-structure generation: determinism, chunk-border straddling, biome gating,
//! and per-type presence over a realistic scan area — for structures, trails,
//! destroyed villages, and boulder scatter.

use fdoom::level::chunk::{CHUNK_SIZE, chunk_coord};
use fdoom::level::infinite_gen::{self, Biome, biome_at};
use fdoom::level::structures_gen::{
    ALL_KINDS, MAX_RADIUS, Placement, StructureKind, TRAIL_JITTER, TRAIL_RANGE, boulder_at,
    chest_positions, kind_radius, placement_in_cell, placements_in_rect, structure_writes,
    trail_writes, trails_in_rect,
};
use fdoom::level::tile::Tiles;
use std::collections::{HashMap, HashSet};

const SEED: i64 = 20260707;

/// Every placement of `kind` with an origin within `radius` tiles of (0, 0).
fn scan_placements(seed: i64, kind: StructureKind, radius: i32) -> Vec<Placement> {
    placements_in_rect(seed, -radius, -radius, radius, radius)
        .into_iter()
        .filter(|p| p.kind == kind)
        .collect()
}

#[test]
fn chunks_with_structures_are_deterministic() {
    let tiles = Tiles::new();
    // pick a chunk that actually contains a structure so the stamp path is exercised
    let p = scan_placements(SEED, StructureKind::Ruins, 4096)
        .into_iter()
        .next()
        .expect("no ruins within 8k x 8k");
    let (cx, cy) = (chunk_coord(p.x), chunk_coord(p.y));
    let a = infinite_gen::generate_chunk(SEED, 0, cx, cy, &tiles);
    let b = infinite_gen::generate_chunk(SEED, 0, cx, cy, &tiles);
    assert_eq!(a.tiles, b.tiles, "same seed+chunk must be identical");
    let c = infinite_gen::generate_chunk(SEED + 1, 0, cx, cy, &tiles);
    assert_ne!(a.tiles, c.tiles, "different seed should differ");
}

#[test]
fn straddling_structures_are_consistent_across_chunks() {
    let tiles = Tiles::new();
    // find structures whose footprint crosses a chunk border, then check every tile of
    // the blueprint against the chunk that owns it — each border chunk must have stamped
    // exactly its share
    let mut checked = 0;
    for kind in ALL_KINDS {
        for p in scan_placements(SEED, kind, 8192) {
            // skip the (rare) case of another structure close enough to overwrite tiles
            let near = placements_in_rect(
                SEED,
                p.x - 2 * MAX_RADIUS - 1,
                p.y - 2 * MAX_RADIUS - 1,
                p.x + 2 * MAX_RADIUS + 1,
                p.y + 2 * MAX_RADIUS + 1,
            );
            if near.len() != 1 {
                continue;
            }

            // later writes overwrite earlier ones (graves stamp over the ground fill):
            // the chunk must hold the *final* value for each position
            let mut expected = std::collections::HashMap::new();
            for (x, y, t) in structure_writes(SEED, p, &tiles) {
                expected.insert((x, y), t);
            }
            let touched: std::collections::HashSet<(i32, i32)> = expected
                .keys()
                .map(|&(x, y)| (chunk_coord(x), chunk_coord(y)))
                .collect();
            if touched.len() < 2 {
                continue; // footprint doesn't straddle a chunk border; keep scanning
            }

            let mut chunks = std::collections::HashMap::new();
            for ((x, y), t) in expected {
                let (cx, cy) = (chunk_coord(x), chunk_coord(y));
                let chunk = chunks
                    .entry((cx, cy))
                    .or_insert_with(|| infinite_gen::generate_chunk(SEED, 0, cx, cy, &tiles));
                let (lx, ly) = (x - cx * CHUNK_SIZE, y - cy * CHUNK_SIZE);
                assert_eq!(
                    chunk.tiles[(lx + ly * CHUNK_SIZE) as usize],
                    t,
                    "{kind:?} at ({}, {}): tile ({x}, {y}) wrong in chunk ({cx}, {cy})",
                    p.x,
                    p.y
                );
            }
            checked += 1;
            break; // one straddler per kind is enough
        }
    }
    assert!(checked >= 2, "found only {checked} straddling structures");
}

#[test]
fn every_kind_appears_and_respects_biomes() {
    for kind in ALL_KINDS {
        let found = scan_placements(SEED, kind, 16384);
        assert!(
            !found.is_empty(),
            "{kind:?}: none within 32k x 32k of origin"
        );
        for p in &found {
            let b = biome_at(SEED, p.x, p.y);
            assert!(
                !matches!(
                    b,
                    Biome::Ocean | Biome::DeepOcean | Biome::Beach | Biome::Mountains
                ),
                "{kind:?} at ({}, {}) placed in {b:?}",
                p.x,
                p.y
            );
        }
    }
}

#[test]
fn cemetery_stamps_real_gravestone_tiles() {
    let tiles = Tiles::new();
    let grave = tiles.get("Grave stone").id;
    let fence = tiles.get("Fence").id;
    let dirt = tiles.get("Dirt").id;
    let p = scan_placements(SEED, StructureKind::Cemetery, 8192)
        .into_iter()
        .next()
        .expect("no cemetery within 16k x 16k");
    let writes = structure_writes(SEED, p, &tiles);
    let graves = writes.iter().filter(|w| w.2 == grave).count();
    assert!(graves >= 4, "cemetery has only {graves} graves");
    assert!(writes.iter().any(|w| w.2 == fence), "no fence segments");
    assert!(writes.iter().any(|w| w.2 == dirt), "no dirt ground");

    // and the stamped chunk really contains them, with data byte 0 (fresh grave state)
    let (cx, cy) = (chunk_coord(p.x), chunk_coord(p.y));
    let chunk = infinite_gen::generate_chunk(SEED, 0, cx, cy, &tiles);
    let in_chunk: Vec<usize> = (0..chunk.tiles.len())
        .filter(|&i| chunk.tiles[i] == grave)
        .collect();
    assert!(!in_chunk.is_empty(), "no grave tiles in the cemetery chunk");
    for i in in_chunk {
        assert_eq!(chunk.data[i], 0, "grave data byte must start at 0");
    }
}

#[test]
fn signature_tiles_present_for_each_kind() {
    let tiles = Tiles::new();
    let expectations: &[(StructureKind, &str)] = &[
        (StructureKind::Ruins, "Stone Wall"),
        (StructureKind::Cemetery, "Grave stone"),
        (StructureKind::StandingStones, "Rock"),
        (StructureKind::Camp, "Wood Planks"),
    ];
    for &(kind, tile_name) in expectations {
        let want = tiles.get(tile_name).id;
        let p = scan_placements(SEED, kind, 16384)
            .into_iter()
            .next()
            .expect("presence test guarantees one exists");
        let (cx, cy) = (chunk_coord(p.x), chunk_coord(p.y));
        let chunk = infinite_gen::generate_chunk(SEED, 0, cx, cy, &tiles);
        assert!(
            chunk.tiles.contains(&want),
            "{kind:?} chunk ({cx}, {cy}) missing its {tile_name} tiles"
        );
    }
}

#[test]
fn ocean_chunks_have_no_structures() {
    let tiles = Tiles::new();
    let forbidden: Vec<u8> = [
        "Stone Wall",
        "Stone Bricks",
        "Grave stone",
        "Fence",
        "Wood Planks",
    ]
    .iter()
    .map(|n| tiles.get(n).id)
    .collect();
    // find a chunk that is entirely deep ocean and make sure nothing was stamped there
    let mut tested = 0;
    'outer: for cy in -60..60 {
        for cx in -60..60 {
            let (bx, by) = (cx * CHUNK_SIZE, cy * CHUNK_SIZE);
            let all_ocean = (0..CHUNK_SIZE + 2 * MAX_RADIUS).step_by(8).all(|dy| {
                (0..CHUNK_SIZE + 2 * MAX_RADIUS).step_by(8).all(|dx| {
                    matches!(
                        biome_at(SEED, bx - MAX_RADIUS + dx, by - MAX_RADIUS + dy),
                        Biome::Ocean | Biome::DeepOcean
                    )
                })
            });
            if !all_ocean {
                continue;
            }
            let chunk = infinite_gen::generate_chunk(SEED, 0, cx, cy, &tiles);
            for &f in &forbidden {
                assert!(
                    !chunk.tiles.contains(&f),
                    "structure tile id {f} in ocean chunk ({cx}, {cy})"
                );
            }
            tested += 1;
            if tested >= 5 {
                break 'outer;
            }
        }
    }
    assert!(tested > 0, "no fully-ocean chunk found to test");
}

#[test]
fn chest_positions_are_deterministic_and_owned_by_one_chunk() {
    let ruins = scan_placements(SEED, StructureKind::Ruins, 16384);
    let with_chest = ruins
        .iter()
        .filter(|p| !chest_positions(SEED, **p).is_empty())
        .count();
    assert!(with_chest > 0, "no ruins chest in {} ruins", ruins.len());
    for p in &ruins {
        assert_eq!(chest_positions(SEED, *p), chest_positions(SEED, *p));
    }
    let camp = scan_placements(SEED, StructureKind::Camp, 16384)
        .into_iter()
        .next()
        .expect("presence test guarantees a camp");
    assert_eq!(chest_positions(SEED, camp), vec![(camp.x + 2, camp.y)]);
}

#[test]
fn placement_grid_is_pure() {
    for kind in ALL_KINDS {
        for cell in [(-3, 7), (0, 0), (11, -2)] {
            assert_eq!(
                placement_in_cell(SEED, kind, cell.0, cell.1),
                placement_in_cell(SEED, kind, cell.0, cell.1)
            );
        }
    }
}

/* --------------------------------------- trails --------------------------------------- */

#[test]
fn trails_link_nearby_structures_deterministically() {
    let tiles = Tiles::new();
    let dirt = tiles.get("dirt").id;
    let pairs = trails_in_rect(SEED, -4096, -4096, 4096, 4096);
    assert!(!pairs.is_empty(), "no trails within 8k x 8k of origin");
    assert_eq!(
        pairs,
        trails_in_rect(SEED, -4096, -4096, 4096, 4096),
        "trail pair set must be deterministic"
    );
    for (a, b) in &pairs {
        for p in [a, b] {
            assert!(
                matches!(
                    p.kind,
                    StructureKind::Ruins | StructureKind::Cemetery | StructureKind::Camp
                ),
                "{:?} is not a trail endpoint kind",
                p.kind
            );
        }
        let (dx, dy) = ((a.x - b.x) as i64, (a.y - b.y) as i64);
        assert!(
            dx * dx + dy * dy <= (TRAIL_RANGE as i64).pow(2),
            "trail ({}, {}) -> ({}, {}) longer than TRAIL_RANGE",
            a.x,
            a.y,
            b.x,
            b.y
        );
    }

    let (a, b) = pairs[0];
    let writes = trail_writes(SEED, a, b, &tiles);
    assert_eq!(
        writes,
        trail_writes(SEED, a, b, &tiles),
        "trail geometry must be deterministic"
    );
    assert!(
        writes.iter().filter(|w| w.2 == dirt).count() >= 20,
        "trail has almost no worn-dirt tiles"
    );
    // the path stays inside the jittered corridor between its endpoints
    let pad = TRAIL_JITTER;
    let (x0, x1) = (a.x.min(b.x) - pad, a.x.max(b.x) + pad);
    let (y0, y1) = (a.y.min(b.y) - pad, a.y.max(b.y) + pad);
    for &(x, y, _) in &writes {
        assert!(
            (x0..=x1).contains(&x) && (y0..=y1).contains(&y),
            "trail tile ({x}, {y}) escaped the corridor of ({}, {}) -> ({}, {})",
            a.x,
            a.y,
            b.x,
            b.y
        );
    }
}

#[test]
fn trails_straddle_chunk_borders_consistently() {
    let tiles = Tiles::new();
    let dirt = tiles.get("dirt").id;
    let torch_dirt = tiles.get("torch dirt").id;
    // soft ground a trail always replaces: if a generated chunk still shows one of
    // these where a trail write should land, that chunk failed to stamp its share
    let soft: Vec<u8> = [
        "grass",
        "sand",
        "snow",
        "Mud",
        "tree",
        "flower",
        "small grass",
        "medium grass",
        "tall grass",
    ]
    .iter()
    .map(|n| tiles.get(n).id)
    .collect();

    let pairs = trails_in_rect(SEED, -4096, -4096, 4096, 4096);
    let mut chunks: HashMap<(i32, i32), fdoom::level::chunk::Chunk> = HashMap::new();
    let mut verified = false;
    for (a, b) in pairs.into_iter().take(20) {
        // last write wins per position, exactly as stamping applies them
        let mut expected: HashMap<(i32, i32), u8> = HashMap::new();
        for (x, y, t) in trail_writes(SEED, a, b, &tiles) {
            expected.insert((x, y), t);
        }
        if expected.is_empty() {
            continue;
        }
        // drop positions inside any structure footprint — structures stamp after trails
        let (xs, ys): (Vec<i32>, Vec<i32>) = expected.keys().copied().unzip();
        let (minx, maxx) = (*xs.iter().min().unwrap(), *xs.iter().max().unwrap());
        let (miny, maxy) = (*ys.iter().min().unwrap(), *ys.iter().max().unwrap());
        let near = placements_in_rect(
            SEED,
            minx - MAX_RADIUS - 1,
            miny - MAX_RADIUS - 1,
            maxx + MAX_RADIUS + 1,
            maxy + MAX_RADIUS + 1,
        );
        expected.retain(|&(x, y), _| {
            !near.iter().any(|p| {
                (x - p.x).abs() <= kind_radius(p.kind) + 1
                    && (y - p.y).abs() <= kind_radius(p.kind) + 1
            })
        });

        let touched: HashSet<(i32, i32)> = expected
            .keys()
            .map(|&(x, y)| (chunk_coord(x), chunk_coord(y)))
            .collect();
        if touched.len() < 2 {
            continue; // want a trail that actually crosses a chunk border
        }

        let mut hits: HashMap<(i32, i32), usize> = HashMap::new();
        for (&(x, y), &t) in &expected {
            let (cx, cy) = (chunk_coord(x), chunk_coord(y));
            let chunk = chunks
                .entry((cx, cy))
                .or_insert_with(|| infinite_gen::generate_chunk(SEED, 0, cx, cy, &tiles));
            let got =
                chunk.tiles[((x - cx * CHUNK_SIZE) + (y - cy * CHUNK_SIZE) * CHUNK_SIZE) as usize];
            if got == t || got == dirt || got == torch_dirt {
                // stamped (possibly by an overlapping trail)
                if got == t {
                    *hits.entry((cx, cy)).or_default() += 1;
                }
            } else {
                // not stamped: only legitimate if the ground there wasn't soft
                assert!(
                    !soft.contains(&got),
                    "chunk ({cx}, {cy}) left soft ground (tile {got}) where trail \
                     ({}, {}) -> ({}, {}) writes tile {t} at ({x}, {y})",
                    a.x,
                    a.y,
                    b.x,
                    b.y
                );
            }
        }
        // a pair counts as fully verified when every chunk it crosses stamped a share
        if touched
            .iter()
            .all(|c| hits.get(c).copied().unwrap_or(0) > 0)
        {
            verified = true;
        }
    }
    assert!(
        verified,
        "no trail verified with stamped tiles on both sides of a chunk border"
    );
}

/* ----------------------------------- destroyed villages -------------------------------- */

#[test]
fn villages_are_ruined_clusters_with_plaza_well_and_chests() {
    let tiles = Tiles::new();
    let stone_wall = tiles.get("Stone Wall").id;
    let stone_floor = tiles.get("Stone Bricks").id;
    let planks = tiles.get("Wood Planks").id;
    let water = tiles.get("water").id;
    let dirt = tiles.get("dirt").id;

    let villages = scan_placements(SEED, StructureKind::Village, 16384);
    assert!(!villages.is_empty(), "no villages within 32k x 32k");

    let p = villages[0];
    let writes = structure_writes(SEED, p, &tiles);
    assert_eq!(
        writes,
        structure_writes(SEED, p, &tiles),
        "village blueprint must be deterministic"
    );
    // last write wins, exactly as stamping applies them
    let mut last: HashMap<(i32, i32), u8> = HashMap::new();
    for &(x, y, t) in &writes {
        assert!(
            (x - p.x).abs() <= kind_radius(p.kind) && (y - p.y).abs() <= kind_radius(p.kind),
            "village write ({x}, {y}) outside its declared radius"
        );
        last.insert((x, y), t);
    }
    let count = |id: u8| last.values().filter(|&&t| t == id).count();
    assert!(count(stone_wall) >= 10, "too few standing walls");
    assert!(count(planks) >= 20, "too few plank floors");
    assert!(count(dirt) >= 30, "no worn plaza/paths");
    assert!(count(stone_floor) >= 1, "no surviving plaza paving");
    assert_eq!(
        last.get(&(p.x, p.y)),
        Some(&water),
        "the rubble well must hold water at the plaza center"
    );

    // 1-2 chests, deterministic, always on an interior plank floor
    let chests = chest_positions(SEED, p);
    assert!(
        (1..=2).contains(&chests.len()),
        "village has {} chests",
        chests.len()
    );
    assert_eq!(chests, chest_positions(SEED, p));
    for &(cx, cy) in &chests {
        assert_eq!(
            last.get(&(cx, cy)),
            Some(&planks),
            "chest at ({cx}, {cy}) must sit on a plank floor"
        );
    }
    // both chest counts occur across villages (that's what "1-2" means)
    let counts: HashSet<usize> = villages
        .iter()
        .map(|v| chest_positions(SEED, *v).len())
        .collect();
    assert!(
        counts.contains(&1) && counts.contains(&2),
        "chest counts seen: {counts:?}"
    );

    // and generated chunks really contain the stamped shell, across the footprint
    let mut seen: HashSet<u8> = HashSet::new();
    for cy in chunk_coord(p.y - MAX_RADIUS)..=chunk_coord(p.y + MAX_RADIUS) {
        for cx in chunk_coord(p.x - MAX_RADIUS)..=chunk_coord(p.x + MAX_RADIUS) {
            seen.extend(
                infinite_gen::generate_chunk(SEED, 0, cx, cy, &tiles)
                    .tiles
                    .iter(),
            );
        }
    }
    for (id, what) in [(stone_wall, "stone walls"), (planks, "plank floors")] {
        assert!(seen.contains(&id), "village chunks missing {what}");
    }
    let (wcx, wcy) = (chunk_coord(p.x), chunk_coord(p.y));
    let well_chunk = infinite_gen::generate_chunk(SEED, 0, wcx, wcy, &tiles);
    let i = ((p.x - wcx * CHUNK_SIZE) + (p.y - wcy * CHUNK_SIZE) * CHUNK_SIZE) as usize;
    assert_eq!(
        well_chunk.tiles[i], water,
        "well water missing in the chunk"
    );
}

/* --------------------------------------- boulders -------------------------------------- */

#[test]
fn boulders_scatter_sparsely_and_straddle_chunks() {
    let tiles = Tiles::new();
    let rock = tiles.get("rock").id;

    // presence, biome gating, determinism, sparseness over a 3k x 3k sample
    let r = 1536;
    let mut anchors = Vec::new();
    for y in -r..r {
        for x in -r..r {
            if let Some(big) = boulder_at(SEED, x, y) {
                assert_eq!(boulder_at(SEED, x, y), Some(big), "boulder_at must be pure");
                assert!(
                    matches!(
                        biome_at(SEED, x, y),
                        Biome::Plains | Biome::Savanna | Biome::Tundra
                    ),
                    "boulder at ({x}, {y}) outside its biomes"
                );
                anchors.push((x, y, big));
            }
        }
    }
    assert!(!anchors.is_empty(), "no boulders in a 3k x 3k sample");
    let area = (2 * r as usize) * (2 * r as usize);
    assert!(
        anchors.len() < area / 1000,
        "boulders too dense: {} in {} tiles",
        anchors.len(),
        area
    );
    assert!(
        anchors.iter().any(|a| a.2) && anchors.iter().any(|a| !a.2),
        "expected a mix of single and 2x2 boulders"
    );

    // every boulder tile is stamped as plain (breakable) rock; verify a handful,
    // preferring 2x2 boulders that straddle a chunk border
    let clear_of_structures = |x: i32, y: i32| {
        placements_in_rect(
            SEED,
            x - MAX_RADIUS - 2,
            y - MAX_RADIUS - 2,
            x + MAX_RADIUS + 3,
            y + MAX_RADIUS + 3,
        )
        .is_empty()
    };
    let straddles = |&(x, y, big): &(i32, i32, bool)| {
        big && (chunk_coord(x) != chunk_coord(x + 1) || chunk_coord(y) != chunk_coord(y + 1))
    };
    let picks: Vec<(i32, i32, bool)> = anchors
        .iter()
        .filter(|a| straddles(a) && clear_of_structures(a.0, a.1))
        .take(2)
        .chain(
            anchors
                .iter()
                .filter(|a| !straddles(a) && clear_of_structures(a.0, a.1))
                .take(3),
        )
        .copied()
        .collect();
    assert!(
        picks.iter().any(straddles),
        "no 2x2 boulder straddling a chunk border found to verify"
    );
    let mut chunks: HashMap<(i32, i32), fdoom::level::chunk::Chunk> = HashMap::new();
    for (x, y, big) in picks {
        let ext = if big { 1 } else { 0 };
        for dy in 0..=ext {
            for dx in 0..=ext {
                let (tx, ty) = (x + dx, y + dy);
                let (cx, cy) = (chunk_coord(tx), chunk_coord(ty));
                let chunk = chunks
                    .entry((cx, cy))
                    .or_insert_with(|| infinite_gen::generate_chunk(SEED, 0, cx, cy, &tiles));
                let i = ((tx - cx * CHUNK_SIZE) + (ty - cy * CHUNK_SIZE) * CHUNK_SIZE) as usize;
                assert_eq!(
                    chunk.tiles[i], rock,
                    "boulder tile ({tx}, {ty}) not rock in chunk ({cx}, {cy})"
                );
            }
        }
    }
}
