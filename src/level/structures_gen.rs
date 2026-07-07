//! Deterministic surface structures for infinite worlds: ruins, cemeteries, standing
//! stones, and abandoned camps.
//!
//! Placement follows the same hash-grid pattern as `infinite_gen::gate_in_cell`: each
//! structure type gets its own coarse cell grid, and each cell holds at most one
//! structure at a jittered, biome-gated position — a pure function of
//! `(world seed, structure kind, cell)`. Chunks stamp every structure whose footprint
//! could overlap them (rect query padded by [`MAX_RADIUS`]), so a structure straddling a
//! chunk border comes out identical from both sides.
//!
//! Tiles are stamped during `infinite_gen::generate_chunk` (before the gate set-pieces,
//! so a rare overlap always leaves the gate intact). Loot chests are entities and can't
//! live in the pure tile pass; they are spawned by [`spawn_chunk_entities`] when
//! `level::ensure_chunks_at` generates a chunk *fresh* (not loaded from disk), and the
//! chunk is marked dirty so it persists and the chest never duplicates.

use super::chunk::{CHUNK_SIZE, Chunk, chunk_coord};
use super::infinite_gen::{Biome, biome_at, hash, unit};
use super::tile::Tiles;
use crate::core::game::Game;
use crate::rng::Rng;

/// Largest half-extent of any structure footprint (13x13 max).
pub const MAX_RADIUS: i32 = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureKind {
    Ruins,
    Cemetery,
    StandingStones,
    Camp,
}

/// Fixed iteration order — stamping order must be identical from every chunk.
pub const ALL_KINDS: [StructureKind; 4] = [
    StructureKind::Ruins,
    StructureKind::Cemetery,
    StructureKind::StandingStones,
    StructureKind::Camp,
];

/// A placed structure: kind + origin (footprint center), in global tile coords.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub kind: StructureKind,
    pub x: i32,
    pub y: i32,
}

/// Per-kind placement parameters: (cell grid size, hash salt, odds a cell has one).
fn spec(kind: StructureKind) -> (i32, u64, f64) {
    match kind {
        StructureKind::Ruins => (224, 0x5255_494E_5321_0001, 0.45),
        StructureKind::Cemetery => (288, 0x4752_4156_4553_0002, 0.40),
        StructureKind::StandingStones => (320, 0x53_544F_4E45_0003, 0.35),
        StructureKind::Camp => (256, 0x43_414D_5046_0004, 0.50),
    }
}

/// Which biomes a structure may spawn in (never ocean/beach/mountains).
fn biome_ok(kind: StructureKind, b: Biome) -> bool {
    match kind {
        StructureKind::Ruins => matches!(b, Biome::Plains | Biome::Forest | Biome::Savanna),
        StructureKind::Cemetery => matches!(b, Biome::Plains | Biome::Forest | Biome::Marsh),
        StructureKind::StandingStones => matches!(b, Biome::Plains | Biome::Savanna),
        StructureKind::Camp => matches!(b, Biome::Forest | Biome::Tundra | Biome::Desert),
    }
}

/// The structure (if any) of `kind` in a placement-grid cell. Pure.
pub fn placement_in_cell(
    seed: i64,
    kind: StructureKind,
    cell_x: i32,
    cell_y: i32,
) -> Option<Placement> {
    let (grid, salt, odds) = spec(kind);
    let h = hash(seed, salt, cell_x, cell_y);
    if unit(h) > odds {
        return None;
    }
    // jitter inside the cell, keeping a full footprint of margin from the cell edge
    let margin = MAX_RADIUS + 1;
    let jx = margin + ((h >> 8) as i32).rem_euclid(grid - 2 * margin);
    let jy = margin + ((h >> 24) as i32).rem_euclid(grid - 2 * margin);
    let (x, y) = (cell_x * grid + jx, cell_y * grid + jy);
    if !biome_ok(kind, biome_at(seed, x, y)) {
        return None;
    }
    Some(Placement { kind, x, y })
}

/// Every structure whose *origin* lies inside `[x0, x1] x [y0, y1]`. Deterministic order
/// (kind, then cell y, then cell x) so overlapping stamps resolve identically everywhere.
pub fn placements_in_rect(seed: i64, x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<Placement> {
    let mut out = Vec::new();
    for kind in ALL_KINDS {
        let (grid, _, _) = spec(kind);
        for cy in y0.div_euclid(grid)..=y1.div_euclid(grid) {
            for cx in x0.div_euclid(grid)..=x1.div_euclid(grid) {
                if let Some(p) = placement_in_cell(seed, kind, cx, cy) {
                    if p.x >= x0 && p.x <= x1 && p.y >= y0 && p.y <= y1 {
                        out.push(p);
                    }
                }
            }
        }
    }
    out
}

/* ------------------------------------ blueprints ------------------------------------ */

/// Tile ids the blueprints stamp with.
struct StructIds {
    grass: u8,
    dirt: u8,
    rock: u8,
    flower: u8,
    stone_wall: u8,
    stone_floor: u8,
    grave: u8,
    fence: u8,
    planks: u8,
    torch_dirt: u8,
}

impl StructIds {
    fn get(tiles: &Tiles) -> StructIds {
        StructIds {
            grass: tiles.get("grass").id,
            dirt: tiles.get("dirt").id,
            rock: tiles.get("rock").id,
            flower: tiles.get("flower").id,
            stone_wall: tiles.get("Stone Wall").id,
            stone_floor: tiles.get("Stone Bricks").id,
            grave: tiles.get("Grave stone").id,
            fence: tiles.get("Fence").id,
            planks: tiles.get("Wood Planks").id,
            torch_dirt: tiles.get("torch dirt").id,
        }
    }
}

/// The full tile footprint of one structure as `(global x, global y, tile id)` writes,
/// in stamping order. Pure function of `(seed, placement)` — this is what guarantees a
/// border-straddling structure looks the same from every chunk that stamps a piece of it.
pub fn structure_writes(seed: i64, p: Placement, tiles: &Tiles) -> Vec<(i32, i32, u8)> {
    let ids = &StructIds::get(tiles);
    let mut w = Vec::new();
    let (ox, oy) = (p.x, p.y);
    // per-tile detail hash, salted per kind so overlapping structures don't correlate
    let detail = |salt: u64, x: i32, y: i32| unit(hash(seed, salt, x, y));

    match p.kind {
        StructureKind::Ruins => {
            // broken rectangle of stone walls over a brick floor, with rubble
            let h = hash(seed, 0xB1DE_0001, ox, oy);
            let hw = 3 + (h % 3) as i32; // half-extents 3..=5 (7x7 .. 11x11)
            let hh = 3 + ((h >> 16) % 3) as i32;
            for dy in -hh..=hh {
                for dx in -hw..=hw {
                    let (x, y) = (ox + dx, oy + dy);
                    let perimeter = dx.abs() == hw || dy.abs() == hh;
                    let doorway = dx == 0 && dy == hh; // always-open south gap
                    let crumbled = detail(0xB1DE_0002, x, y) < 0.30;
                    let t = if perimeter && !doorway && !crumbled {
                        ids.stone_wall
                    } else if !perimeter && detail(0xB1DE_0003, x, y) < 0.06 {
                        ids.rock // rubble
                    } else if detail(0xB1DE_0004, x, y) < 0.12 {
                        ids.dirt // floor worn through to earth
                    } else {
                        ids.stone_floor
                    };
                    w.push((x, y, t));
                }
            }
        }
        StructureKind::Cemetery => {
            // dirt plot with a broken fence edge and graves spaced 2 apart
            let h = hash(seed, 0xCE4E_0001, ox, oy);
            let rx = 4 + (h % 3) as i32; // half-extents 4..=6 (9x9 .. 13x13)
            let ry = 4 + ((h >> 16) % 3) as i32;
            for dy in -ry..=ry {
                for dx in -rx..=rx {
                    let (x, y) = (ox + dx, oy + dy);
                    let perimeter = dx.abs() == rx || dy.abs() == ry;
                    let gate = dx == 0 && dy == ry;
                    let t = if perimeter && !gate && detail(0xCE4E_0002, x, y) < 0.60 {
                        ids.fence
                    } else {
                        ids.dirt
                    };
                    w.push((x, y, t));
                }
            }
            // grave rows: every 2 tiles, aligned to the origin, one tile in from the fence
            for dy in (-(ry - 2)..=(ry - 2)).step_by(2) {
                for dx in (-(rx - 2)..=(rx - 2)).step_by(2) {
                    let (x, y) = (ox + dx, oy + dy);
                    if detail(0xCE4E_0003, x, y) < 0.85 {
                        w.push((x, y, ids.grave));
                    }
                }
            }
        }
        StructureKind::StandingStones => {
            // a ring of stones on cleared grass with a flower at the center
            let h = hash(seed, 0x57ED_0001, ox, oy);
            let r = 3 + (h % 2) as i32; // radius 3 or 4
            for dy in -(r + 1)..=(r + 1) {
                for dx in -(r + 1)..=(r + 1) {
                    let d2 = dx * dx + dy * dy;
                    if d2 > (r + 1) * (r + 1) {
                        continue;
                    }
                    let (x, y) = (ox + dx, oy + dy);
                    // ring band: |d2 - r²| <= r, with a few fallen (missing) stones
                    let on_ring = (d2 - r * r).abs() <= r;
                    let t = if dx == 0 && dy == 0 {
                        ids.flower
                    } else if on_ring && detail(0x57ED_0002, x, y) < 0.80 {
                        ids.rock
                    } else {
                        ids.grass
                    };
                    w.push((x, y, t));
                }
            }
        }
        StructureKind::Camp => {
            // trampled clearing, a still-burning torch, and a plank lean-to
            for dy in -3..=3 {
                for dx in -3..=3 {
                    if dx * dx + dy * dy > 10 {
                        continue;
                    }
                    w.push((ox + dx, oy + dy, ids.dirt));
                }
            }
            for (dx, dy) in [(-2, -1), (-1, -1), (-2, 0), (-1, 0)] {
                w.push((ox + dx, oy + dy, ids.planks));
            }
            w.push((ox, oy, ids.torch_dirt));
        }
    }
    w
}

/* ----------------------------------- chunk stamping ---------------------------------- */

/// Stamp every structure overlapping the chunk. Called from
/// `infinite_gen::generate_chunk`; pure, surface only.
pub fn stamp_chunk(seed: i64, depth: i32, cx: i32, cy: i32, chunk: &mut Chunk, tiles: &Tiles) {
    if depth != 0 {
        return;
    }
    let base_x = cx * CHUNK_SIZE;
    let base_y = cy * CHUNK_SIZE;
    let placements = placements_in_rect(
        seed,
        base_x - MAX_RADIUS,
        base_y - MAX_RADIUS,
        base_x + CHUNK_SIZE - 1 + MAX_RADIUS,
        base_y + CHUNK_SIZE - 1 + MAX_RADIUS,
    );
    for p in placements {
        for (x, y, t) in structure_writes(seed, p, tiles) {
            let (lx, ly) = (x - base_x, y - base_y);
            if (0..CHUNK_SIZE).contains(&lx) && (0..CHUNK_SIZE).contains(&ly) {
                chunk.tiles[(lx + ly * CHUNK_SIZE) as usize] = t;
            }
        }
    }
}

/* ------------------------------------ loot chests ------------------------------------ */

/// The global tile the structure's loot chest sits on, if the structure has one.
/// Pure, so exactly one chunk (the one containing this tile) owns the spawn.
pub fn chest_pos(seed: i64, p: Placement) -> Option<(i32, i32)> {
    match p.kind {
        // ~60% of ruins hide a chest at the center
        StructureKind::Ruins => {
            (unit(hash(seed, 0xB1DE_0005, p.x, p.y)) < 0.60).then_some((p.x, p.y))
        }
        // every camp has one, beside the lean-to
        StructureKind::Camp => Some((p.x + 2, p.y)),
        _ => None,
    }
}

/// Spawn structure entities (loot chests) for a chunk that was just generated fresh.
/// Marks the chunk dirty so it persists to disk and never generates fresh again —
/// that's what prevents duplicate chests.
pub fn spawn_chunk_entities(g: &mut Game, lvl: usize, cx: i32, cy: i32) {
    if g.level(lvl).depth != 0 || !g.level(lvl).is_infinite() {
        return;
    }
    let seed = g.world_seed;
    let base_x = cx * CHUNK_SIZE;
    let base_y = cy * CHUNK_SIZE;
    let placements = placements_in_rect(
        seed,
        base_x - MAX_RADIUS,
        base_y - MAX_RADIUS,
        base_x + CHUNK_SIZE - 1 + MAX_RADIUS,
        base_y + CHUNK_SIZE - 1 + MAX_RADIUS,
    );
    for p in placements {
        let Some((tx, ty)) = chest_pos(seed, p) else {
            continue;
        };
        if chunk_coord(tx) != cx || chunk_coord(ty) != cy {
            continue; // another chunk owns this chest
        }
        let mut chest = crate::entity::furniture::chest::new();
        fill_structure_chest(g, &mut chest, p.kind, hash(seed, 0x100D_0006, tx, ty));
        g.level_mut(lvl).add_at(chest, tx, ty, true, lvl);
        // touch the tile's data byte (same value) purely to set the chunk's dirty flag
        let data = g.level(lvl).get_data(tx, ty);
        g.level_mut(lvl).set_data(tx, ty, data);
    }
}

/// Modest early-game loot, deterministic per chest position.
fn fill_structure_chest(
    g: &mut Game,
    chest: &mut crate::entity::Entity,
    kind: StructureKind,
    h: u64,
) {
    use crate::item::registry::get;
    let mut rnd = Rng::new(h as i64);

    // (1-in-chance, item, count) — same convention as the spawner-dungeon chests
    let loot: &[(i32, &str, i32)] = match kind {
        StructureKind::Ruins => &[
            (2, "Torch", 3),
            (2, "Stone", 6),
            (3, "Wood", 5),
            (3, "Cord", 2),
            (3, "Bread", 2),
            (4, "Apple", 2),
            (5, "Coal", 3),
            (10, "Iron", 1),
        ],
        _ => &[
            (2, "Torch", 2),
            (2, "Bread", 2),
            (2, "Wood", 4),
            (3, "Cord", 3),
            (4, "arrow", 4),
            (5, "Apple", 2),
            (12, "Iron", 1),
        ],
    };
    let inventory = &mut chest.chest_mut().expect("chest").inventory;
    for &(chance, name, num) in loot {
        let item = get(g, name);
        inventory.try_add_num(&mut rnd, chance, Some(item), num);
    }
    // never leave a completely empty chest
    if inventory.inv_size() < 1 {
        inventory.add_num(get(g, "Wood"), 4);
        inventory.add_num(get(g, "Torch"), 2);
    }
}
