//! Position-evaluable world generation for infinite levels.
//!
//! Unlike the classic whole-map generator (`level_gen.rs`, still used for the finite sky
//! and dungeon set-pieces), every function here is a pure function of
//! `(world seed, depth, tile x, tile y)`, so any chunk can be generated independently in
//! any order and always comes out the same.
//!
//! Layer plan (depths as in the classic game):
//! -  0: surface — water/sand/grass/trees/rock outcrops, biome-varied
//! - -1..-3: mines — rock with carved caves, dirt floors, depth-appropriate ore veins,
//!   lava pockets deeper down
//!
//! Stairs are placed on a deterministic grid-hash so that layer N's "stairs down" always
//! has a matching "stairs up" (with cleared surroundings) on layer N-1.

use super::chunk::{CHUNK_SIZE, Chunk};
use super::tile::Tiles;

/* ---------------------------------- hashing/noise ---------------------------------- */

/// SplitMix64-style avalanche over the packed inputs.
fn hash(seed: i64, salt: u64, x: i32, y: i32) -> u64 {
    let mut z = (seed as u64)
        ^ salt.wrapping_mul(0x9E3779B97F4A7C15)
        ^ (x as u32 as u64).wrapping_mul(0xC2B2AE3D27D4EB4F)
        ^ ((y as u32 as u64) << 32).wrapping_mul(0x165667B19E3779F9);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Uniform [0, 1) from a hash.
fn unit(h: u64) -> f64 {
    (h >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
}

/// Value noise: bilinear interpolation of hashed lattice values, `period` tiles apart.
fn value_noise(seed: i64, salt: u64, x: i32, y: i32, period: i32) -> f64 {
    let fx = x.div_euclid(period);
    let fy = y.div_euclid(period);
    let tx = x.rem_euclid(period) as f64 / period as f64;
    let ty = y.rem_euclid(period) as f64 / period as f64;

    let v00 = unit(hash(seed, salt, fx, fy));
    let v10 = unit(hash(seed, salt, fx + 1, fy));
    let v01 = unit(hash(seed, salt, fx, fy + 1));
    let v11 = unit(hash(seed, salt, fx + 1, fy + 1));

    // smoothstep fade
    let sx = tx * tx * (3.0 - 2.0 * tx);
    let sy = ty * ty * (3.0 - 2.0 * ty);

    let a = v00 + (v10 - v00) * sx;
    let b = v01 + (v11 - v01) * sx;
    a + (b - a) * sy
}

/// Fractal (octaved) value noise in [0, 1).
fn fractal(seed: i64, salt: u64, x: i32, y: i32, base_period: i32, octaves: u32) -> f64 {
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut total = 0.0;
    let mut period = base_period;
    for o in 0..octaves {
        sum += value_noise(seed, salt.wrapping_add(o as u64 * 101), x, y, period.max(1)) * amp;
        total += amp;
        amp *= 0.5;
        period /= 2;
    }
    sum / total
}

/* ------------------------------------- stairs -------------------------------------- */

/// Stairs live on a coarse grid: each `STAIR_GRID`² cell holds at most one stairwell,
/// jittered by hash. The same function answers for every layer, so layers always agree.
const STAIR_GRID: i32 = 48;

/// The stairwell position for a grid cell, if that cell has one connecting `depth` down
/// to `depth - 1` (surface 0 → -1, ... -2 → -3). Returns global tile coords.
pub fn stairwell_in_cell(seed: i64, depth: i32, cell_x: i32, cell_y: i32) -> Option<(i32, i32)> {
    if !(-2..=0).contains(&depth) {
        return None; // classic stairs to sky/dungeon are handled by set-piece gates
    }
    const STAIR_SALT: u64 = 0x57A1257A1257A125;
    let h = hash(
        seed,
        STAIR_SALT ^ depth.unsigned_abs() as u64,
        cell_x,
        cell_y,
    );
    // ~70% of cells have a stairwell to keep descent findable without a map
    if unit(h) > 0.7 {
        return None;
    }
    // jitter inside the cell, away from the border so the gate structure fits
    let jx = 4 + (h >> 8) as i32 % (STAIR_GRID - 8);
    let jy = 4 + (h >> 24) as i32 % (STAIR_GRID - 8);
    Some((cell_x * STAIR_GRID + jx, cell_y * STAIR_GRID + jy))
}

/// All stairwells (between `depth` and `depth - 1`) whose position lands within the given
/// tile rect. Query with a margin: gates carve a few tiles around the stairs.
pub fn stairwells_in_rect(
    seed: i64,
    depth: i32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for cy in (y0 - STAIR_GRID).div_euclid(STAIR_GRID)..=(y1 + STAIR_GRID).div_euclid(STAIR_GRID) {
        for cx in
            (x0 - STAIR_GRID).div_euclid(STAIR_GRID)..=(x1 + STAIR_GRID).div_euclid(STAIR_GRID)
        {
            if let Some((sx, sy)) = stairwell_in_cell(seed, depth, cx, cy) {
                if sx >= x0 && sx <= x1 && sy >= y0 && sy <= y1 {
                    out.push((sx, sy));
                }
            }
        }
    }
    out
}

/* ------------------------------------ tile rules ------------------------------------ */

struct Ids {
    grass: u8,
    dirt: u8,
    sand: u8,
    water: u8,
    lava: u8,
    rock: u8,
    tree: u8,
    cactus: u8,
    flower: u8,
    tall_grass: [u8; 3],
    iron: u8,
    gold: u8,
    gem: u8,
    lapis: u8,
    stairs_down: u8,
    stairs_up: u8,
}

impl Ids {
    fn get(tiles: &Tiles) -> Ids {
        Ids {
            grass: tiles.get("grass").id,
            dirt: tiles.get("dirt").id,
            sand: tiles.get("sand").id,
            water: tiles.get("water").id,
            lava: tiles.get("lava").id,
            rock: tiles.get("rock").id,
            tree: tiles.get("tree").id,
            cactus: tiles.get("cactus").id,
            flower: tiles.get("flower").id,
            tall_grass: [
                tiles.get("small grass").id,
                tiles.get("medium grass").id,
                tiles.get("tall grass").id,
            ],
            iron: tiles.get("iron ore").id,
            gold: tiles.get("gold ore").id,
            gem: tiles.get("gem ore").id,
            lapis: tiles.get("lapis").id,
            stairs_down: tiles.get("stairs down").id,
            stairs_up: tiles.get("stairs up").id,
        }
    }
}

/// The surface tile at a global position (before stairs/gates are stamped).
fn surface_tile(seed: i64, x: i32, y: i32, ids: &Ids) -> u8 {
    // smoother field for water/beach so lakes have clean shores (a high-frequency
    // octave here would speckle single water tiles all over the grass)
    let elevation = fractal(seed, 1, x, y, 64, 2);
    let moisture = fractal(seed, 2, x, y, 96, 3);
    let detail = unit(hash(seed, 3, x, y));

    if elevation < 0.34 {
        return ids.water;
    }
    if elevation < 0.37 {
        return ids.sand;
    }
    let ruggedness = fractal(seed, 5, x, y, 48, 4);
    if ruggedness > 0.78 {
        return ids.rock;
    }

    // dry pockets become deserts
    if moisture < 0.30 {
        if detail < 0.012 {
            return ids.cactus;
        }
        return ids.sand;
    }

    // forest density scales with moisture
    let tree_chance = ((moisture - 0.45) * 0.5).max(0.0) + 0.04;
    if detail < tree_chance {
        return ids.tree;
    }
    if detail < tree_chance + 0.015 {
        return ids.flower;
    }
    if detail < tree_chance + 0.055 {
        let which = (hash(seed, 4, x, y) % 3) as usize;
        return ids.tall_grass[which];
    }

    ids.grass
}

/// The mine tile at a global position for depth -1..-3 (before stairs are stamped).
fn mine_tile(seed: i64, depth: i32, x: i32, y: i32, ids: &Ids) -> u8 {
    let salt = 10 + depth.unsigned_abs() as u64;
    let cave = fractal(seed, salt, x, y, 32, 4);
    let detail = unit(hash(seed, salt + 90, x, y));

    // carved cave space vs solid rock
    if !(0.32..0.62).contains(&cave) {
        // solid rock, with ore veins hiding inside
        let vein = fractal(seed, salt + 40, x, y, 12, 2);
        if vein > 0.78 && detail < 0.6 {
            return match depth {
                -1 => {
                    if detail < 0.08 {
                        ids.lapis
                    } else {
                        ids.iron
                    }
                }
                -2 => {
                    if detail < 0.08 {
                        ids.lapis
                    } else {
                        ids.gold
                    }
                }
                _ => ids.gem,
            };
        }
        return ids.rock;
    }

    // open cave floor; deeper layers grow lava pockets
    let lava_threshold = match depth {
        -3 => 0.86,
        -2 => 0.93,
        _ => 0.985,
    };
    let pocket = fractal(seed, salt + 70, x, y, 24, 2);
    if pocket > lava_threshold {
        return ids.lava;
    }
    if pocket < 0.12 {
        return ids.water;
    }
    ids.dirt
}

/* ----------------------------------- chunk assembly ---------------------------------- */

/// Generate one chunk of an infinite layer. Pure: same inputs, same chunk.
pub fn generate_chunk(seed: i64, depth: i32, cx: i32, cy: i32, tiles: &Tiles) -> Chunk {
    let ids = Ids::get(tiles);
    let mut chunk = Chunk::new();

    let base_x = cx * CHUNK_SIZE;
    let base_y = cy * CHUNK_SIZE;

    for ly in 0..CHUNK_SIZE {
        for lx in 0..CHUNK_SIZE {
            let x = base_x + lx;
            let y = base_y + ly;
            let t = match depth {
                0 => surface_tile(seed, x, y, &ids),
                -1 | -2 | -3 => mine_tile(seed, depth, x, y, &ids),
                _ => ids.rock, // infinite gen only covers surface + mines
            };
            chunk.tiles[(lx + ly * CHUNK_SIZE) as usize] = t;
        }
    }

    // stamp stairwells: down-stairs on this layer, up-stairs from the layer above,
    // each with a small cleared apron so they're always enterable
    let margin = 2;
    let (x0, y0) = (base_x - margin, base_y - margin);
    let (x1, y1) = (
        base_x + CHUNK_SIZE - 1 + margin,
        base_y + CHUNK_SIZE - 1 + margin,
    );

    let clear_tile = if depth == 0 { ids.grass } else { ids.dirt };
    let mut stamp = |sx: i32, sy: i32, stairs: u8| {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (tx, ty) = (sx + dx, sy + dy);
                if tx >= base_x
                    && tx < base_x + CHUNK_SIZE
                    && ty >= base_y
                    && ty < base_y + CHUNK_SIZE
                {
                    let i = ((tx - base_x) + (ty - base_y) * CHUNK_SIZE) as usize;
                    chunk.tiles[i] = if dx == 0 && dy == 0 {
                        stairs
                    } else {
                        clear_tile
                    };
                }
            }
        }
    };

    // stairs down from this layer (surface..-2 lead downward)
    for (sx, sy) in stairwells_in_rect(seed, depth, x0, y0, x1, y1) {
        stamp(sx, sy, ids.stairs_down);
    }
    // stairs up to the layer above (mines -1..-3 receive the matching stairwell)
    if (-3..=-1).contains(&depth) {
        for (sx, sy) in stairwells_in_rect(seed, depth + 1, x0, y0, x1, y1) {
            stamp(sx, sy, ids.stairs_up);
        }
    }

    // set-piece gates: surface sky-towers (stairs up, hard-rock ring) and deep dungeon
    // gates (stairs down, obsidian ring) leading to the finite classic levels
    if depth == 0 || depth == -3 {
        let ring_id = if depth == 0 {
            tiles.get("hard rock").id
        } else {
            tiles.get("obsidian wall").id
        };
        let pad_id = if depth == 0 {
            ids.rock
        } else {
            tiles.get("obsidian").id
        };
        let stairs = if depth == 0 {
            ids.stairs_up
        } else {
            ids.stairs_down
        };
        for (gx, gy) in gates_in_rect(seed, depth, x0 - 2, y0 - 2, x1 + 2, y1 + 2) {
            for dy in -2..=2i32 {
                for dx in -2..=2i32 {
                    let (tx, ty) = (gx + dx, gy + dy);
                    if tx < base_x
                        || tx >= base_x + CHUNK_SIZE
                        || ty < base_y
                        || ty >= base_y + CHUNK_SIZE
                    {
                        continue;
                    }
                    let i = ((tx - base_x) + (ty - base_y) * CHUNK_SIZE) as usize;
                    let ring = dx.abs() == 2 || dy.abs() == 2;
                    // ring with a doorway on the south side
                    chunk.tiles[i] = if dx == 0 && dy == 0 {
                        stairs
                    } else if ring && !(dx == 0 && dy == 2) {
                        ring_id
                    } else {
                        pad_id
                    };
                }
            }
        }
    }

    chunk
}

/// Rare surface towers with stairs UP to the (finite) sky level, and rare dungeon gates
/// on the deepest mine with stairs DOWN to the (finite) dungeon. Coarser grid, lower odds
/// than regular stairwells.
const GATE_GRID: i32 = 160;

pub fn gate_in_cell(seed: i64, depth: i32, cell_x: i32, cell_y: i32) -> Option<(i32, i32)> {
    if depth != 0 && depth != -3 {
        return None;
    }
    const GATE_SALT: u64 = 0x6A7E6A7E6A7E6A7E;
    let h = hash(
        seed,
        GATE_SALT ^ depth.unsigned_abs() as u64,
        cell_x,
        cell_y,
    );
    if unit(h) > 0.5 {
        return None;
    }
    let jx = 8 + (h >> 8) as i32 % (GATE_GRID - 16);
    let jy = 8 + (h >> 24) as i32 % (GATE_GRID - 16);
    Some((cell_x * GATE_GRID + jx, cell_y * GATE_GRID + jy))
}

pub fn gates_in_rect(seed: i64, depth: i32, x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for cy in (y0 - GATE_GRID).div_euclid(GATE_GRID)..=(y1 + GATE_GRID).div_euclid(GATE_GRID) {
        for cx in (x0 - GATE_GRID).div_euclid(GATE_GRID)..=(x1 + GATE_GRID).div_euclid(GATE_GRID) {
            if let Some((gx, gy)) = gate_in_cell(seed, depth, cx, cy) {
                if gx >= x0 && gx <= x1 && gy >= y0 && gy <= y1 {
                    out.push((gx, gy));
                }
            }
        }
    }
    out
}

/// A good spawn position near the origin on the surface: the first grass tile on an
/// outward spiral (bounded; falls back to (0, 0)).
pub fn find_surface_spawn(seed: i64, tiles: &Tiles) -> (i32, i32) {
    let ids = Ids::get(tiles);
    for radius in 0i32..200 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx.abs() != radius && dy.abs() != radius {
                    continue; // ring only
                }
                if surface_tile(seed, dx, dy, &ids) == ids.grass {
                    return (dx, dy);
                }
            }
        }
    }
    (0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_generation_is_deterministic() {
        let tiles = Tiles::new();
        let a = generate_chunk(1234, 0, 3, -2, &tiles);
        let b = generate_chunk(1234, 0, 3, -2, &tiles);
        assert_eq!(a.tiles, b.tiles);
        let c = generate_chunk(1235, 0, 3, -2, &tiles);
        assert_ne!(a.tiles, c.tiles);
    }

    #[test]
    fn stairs_pair_across_layers() {
        let tiles = Tiles::new();
        let seed = 777;
        let down_id = tiles.get("stairs down").id;
        let up_id = tiles.get("stairs up").id;

        // find a stairwell from the surface, then check the mine chunk below has the
        // matching up-stairs at the same global position
        let stairs = stairwells_in_rect(seed, 0, -500, -500, 500, 500);
        assert!(!stairs.is_empty(), "no stairwells within 1000x1000 tiles");
        let (sx, sy) = stairs[0];

        let cx = sx >> super::super::chunk::CHUNK_SHIFT;
        let cy = sy >> super::super::chunk::CHUNK_SHIFT;
        let surface = generate_chunk(seed, 0, cx, cy, &tiles);
        let mine = generate_chunk(seed, -1, cx, cy, &tiles);

        let lx = sx & (CHUNK_SIZE - 1);
        let ly = sy & (CHUNK_SIZE - 1);
        let i = (lx + ly * CHUNK_SIZE) as usize;
        assert_eq!(surface.tiles[i], down_id, "surface should have stairs down");
        assert_eq!(mine.tiles[i], up_id, "mine should have matching stairs up");
    }

    #[test]
    fn surface_has_reasonable_biome_mix() {
        let tiles = Tiles::new();
        let ids = Ids::get(&tiles);
        let mut counts = std::collections::HashMap::new();
        for y in -128..128 {
            for x in -128..128 {
                *counts.entry(surface_tile(42, x, y, &ids)).or_insert(0usize) += 1;
            }
        }
        let total = 256 * 256;
        let grass = counts.get(&ids.grass).copied().unwrap_or(0);
        let water = counts.get(&ids.water).copied().unwrap_or(0);
        let trees = counts.get(&ids.tree).copied().unwrap_or(0);
        assert!(grass * 100 / total > 20, "grass too rare: {grass}/{total}");
        assert!(water > 0, "no water at all");
        assert!(trees > 50, "almost no trees: {trees}");
    }

    #[test]
    fn mines_have_ores() {
        let tiles = Tiles::new();
        let ids = Ids::get(&tiles);
        for (depth, ore) in [(-1, ids.iron), (-2, ids.gold), (-3, ids.gem)] {
            let mut n = 0;
            for y in -128..128 {
                for x in -128..128 {
                    if mine_tile(999, depth, x, y, &ids) == ore {
                        n += 1;
                    }
                }
            }
            assert!(n > 40, "depth {depth}: only {n} ore tiles in 256x256");
        }
    }
}
