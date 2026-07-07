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
pub(crate) fn hash(seed: i64, salt: u64, x: i32, y: i32) -> u64 {
    let mut z = (seed as u64)
        ^ salt.wrapping_mul(0x9E3779B97F4A7C15)
        ^ (x as u32 as u64).wrapping_mul(0xC2B2AE3D27D4EB4F)
        ^ ((y as u32 as u64) << 32).wrapping_mul(0x165667B19E3779F9);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Uniform [0, 1) from a hash.
pub(crate) fn unit(h: u64) -> f64 {
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
    snow: u8,
    snow_tree: u8,
    deep_water: u8,
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
            snow: tiles.get("snow").id,
            snow_tree: tiles.get("snow tree").id,
            deep_water: tiles.get("Deep Water").id,
            iron: tiles.get("iron ore").id,
            gold: tiles.get("gold ore").id,
            gem: tiles.get("gem ore").id,
            lapis: tiles.get("lapis").id,
            stairs_down: tiles.get("stairs down").id,
            stairs_up: tiles.get("stairs up").id,
        }
    }
}

/// The biome at a global surface position — Minecraft-scale regions from
/// continental-frequency temperature/moisture fields (Whittaker-style table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Biome {
    Ocean,
    DeepOcean,
    Beach,
    Mountains,
    Tundra,
    Desert,
    Marsh,
    Forest,
    Savanna,
    Plains,
}

pub fn biome_at(seed: i64, x: i32, y: i32) -> Biome {
    // continental fields: several-hundred-tile features so biomes feel expansive
    let continent = fractal(seed, 1, x, y, 384, 3);
    let temperature = fractal(seed, 6, x, y, 512, 2);
    let moisture = fractal(seed, 2, x, y, 448, 2);
    // local ruggedness: mountain ranges + irregular coastlines
    let rough = fractal(seed, 5, x, y, 48, 4);

    let land = continent + (rough - 0.5) * 0.08;
    if land < 0.36 {
        return Biome::DeepOcean; // open ocean: too deep to swim, raft country
    }
    if land < 0.42 {
        return Biome::Ocean;
    }
    if land < 0.445 {
        return Biome::Beach;
    }
    // ranges: a broad mountain belt refined by local ruggedness
    let belt = fractal(seed, 9, x, y, 320, 2);
    if belt > 0.70 && rough > 0.55 {
        return Biome::Mountains;
    }

    if temperature < 0.35 {
        Biome::Tundra
    } else if temperature > 0.68 && moisture < 0.42 {
        Biome::Desert
    } else if moisture > 0.74 {
        Biome::Marsh
    } else if moisture > 0.52 {
        Biome::Forest
    } else if moisture < 0.34 {
        Biome::Savanna
    } else {
        Biome::Plains
    }
}

/// The surface tile at a global position (before stairs/gates are stamped).
fn surface_tile(seed: i64, x: i32, y: i32, ids: &Ids) -> u8 {
    let detail = unit(hash(seed, 3, x, y));
    let tuft = |salt: u64| {
        let which = (hash(seed, salt, x, y) % 3) as usize;
        ids.tall_grass[which]
    };

    match biome_at(seed, x, y) {
        Biome::Ocean => ids.water,
        Biome::DeepOcean => ids.deep_water,
        Biome::Beach => ids.sand,
        Biome::Mountains => ids.rock,
        Biome::Tundra => {
            // snowfields with scattered firs and the odd bare rock
            if detail < 0.055 {
                ids.snow_tree
            } else if detail < 0.062 {
                ids.rock
            } else {
                ids.snow
            }
        }
        Biome::Desert => {
            if detail < 0.014 {
                ids.cactus
            } else if detail < 0.020 {
                ids.rock
            } else {
                ids.sand
            }
        }
        Biome::Marsh => {
            // blobby pools (mid-frequency, so no lonely single water tiles)
            let pool = fractal(seed, 7, x, y, 14, 2);
            if pool > 0.66 {
                return ids.water;
            }
            if detail < 0.16 {
                tuft(4)
            } else if detail < 0.175 {
                ids.flower
            } else {
                ids.grass
            }
        }
        Biome::Forest => {
            // dense canopy with clearings
            let clearing = fractal(seed, 8, x, y, 24, 2);
            let trees = if clearing > 0.62 { 0.03 } else { 0.16 };
            if detail < trees {
                ids.tree
            } else if detail < trees + 0.05 {
                tuft(4)
            } else {
                ids.grass
            }
        }
        Biome::Savanna => {
            // dry open country: lone trees, lots of dry tufts, blobby parched patches
            // (a mid-frequency mask — single scattered sand tiles read as noise)
            let parched = fractal(seed, 10, x, y, 18, 2);
            if parched > 0.74 {
                return ids.sand;
            }
            if detail < 0.008 {
                ids.tree
            } else if detail < 0.10 {
                tuft(4)
            } else {
                ids.grass
            }
        }
        Biome::Plains => {
            if detail < 0.015 {
                ids.tree
            } else if detail < 0.055 {
                ids.flower
            } else if detail < 0.10 {
                tuft(4)
            } else {
                ids.grass
            }
        }
    }
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
                -3..=-1 => mine_tile(seed, depth, x, y, &ids),
                _ => ids.rock, // infinite gen only covers surface + mines
            };
            chunk.tiles[(lx + ly * CHUNK_SIZE) as usize] = t;
        }
    }

    // surface structures (ruins, cemeteries, ...) — stamped before the gate set-pieces
    // below so a rare overlap always leaves the gate intact
    super::structures_gen::stamp_chunk(seed, depth, cx, cy, &mut chunk, tiles);

    // stamp stairwells: down-stairs on this layer, up-stairs from the layer above,
    // each with a small cleared apron so they're always enterable
    let margin = 2;
    let (x0, y0) = (base_x - margin, base_y - margin);
    let (x1, y1) = (
        base_x + CHUNK_SIZE - 1 + margin,
        base_y + CHUNK_SIZE - 1 + margin,
    );

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

/// Rare dungeon gates on the deepest mine with stairs DOWN to the (finite) dungeon.
/// Coarser grid, lower odds than regular stairwells.
const GATE_GRID: i32 = 160;

pub fn gate_in_cell(seed: i64, depth: i32, cell_x: i32, cell_y: i32) -> Option<(i32, i32)> {
    if depth != -3 {
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
    // coarse ring scan (biome regions are hundreds of tiles, so step by 4)
    for radius in 0i32..300 {
        let r = radius * 4;
        for dy in (-r..=r).step_by(4) {
            for dx in (-r..=r).step_by(4) {
                if dx.abs() != r && dy.abs() != r {
                    continue; // ring only
                }
                let hospitable = matches!(
                    biome_at(seed, dx, dy),
                    Biome::Plains | Biome::Forest | Biome::Savanna | Biome::Marsh
                );
                if hospitable && surface_tile(seed, dx, dy, &ids) == ids.grass {
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
    fn no_preplaced_stairs_on_infinite_layers() {
        // descent is dig-based now: generated chunks must not contain stairs tiles
        let tiles = Tiles::new();
        let down = tiles.get("stairs down").id;
        let up = tiles.get("stairs up").id;
        for depth in [0, -1, -2] {
            for (cx, cy) in [(0, 0), (3, -2), (-5, 7)] {
                let c = generate_chunk(777, depth, cx, cy, &tiles);
                assert!(
                    !c.tiles.iter().any(|&t| t == down || t == up),
                    "depth {depth} chunk ({cx},{cy}) has pre-placed stairs"
                );
            }
        }
    }

    #[test]
    fn biomes_are_large_and_all_present() {
        // sample a wide area on a coarse lattice: every biome family should appear,
        // and regions should be big (few biome changes along a straight walk)
        let seed = 424242;
        let mut seen = std::collections::HashSet::new();
        for y in (-4096..4096).step_by(64) {
            for x in (-4096..4096).step_by(64) {
                seen.insert(biome_at(seed, x, y));
            }
        }
        for b in [
            Biome::Ocean,
            Biome::Beach,
            Biome::Tundra,
            Biome::Desert,
            Biome::Forest,
            Biome::Plains,
        ] {
            assert!(seen.contains(&b), "biome {b:?} missing from 8k x 8k sample");
        }

        // region size: walking 2048 tiles shouldn't flip biome often
        let mut changes = 0;
        let mut last = biome_at(seed, -1024, 0);
        for x in -1024..1024 {
            let b = biome_at(seed, x, 0);
            if b != last {
                changes += 1;
                last = b;
            }
        }
        assert!(
            changes < 40,
            "biomes too small: {changes} changes over 2048 tiles"
        );
    }

    #[test]
    fn spawn_lands_on_grass() {
        let tiles = Tiles::new();
        for seed in [1, 999, -55, 20260707] {
            let (sx, sy) = find_surface_spawn(seed, &tiles);
            let ids = Ids::get(&tiles);
            assert_eq!(
                surface_tile(seed, sx, sy, &ids),
                ids.grass,
                "seed {seed}: spawn ({sx},{sy}) not grass"
            );
        }
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
