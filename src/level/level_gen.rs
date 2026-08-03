//! The classic finite-world generator (surface, mines, dungeon). Originally ported from
//! the Java version (see tag v0.1.0) and kept bit-compatible with it: each
//! `create_and_validate_*` seeds a fresh `Rng::new(world_seed)`, so identical seeds
//! produce byte-identical maps (see `tests/level_gen_parity.rs`).
//!
//! Several oddities in the math below are deliberate for that reason and flagged
//! inline — "fixing" them would change every generated world. The one non-reproducible
//! part is the surface's ruins/graveyard decoration (`history_gen`, at the bottom),
//! which draws from the live game Rng rather than the world seed.

use crate::level::tile::Tiles;
use crate::level::tile::ids;
use crate::rng::Rng;

/// Minimum spacing between generated stairwells, in tiles.
const STAIR_RADIUS: i32 = 15;

/// Java `LevelGen` instances are just noise maps (the `values` array); the static map
/// builders are free functions below.
pub struct LevelGen {
    /// An array of doubles, used to help making noise for the map.
    pub values: Vec<f64>,
    w: i32,
    h: i32,
}

impl LevelGen {
    /// Java `LevelGen(w, h, featureSize)` — creates noise for level generation.
    /// The Java constructor draws from the shared static `random`; here it is passed in.
    pub fn new(w: i32, h: i32, feature_size: i32, random: &mut Rng) -> LevelGen {
        let mut lg = LevelGen {
            values: vec![0.0; (w * h) as usize],
            w,
            h,
        };

        // both loops deliberately bound by w, not h (levels are square; the bound is
        // part of the seeded noise layout)
        let mut y = 0;
        while y < w {
            let mut x = 0;
            while x < w {
                // sets the random value from -1 to 1 at the given coordinate.
                let v = random.next_float() * 2.0 - 1.0; // float math, then widened
                lg.set_sample(x, y, v as f64);
                x += feature_size;
            }
            y += feature_size;
        }

        let mut step_size = feature_size;
        // integer division on purpose: scale starts at 0 for any w > 2, so the first
        // pass is a pure average and randomness only enters via the scale_mod growth
        let mut scale: f64 = (2 / w) as f64;
        let mut scale_mod: f64 = 1.0;
        loop {
            let half_step = step_size / 2;
            let mut y = 0;
            while y < lg.h {
                let mut x = 0;
                while x < lg.w {
                    let a = lg.sample(x, y);
                    let b = lg.sample(x + step_size, y);
                    let c = lg.sample(x, y + step_size);
                    let d = lg.sample(x + step_size, y + step_size);

                    // f32 math first, widened to f64 only at the multiply with `scale`;
                    // the precision loss is part of the seeded noise shape
                    let e = (a + b + c + d) / 4.0
                        + ((random.next_float() * 2.0 - 1.0) * step_size as f32) as f64 * scale;
                    lg.set_sample(x + half_step, y + half_step, e);
                    x += step_size;
                }
                y += step_size;
            }

            let mut y = 0;
            while y < lg.h {
                let mut x = 0;
                while x < lg.w {
                    let a = lg.sample(x, y); // middle (current) tile
                    let b = lg.sample(x + step_size, y); // right tile
                    let c = lg.sample(x, y + step_size); // bottom tile
                    let d = lg.sample(x + half_step, y + half_step); // mid-right, mid-bottom
                    let e = lg.sample(x + half_step, y - half_step); // mid-right, mid-top
                    let f = lg.sample(x - half_step, y + half_step); // mid-left, mid-bottom

                    let hh = (a + b + d + e) / 4.0
                        + ((random.next_float() * 2.0 - 1.0) * step_size as f32) as f64
                            * scale
                            * 0.5;
                    let g = (a + c + d + f) / 4.0
                        + ((random.next_float() * 2.0 - 1.0) * step_size as f32) as f64
                            * scale
                            * 0.5;
                    lg.set_sample(x + half_step, y, hh); // mid-right
                    lg.set_sample(x, y + half_step, g); // mid-bottom
                    x += step_size;
                }
                y += step_size;
            }

            step_size /= 2;
            scale *= scale_mod + 0.8;
            scale_mod *= 0.3;
            if step_size <= 1 {
                break;
            }
        }
        lg
    }

    fn sample(&self, x: i32, y: i32) -> f64 {
        self.values[((x & (self.w - 1)) + (y & (self.h - 1)) * self.w) as usize]
    }

    fn set_sample(&mut self, x: i32, y: i32, value: f64) {
        self.values[((x & (self.w - 1)) + (y & (self.h - 1)) * self.w) as usize] = value;
    }
}

/// Java `createAndValidateMap(w, h, level)` returning `byte[][] {tiles, data}`.
///
/// `world_seed` replaces the `worldSeed = WorldGenDisplay.getSeed()` static;
/// `gen_type`/`theme` replace `Settings.get("Type")`/`Settings.get("Theme")`
/// ("Island"/"Box"/"Mountain"/"Irregular" and "Normal"/"Forest"/"Desert"/"Plain"/"Hell").
///
/// `history_random` drives the ruins/graveyard decoration and is *not* derived from the
/// world seed (in-game it is `g.random`), so two worlds with the same seed can differ
/// in their ruins.
#[allow(clippy::too_many_arguments)]
pub fn create_and_validate_map(
    w: i32,
    h: i32,
    level: i32,
    _tiles: &Tiles,
    world_seed: i64,
    gen_type: &str,
    theme: &str,
    history_random: &mut Rng,
) -> Option<(Vec<u8>, Vec<u8>)> {
    if level == 0 {
        return Some(create_and_validate_top_map(
            w,
            h,
            world_seed,
            gen_type,
            theme,
            history_random,
        ));
    }
    if level == -4 {
        return Some(create_and_validate_dungeon(w, h, world_seed));
    }

    if level > -4 && level < 0 {
        return Some(create_and_validate_underground_map(
            w, h, -level, world_seed,
        ));
    }

    eprintln!("LevelGen ERROR: level index is not valid. Could not generate a level.");

    None
}

fn create_and_validate_top_map(
    w: i32,
    h: i32,
    world_seed: i64,
    gen_type: &str,
    theme: &str,
    history_random: &mut Rng,
) -> (Vec<u8>, Vec<u8>) {
    let mut random = Rng::new(world_seed);
    let mut attempt = 0;
    loop {
        let result = create_top_map(w, h, &mut random, gen_type, theme);

        let mut count = [0i32; 256];

        for i in 0..(w * h) as usize {
            count[result.0[i] as usize] += 1;
        }

        attempt += 1;

        if count[(ids::ROCK.raw()) as usize] < 100 {
            continue;
        }
        if count[(ids::SAND.raw()) as usize] < 100 {
            continue;
        }
        if count[(ids::GRASS.raw()) as usize] < 100 {
            continue;
        }
        if count[(ids::TREE.raw()) as usize] < 100 {
            continue;
        }
        if count[(ids::SNOW.raw()) as usize] < 100 {
            continue;
        }
        if count[(ids::STAIRS_DOWN.raw()) as usize] < w / 21 {
            continue; // size 128 = 6 stairs min
        }
        if count[(ids::QUICK_SAND.raw()) as usize] < 1 {
            continue;
        }

        let mut attempt_history = 0;
        loop {
            // add human influence
            let humans = history_gen::add_history_to_map(&result, w, h, history_random);

            attempt_history += 1;
            if attempt_history > 5 {
                if attempt > 100 {
                    // I give up! Take this map and leave!
                    return result;
                }
                // lets try another world.
                break;
            }

            // check human land properties
            let mut count = [0i32; 256];

            for i in 0..(w * h) as usize {
                // the mask really is 0xfff, not 0xff: it keeps a sign-extended negative
                // byte from indexing past the 256-entry array, and tile ids never reach
                // 128, so in practice it behaves like & 0xff
                count[((humans.0[i] as i8 as i32) & 0xfff) as usize] += 1;
            }

            if count[(ids::FENCE.raw()) as usize] < 15 {
                continue;
            }
            if count[(ids::GRAVE_STONE.raw()) as usize] < 9 {
                continue;
            }

            return humans;
        }
    }
}

/// The cave/ore generator for the mines (depths -1..-3).
fn create_and_validate_underground_map(
    w: i32,
    h: i32,
    depth: i32,
    world_seed: i64,
) -> (Vec<u8>, Vec<u8>) {
    let mut random = Rng::new(world_seed);
    loop {
        let result = create_underground_map(w, h, depth, &mut random);

        let mut count = [0i32; 256];

        for i in 0..(w * h) as usize {
            count[result.0[i] as usize] += 1;
        }
        if count[(ids::ROCK.raw()) as usize] < 100 {
            continue;
        }
        if count[(ids::DIRT.raw()) as usize] < 100 {
            continue;
        }
        if count[((ids::IRON_ORE.raw()) as i32 + depth - 1) as usize] < 20 {
            continue;
        }

        if depth < 3 && count[(ids::STAIRS_DOWN.raw()) as usize] < w / 32 {
            continue; // size 128 = 4 stairs min
        }

        return result;
    }
}

fn create_and_validate_dungeon(w: i32, h: i32, world_seed: i64) -> (Vec<u8>, Vec<u8>) {
    let mut random = Rng::new(world_seed);
    loop {
        let result = create_dungeon(w, h, &mut random);

        let mut count = [0i32; 256];

        for i in 0..(w * h) as usize {
            count[result.0[i] as usize] += 1;
        }
        if count[(ids::OBSIDIAN.raw()) as usize] < 100 {
            continue;
        }
        if count[(ids::OBSIDIAN_WALL.raw()) as usize] < 100 {
            continue;
        }

        return result;
    }
}

fn create_top_map(
    w: i32,
    h: i32,
    random: &mut Rng,
    gen_type: &str,
    theme: &str,
) -> (Vec<u8>, Vec<u8>) {
    // creates a bunch of value maps, some with small size...
    let mnoise1 = LevelGen::new(w, h, 16, random);
    let mnoise2 = LevelGen::new(w, h, 16, random);
    let mnoise3 = LevelGen::new(w, h, 16, random);
    // ...and some with larger size.
    let noise1 = LevelGen::new(w, h, 32, random);
    let noise2 = LevelGen::new(w, h, 32, random);

    let mut map = vec![0u8; (w * h) as usize];
    let mut data = vec![0u8; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = (x + y * w) as usize;

            let val = (noise1.values[i] - noise2.values[i]).abs() * 3.0 - 2.0;
            let mval = (mnoise1.values[i] - mnoise2.values[i]).abs();
            let mval = (mval - mnoise3.values[i]).abs() * 3.0 - 2.0;

            // this calculates a sort of distance based on the current coordinate.
            let mut xd = x as f64 / (w as f64 - 1.0) * 2.0 - 1.0;
            let mut yd = y as f64 / (h as f64 - 1.0) * 2.0 - 1.0;
            if xd < 0.0 {
                xd = -xd;
            }
            if yd < 0.0 {
                yd = -yd;
            }
            let mut dist = if xd >= yd { xd } else { yd };
            dist = dist * dist * dist * dist;
            dist = dist * dist * dist * dist;
            let val = val + 1.0 - dist * 20.0;

            match gen_type {
                "Island" => {
                    if val < -0.5 {
                        if theme == "Hell" {
                            map[i] = ids::LAVA.raw();
                        } else {
                            map[i] = ids::WATER.raw();
                        }
                    } else if val > 0.5 && mval < -1.5 {
                        map[i] = ids::ROCK.raw();
                    } else {
                        map[i] = ids::GRASS.raw();
                    }
                }
                "Box" => {
                    if val < -1.5 {
                        if theme == "Hell" {
                            map[i] = ids::LAVA.raw();
                        } else {
                            map[i] = ids::WATER.raw();
                        }
                    } else if val > 0.5 && mval < -1.5 {
                        map[i] = ids::ROCK.raw();
                    } else {
                        map[i] = ids::GRASS.raw();
                    }
                }
                "Mountain" => {
                    if val < -0.4 {
                        map[i] = ids::GRASS.raw();
                    } else if val > 0.5 && mval < -1.5 {
                        if theme == "Hell" {
                            map[i] = ids::LAVA.raw();
                        } else {
                            map[i] = ids::WATER.raw();
                        }
                    } else {
                        map[i] = ids::ROCK.raw();
                    }
                }
                "Irregular" => {
                    if val < -0.5 && mval < -0.5 {
                        if theme == "Hell" {
                            map[i] = ids::LAVA.raw();
                        }
                        if theme != "Hell" {
                            map[i] = ids::WATER.raw();
                        }
                    } else if val > 0.5 && mval < -1.5 {
                        map[i] = ids::ROCK.raw();
                    } else {
                        map[i] = ids::GRASS.raw();
                    }
                }
                _ => {} // unrecognized gen type: tiles stay id 0 (grass)
            }
        }
    }

    for _ in 0..(w * h / 2800) {
        let xs = random.next_int_bound(w);
        let ys = random.next_int_bound(h);
        for _ in 0..10 {
            let x = xs + random.next_int_bound(21) - 10;
            let y = ys + random.next_int_bound(21) - 10;
            for _ in 0..100 {
                let xo = x + random.next_int_bound(5) - random.next_int_bound(5);
                let yo = y + random.next_int_bound(5) - random.next_int_bound(5);
                for yy in yo - 1..=yo + 1 {
                    for xx in xo - 1..=xo + 1 {
                        if xx >= 0 && yy >= 0 && xx < w && yy < h {
                            let idx = (xx + yy * w) as usize;
                            if map[idx] == ids::GRASS.raw() {
                                map[idx] = ids::SNOW.raw();
                            }
                        }
                    }
                }
            }
        }
    }

    for _ in 0..(w * h / 1200) {
        let x = random.next_int_bound(w);
        let y = random.next_int_bound(h);
        for _ in 0..45 {
            let xx = x + random.next_int_bound(5) - random.next_int_bound(5);
            let yy = y + random.next_int_bound(5) - random.next_int_bound(5);
            if xx >= 0 && yy >= 0 && xx < w && yy < h {
                let idx = (xx + yy * w) as usize;
                if map[idx] == ids::GRASS.raw() {
                    map[idx] = ids::PUMPKIN.raw();
                }
            }
        }
    }

    for _ in 0..(w * h / 1200) {
        let x = random.next_int_bound(w);
        let y = random.next_int_bound(h);

        for _ in 0..12 {
            let xx = x + random.next_int_bound(5) - random.next_int_bound(5);
            let yy = y + random.next_int_bound(5) - random.next_int_bound(5);
            if xx >= 0 && yy >= 0 && xx < w && yy < h {
                let idx = (xx + yy * w) as usize;
                if map[idx] == ids::GRASS.raw() {
                    map[idx] = ids::GRAVE_STONE.raw();
                }
            }
        }
    }

    if theme == "Desert" {
        for _ in 0..(w * h / 200) {
            let xs = random.next_int_bound(w);
            let ys = random.next_int_bound(h);
            for _ in 0..10 {
                let x = xs + random.next_int_bound(21) - 10;
                let y = ys + random.next_int_bound(21) - 10;
                for _ in 0..100 {
                    let xo = x + random.next_int_bound(5) - random.next_int_bound(5);
                    let yo = y + random.next_int_bound(5) - random.next_int_bound(5);
                    for yy in yo - 1..=yo + 1 {
                        for xx in xo - 1..=xo + 1 {
                            if xx >= 0 && yy >= 0 && xx < w && yy < h {
                                let idx = (xx + yy * w) as usize;
                                if map[idx] == ids::GRASS.raw() {
                                    map[idx] = ids::SAND.raw();
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if theme != "Desert" {
        for _ in 0..(w * h / 2800) {
            let xs = random.next_int_bound(w);
            let ys = random.next_int_bound(h);
            for _ in 0..10 {
                let x = xs + random.next_int_bound(21) - 10;
                let y = ys + random.next_int_bound(21) - 10;
                for _ in 0..100 {
                    let xo = x + random.next_int_bound(5) - random.next_int_bound(5);
                    let yo = y + random.next_int_bound(5) - random.next_int_bound(5);
                    for yy in yo - 1..=yo + 1 {
                        for xx in xo - 1..=xo + 1 {
                            if xx >= 0 && yy >= 0 && xx < w && yy < h {
                                let idx = (xx + yy * w) as usize;
                                if map[idx] == ids::GRASS.raw() {
                                    map[idx] = ids::SAND.raw();
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if theme == "Forest" {
        for _ in 0..(w * h / 200) {
            let x = random.next_int_bound(w);
            let y = random.next_int_bound(h);
            for _ in 0..200 {
                let xx = x + random.next_int_bound(15) - random.next_int_bound(15);
                let yy = y + random.next_int_bound(15) - random.next_int_bound(15);
                if xx >= 0 && yy >= 0 && xx < w && yy < h {
                    let idx = (xx + yy * w) as usize;
                    if map[idx] == ids::GRASS.raw() {
                        map[idx] = ids::TREE.raw();
                    }
                }
            }
        }
    }
    if theme != "Forest" && theme != "Plain" {
        for _ in 0..(w * h / 1200) {
            let x = random.next_int_bound(w);
            let y = random.next_int_bound(h);
            for _ in 0..200 {
                let xx = x + random.next_int_bound(15) - random.next_int_bound(15);
                let yy = y + random.next_int_bound(15) - random.next_int_bound(15);
                if xx >= 0 && yy >= 0 && xx < w && yy < h {
                    let idx = (xx + yy * w) as usize;
                    if map[idx] == ids::GRASS.raw() {
                        map[idx] = ids::TREE.raw();
                    } else if map[idx] == ids::SNOW.raw() {
                        map[idx] = ids::SNOW_TREE.raw();
                    }
                }
            }
        }
    }

    if theme == "Plain" {
        for _ in 0..(w * h / 2800) {
            let x = random.next_int_bound(w);
            let y = random.next_int_bound(h);
            for _ in 0..200 {
                let xx = x + random.next_int_bound(15) - random.next_int_bound(15);
                let yy = y + random.next_int_bound(15) - random.next_int_bound(15);
                if xx >= 0 && yy >= 0 && xx < w && yy < h {
                    let idx = (xx + yy * w) as usize;
                    if map[idx] == ids::GRASS.raw() {
                        map[idx] = ids::TREE.raw();
                    }
                }
            }
        }
    }
    if theme != "Plain" {
        for _ in 0..(w * h / 400) {
            let x = random.next_int_bound(w);
            let y = random.next_int_bound(h);
            for _ in 0..200 {
                let xx = x + random.next_int_bound(15) - random.next_int_bound(15);
                let yy = y + random.next_int_bound(15) - random.next_int_bound(15);
                if xx >= 0 && yy >= 0 && xx < w && yy < h {
                    let idx = (xx + yy * w) as usize;
                    if map[idx] == ids::GRASS.raw() {
                        map[idx] = ids::TREE.raw();
                    }
                }
            }
        }
    }

    for _ in 0..(w * h / 400) {
        let x = random.next_int_bound(w);
        let y = random.next_int_bound(h);
        let col = random.next_int_bound(4);
        for _ in 0..30 {
            let xx = x + random.next_int_bound(5) - random.next_int_bound(5);
            let yy = y + random.next_int_bound(5) - random.next_int_bound(5);
            if xx >= 0 && yy >= 0 && xx < w && yy < h {
                let idx = (xx + yy * w) as usize;
                if map[idx] == ids::GRASS.raw() {
                    map[idx] = ids::FLOWER.raw();
                    // data determines which way the flower faces
                    data[idx] = (col + random.next_int_bound(4) * 16) as u8;
                }
            }
        }
    }

    for _ in 0..(w * h / 100) {
        let xx = random.next_int_bound(w);
        let yy = random.next_int_bound(h);
        if xx >= 0 && yy >= 0 && xx < w && yy < h {
            let idx = (xx + yy * w) as usize;
            if map[idx] == ids::SAND.raw() {
                map[idx] = ids::CACTUS.raw();
            }
        }
    }

    for _ in 0..(w * h / 100) {
        let xx = random.next_int_bound(w);
        let yy = random.next_int_bound(h);
        if xx >= 0 && yy >= 0 && xx < w && yy < h {
            let idx = (xx + yy * w) as usize;
            if map[idx] == ids::SAND.raw() {
                map[idx] = ids::QUICK_SAND.raw();
                break;
            }
        }
    }

    let mut count = 0;

    'stairs_loop: for _ in 0..(w * h / 100) {
        // loops a certain number of times, more for bigger world sizes.
        let x = random.next_int_bound(w - 2) + 1;
        let y = random.next_int_bound(h - 2) + 1;

        // the first loop, which checks to make sure that a new stairs tile will be
        // completely surrounded by rock.
        for yy in y - 1..=y + 1 {
            for xx in x - 1..=x + 1 {
                if map[(xx + yy * w) as usize] != ids::ROCK.raw() {
                    continue 'stairs_loop;
                }
            }
        }

        // this should prevent any stairsDown tile from being within 30 tiles of any
        // other stairsDown tile.
        for yy in 0.max(y - STAIR_RADIUS)..=(h - 1).min(y + STAIR_RADIUS) {
            for xx in 0.max(x - STAIR_RADIUS)..=(w - 1).min(x + STAIR_RADIUS) {
                if map[(xx + yy * w) as usize] == ids::STAIRS_DOWN.raw() {
                    continue 'stairs_loop;
                }
            }
        }

        map[(x + y * w) as usize] = ids::STAIRS_DOWN.raw();

        count += 1;
        if count >= w / 21 {
            break;
        }
    }

    (map, data)
}

fn create_dungeon(w: i32, h: i32, random: &mut Rng) -> (Vec<u8>, Vec<u8>) {
    let noise1 = LevelGen::new(w, h, 8, random);
    let noise2 = LevelGen::new(w, h, 8, random);

    let mut map = vec![0u8; (w * h) as usize];
    let data = vec![0u8; (w * h) as usize];

    for y in 0..h {
        for x in 0..w {
            let i = (x + y * w) as usize;

            let val = (noise1.values[i] - noise2.values[i]).abs() * 3.0 - 2.0;

            let mut xd = x as f64 / (w as f64 - 1.1) * 2.0 - 1.0;
            let mut yd = y as f64 / (h as f64 - 1.1) * 2.0 - 1.0;
            if xd < 0.0 {
                xd = -xd;
            }
            if yd < 0.0 {
                yd = -yd;
            }
            let mut dist = if xd >= yd { xd } else { yd };
            dist = dist * dist * dist * dist;
            dist = dist * dist * dist * dist;
            let val = -val * 1.0 - 2.2;
            let val = val + 1.0 - dist * 2.0;

            if val < -0.35 {
                map[i] = ids::OBSIDIAN_WALL.raw();
            } else {
                map[i] = ids::OBSIDIAN.raw();
            }
        }
    }

    'lava_loop: for _ in 0..(w * h / 450) {
        let x = random.next_int_bound(w - 2) + 1;
        let y = random.next_int_bound(h - 2) + 1;

        for yy in y - 1..=y + 1 {
            for xx in x - 1..=x + 1 {
                if map[(xx + yy * w) as usize] != ids::OBSIDIAN_WALL.raw() {
                    continue 'lava_loop;
                }
            }
        }

        map[(x + y * w) as usize] = ids::LAVA.raw();
        map[(x + (y + 1) * w) as usize] = ids::LAVA.raw();
        map[(x + 1 + (y + 1) * w) as usize] = ids::LAVA.raw();
        map[(x + 1 + y * w) as usize] = ids::LAVA.raw();
    }

    (map, data)
}

/// Java `createUndergroundMap`.
fn create_underground_map(w: i32, h: i32, depth: i32, random: &mut Rng) -> (Vec<u8>, Vec<u8>) {
    let mnoise1 = LevelGen::new(w, h, 16, random);
    let mnoise2 = LevelGen::new(w, h, 16, random);
    let mnoise3 = LevelGen::new(w, h, 16, random);

    let nnoise1 = LevelGen::new(w, h, 16, random);
    let nnoise2 = LevelGen::new(w, h, 16, random);
    let nnoise3 = LevelGen::new(w, h, 16, random);

    let wnoise1 = LevelGen::new(w, h, 16, random);
    let wnoise2 = LevelGen::new(w, h, 16, random);
    let wnoise3 = LevelGen::new(w, h, 16, random);

    let noise1 = LevelGen::new(w, h, 32, random);
    let noise2 = LevelGen::new(w, h, 32, random);

    let mut map = vec![0u8; (w * h) as usize];
    let data = vec![0u8; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = (x + y * w) as usize;

            let val = (noise1.values[i] - noise2.values[i]).abs() * 3.0 - 2.0;

            let mval = (mnoise1.values[i] - mnoise2.values[i]).abs();
            let mval = (mval - mnoise3.values[i]).abs() * 3.0 - 2.0;

            let nval = (nnoise1.values[i] - nnoise2.values[i]).abs();
            let nval = (nval - nnoise3.values[i]).abs() * 3.0 - 2.0;

            // wval's first step is computed and discarded — its second step deliberately
            // reuses nval. The wnoise1/wnoise2 maps must still be constructed above so
            // the RNG stream (and thus seed parity) stays aligned.
            let _wval = (wnoise1.values[i] - wnoise2.values[i]).abs();
            let wval = (nval - wnoise3.values[i]).abs() * 3.0 - 2.0;

            let mut xd = x as f64 / (w as f64 - 1.0) * 2.0 - 1.0;
            let mut yd = y as f64 / (h as f64 - 1.0) * 2.0 - 1.0;
            if xd < 0.0 {
                xd = -xd;
            }
            if yd < 0.0 {
                yd = -yd;
            }
            let dist = if xd >= yd { xd } else { yd };
            let dist = dist.powf(8.0);
            let val = val + 1.0 - dist * 20.0;

            // integer division first is deliberate: depth / 2 * 3 = 0, 3, 3 for depths 1..=3
            if val > -1.0 && wval < -1.0 + (depth / 2 * 3) as f64 {
                if depth == 3 {
                    map[i] = ids::LAVA.raw();
                } else if depth == 1 {
                    map[i] = ids::DIRT.raw();
                } else {
                    map[i] = ids::WATER.raw();
                }
            } else if val > -2.0 && (mval < -1.7 || nval < -1.4) {
                map[i] = ids::DIRT.raw();
            } else {
                map[i] = ids::ROCK.raw();
            }
        }
    }
    {
        let r = 2;
        for _ in 0..(w * h / 400) {
            let x = random.next_int_bound(w);
            let y = random.next_int_bound(h);
            for _ in 0..30 {
                let xx = x + random.next_int_bound(5) - random.next_int_bound(5);
                let yy = y + random.next_int_bound(5) - random.next_int_bound(5);
                if xx >= r && yy >= r && xx < w - r && yy < h - r {
                    let idx = (xx + yy * w) as usize;
                    if map[idx] == ids::ROCK.raw() {
                        map[idx] = ((ids::IRON_ORE.raw()) as i32 + depth - 1) as u8;
                    }
                }
            }
            for _ in 0..10 {
                let xx = x + random.next_int_bound(3) - random.next_int_bound(2);
                let yy = y + random.next_int_bound(3) - random.next_int_bound(2);
                if xx >= r && yy >= r && xx < w - r && yy < h - r {
                    let idx = (xx + yy * w) as usize;
                    if map[idx] == ids::ROCK.raw() {
                        map[idx] = ids::LAPIS.raw();
                    }
                }
            }
        }
    }

    if depth > 2 {
        let r = 1;
        // The dungeon entrance always lands at (60, 60): xx/yy never change, so the
        // loops below just redraw the same 5x5 stamp over itself. Kept as-is — the
        // fixed position is load-bearing for existing worlds.
        let xx = 60;
        let yy = 60;
        for _ in 0..(w * h / 380) {
            for _ in 0..10 {
                if xx < w - r && yy < h - r {
                    let ow = ids::OBSIDIAN_WALL.raw();
                    let ob = ids::OBSIDIAN.raw();
                    map[(xx + yy * w) as usize] = ow;
                    map[(xx + 1 + yy * w) as usize] = ow;
                    map[(xx + (yy + 1) * w) as usize] = ow;
                    map[(xx + 2 + yy * w) as usize] = ow;
                    map[(xx + (yy + 2) * w) as usize] = ow;
                    map[(xx + 3 + yy * w) as usize] = ow;
                    map[(xx + (yy + 3) * w) as usize] = ow;
                    map[(xx + 4 + yy * w) as usize] = ow;
                    map[(xx + (yy + 4) * w) as usize] = ow;
                    map[(xx + 4 + (yy + 1) * w) as usize] = ow;
                    map[(xx + 4 + (yy + 2) * w) as usize] = ow;
                    map[(xx + 4 + (yy + 3) * w) as usize] = ow;
                    map[(xx + 4 + (yy + 4) * w) as usize] = ow;
                    map[(xx + 3 + (yy + 1) * w) as usize] = ob;
                    map[(xx + 3 + (yy + 2) * w) as usize] = ob;
                    map[(xx + 3 + (yy + 3) * w) as usize] = ob;
                    map[(xx + 3 + (yy + 4) * w) as usize] = ow;
                    map[(xx + 2 + (yy + 1) * w) as usize] = ob;
                    map[(xx + 2 + (yy + 2) * w) as usize] = ids::STAIRS_DOWN.raw();
                    map[(xx + 2 + (yy + 3) * w) as usize] = ob;
                    map[(xx + 2 + (yy + 4) * w) as usize] = ow;
                    map[(xx + 1 + (yy + 1) * w) as usize] = ob;
                    map[(xx + 1 + (yy + 2) * w) as usize] = ob;
                    map[(xx + 1 + (yy + 3) * w) as usize] = ob;
                    map[(xx + 1 + (yy + 4) * w) as usize] = ow;
                }
            }
        }
    }

    if depth < 3 {
        let mut count = 0;
        'stairs_loop: for _ in 0..(w * h / 100) {
            let x = random.next_int_bound(w - 20) + 10;
            let y = random.next_int_bound(h - 20) + 10;

            for yy in y - 1..=y + 1 {
                for xx in x - 1..=x + 1 {
                    if map[(xx + yy * w) as usize] != ids::ROCK.raw() {
                        continue 'stairs_loop;
                    }
                }
            }

            // this should prevent any stairsDown tile from being within 30 tiles of any
            // other stairsDown tile.
            for yy in 0.max(y - STAIR_RADIUS)..=(h - 1).min(y + STAIR_RADIUS) {
                for xx in 0.max(x - STAIR_RADIUS)..=(w - 1).min(x + STAIR_RADIUS) {
                    if map[(xx + yy * w) as usize] == ids::STAIRS_DOWN.raw() {
                        continue 'stairs_loop;
                    }
                }
            }

            map[(x + y * w) as usize] = ids::STAIRS_DOWN.raw();
            count += 1;
            if count >= w / 32 {
                break;
            }
        }
    }

    (map, data)
}

/// "Human history" decoration for the surface map: stamps ruins (currently only
/// graveyards) onto suitable terrain clusters.
mod history_gen {
    use crate::level::tile::ids;
    use crate::rng::Rng;

    // Patterns store tile ids as signed bytes; -1 ("O") is transparent.
    struct Patterns {
        #[allow(dead_code)]
        hut1: Vec<Vec<i8>>,
        graveyard1: Vec<Vec<i8>>,
    }

    fn make_patterns() -> Patterns {
        let o: i8 = -1; // zer0 (transparent)
        let g = ids::GRASS.raw() as i8; // Grass
        let d = ids::DIRT.raw() as i8; // Dirt
        let w = ids::WOOD_WALL.raw() as i8; // Wooden Wall
        let f = ids::FENCE.raw() as i8; // Fence
        let s = ids::STONE_WALL.raw() as i8; // Stone wall
        let p = ids::STONE_BRICKS.raw() as i8; // Stone floor
        let gr = ids::GRAVE_STONE.raw() as i8; // Grave stone
        let _ = (o, g, s, p); // unused by the active patterns

        Patterns {
            hut1: vec![vec![w, w, w], vec![w, d, w]],
            graveyard1: vec![
                vec![f, f, f, f, f],
                vec![f, gr, gr, gr, f],
                vec![f, gr, gr, gr, f],
                vec![f, gr, gr, gr, f],
                vec![f, f, d, f, f],
            ],
        }
    }

    /// Java `HistoryGenPattern.transpose(x)`.
    fn transpose(x: &[Vec<i8>]) -> Vec<Vec<i8>> {
        let mut r = vec![vec![0i8; x.len()]; x[0].len()];
        for (i, row) in x.iter().enumerate() {
            for (j, &v) in row.iter().enumerate() {
                r[j][i] = v;
            }
        }
        r
    }

    /* ---------------- HistoryGen ---------------- */

    /// Stamp the surface map with ruins; returns the decorated (tiles, data) copy.
    /// `rand` is the live game Rng, not seed-derived (see `create_and_validate_map`).
    pub fn add_history_to_map(
        original_map: &(Vec<u8>, Vec<u8>),
        w: i32,
        h: i32,
        rand: &mut Rng,
    ) -> (Vec<u8>, Vec<u8>) {
        let mut map = original_map.0.clone();
        let data = original_map.1.clone();

        let forest_ids: [i8; 3] = [
            ids::TREE.raw() as i8,
            ids::GRASS.raw() as i8,
            ids::DIRT.raw() as i8,
        ];

        // the graveyard is the only pattern actually placed (hut1 is kept but unused)
        let patterns = make_patterns();
        let scenery_forest: Vec<&Vec<Vec<i8>>> = vec![&patterns.graveyard1];

        // generate forest scenery
        let places = rand.next_int_bound(5) + 2;
        for _ in 0..places {
            add_random_scenery_item(&mut map, &scenery_forest, 8, &forest_ids, 0.9, w, h, rand);
        }

        (map, data)
    }

    /// Java `HistoryGen.addRandomSceneryItem(map, scenery, margin, tileIds, threshold)`.
    #[allow(clippy::too_many_arguments)]
    fn add_random_scenery_item(
        map: &mut [u8],
        scenery: &[&Vec<Vec<i8>>],
        margin: i32,
        tile_ids: &[i8],
        threshold: f64,
        w: i32,
        h: i32,
        rand: &mut Rng,
    ) -> bool {
        // Pick a random pattern
        let pattern = scenery[rand.next_int_bound(scenery.len() as i32) as usize];

        // find a tile cluster
        let mut radius = if pattern.len() > pattern[0].len() {
            pattern.len() as i32
        } else {
            pattern[0].len() as i32
        };

        radius += margin;
        let pos = find_tile_cluster(map, tile_ids, radius, threshold, w, h, rand);
        if pos == -1 {
            // could not find a location
            return false;
        }

        // place pattern randomly
        let x = pos % w;
        let y = pos / w;

        apply_pattern(map, pattern, x, y, rand.next_int_bound(8), w, h);

        true
    }

    /// Java `HistoryGen.findTileCluster(map, tileIds, radius, threshold)`.
    fn find_tile_cluster(
        map: &[u8],
        tile_ids: &[i8],
        radius: i32,
        threshold: f64,
        w: i32,
        h: i32,
        rand: &mut Rng,
    ) -> i32 {
        let mut attempt = 0;
        while attempt < 100 {
            attempt += 1;

            // find random position
            let position = rand.next_int_bound(w * h);

            // inspect the area
            let mut tiles_ok = 0i32;
            for x in 0..radius {
                for y in 0..radius {
                    let xr = (position % w) + x - radius / 2;
                    let yr = (position / w) + y - radius / 2;
                    let i = xr + yr * w;
                    if xr <= 0 || xr >= w || yr <= 0 || yr >= h {
                        continue;
                    }
                    // check for any of the valid tiles
                    for &t in tile_ids {
                        if map[i as usize] as i8 == t {
                            tiles_ok += 1;
                            break;
                        }
                    }
                }
            }

            // integer division is deliberate: share is 0.0 unless *every* tile in the
            // area matched, so any threshold in (0, 1] means "all tiles must match"
            let share = (tiles_ok / (radius * radius)) as f64;
            if share >= threshold {
                return position;
            }
        }
        -1
    }

    /// Java `HistoryGen.applyPattern(map, pattern, x, y, dir)`.
    fn apply_pattern(
        map: &mut [u8],
        pattern: &[Vec<i8>],
        x: i32,
        y: i32,
        dir: i32,
        w: i32,
        h: i32,
    ) {
        // prepare pattern; transpose it if necessary
        let transposed;
        let p: &[Vec<i8>] = if dir / 4 == 0 {
            pattern
        } else {
            transposed = transpose(pattern);
            &transposed
        };

        for ya in 0..p.len() as i32 {
            for xa in 0..p[ya as usize].len() as i32 {
                let yp = if dir / 2 == 0 {
                    ya
                } else {
                    p.len() as i32 - (ya + 1)
                };
                let xp = if dir % 2 == 0 {
                    xa
                } else {
                    p[ya as usize].len() as i32 - (xa + 1)
                };
                if p[yp as usize][xp as usize] <= 0 {
                    // `<= 0` skips Grass cells (id 0) as well as transparent (-1)
                    continue;
                }
                if x + xa < 0 || x + xa >= w || y + ya < 0 || y + ya >= h {
                    continue;
                }
                map[((x + xa) + (y + ya) * w) as usize] = p[yp as usize][xp as usize] as u8;
            }
        }
    }
}
