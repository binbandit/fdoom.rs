//! Surface-structure generation: determinism, chunk-border straddling, biome gating,
//! and per-type presence over a realistic scan area.

use fdoom::level::chunk::{CHUNK_SIZE, chunk_coord};
use fdoom::level::infinite_gen::{self, Biome, biome_at};
use fdoom::level::structures_gen::{
    ALL_KINDS, MAX_RADIUS, Placement, StructureKind, chest_pos, placement_in_cell,
    placements_in_rect, structure_writes,
};
use fdoom::level::tile::Tiles;

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
        .filter(|p| chest_pos(SEED, **p).is_some())
        .count();
    assert!(with_chest > 0, "no ruins chest in {} ruins", ruins.len());
    for p in &ruins {
        assert_eq!(chest_pos(SEED, *p), chest_pos(SEED, *p));
    }
    let camp = scan_placements(SEED, StructureKind::Camp, 16384)
        .into_iter()
        .next()
        .expect("presence test guarantees a camp");
    let (tx, ty) = chest_pos(SEED, camp).expect("camps always have a chest");
    assert_eq!((tx, ty), (camp.x + 2, camp.y));
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
