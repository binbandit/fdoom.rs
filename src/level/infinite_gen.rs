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
use super::tile::{Tiles, tidal};

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

/// The composite land/elevation field: continent (salt 1) plus coastal ruggedness
/// (salt 5). This is the single definition of "how high above the sea is this tile" —
/// `biome_at` thresholds it into DeepOcean/Ocean/Beach, the shore arms of
/// `surface_tile` carve the tidal band out of it, and `tile/tidal.rs` compares it
/// against the day-clock tide level to decide whether a Tidal Flat is submerged.
/// Those consumers must all read the *same* field, which is why they share this one
/// function instead of re-deriving it.
fn land_parts(seed: i64, x: i32, y: i32) -> (f64, f64) {
    let continent = fractal(seed, 1, x, y, 384, 3);
    let rough = fractal(seed, 5, x, y, 48, 4);
    (continent + (rough - 0.5) * 0.08, rough)
}

/// The land/elevation value at a tile — see `land_parts`.
pub fn land_at(seed: i64, x: i32, y: i32) -> f64 {
    land_parts(seed, x, y).0
}

/* ---------------------------------- fossicking fields --------------------------------- */

/// Salt of the mineral-richness field (see `richness_at`).
pub const RICHNESS_SALT: u64 = 0xF055_1C4E; // "fossicker"

/// Per-area mineral richness in `[0, 1)` — the "reading the land" field the fossicking
/// systems share. One creek-scale field for **every** depth: it scales the Prospector's
/// Pan find odds on the surface, decides which rock outcrops carry a mineral-seep
/// stain, and lowers the ore-vein threshold in the mines directly below — so a stained
/// outcrop or a paying creek genuinely marks good digging underneath.
pub fn richness_at(seed: i64, x: i32, y: i32) -> f64 {
    fractal(seed, RICHNESS_SALT, x, y, 96, 2)
}

/// Salt of the per-tile mineral-seep stain roll (raw hash, no fractal).
const STAIN_SALT: u64 = 0x5EE9; // "seep"

/// Does a rock outcrop at this surface position carry a visible mineral-seep stain?
/// True only on genuinely rich ground (the shared richness field, so the mines below
/// really are richer) and only on some tiles of it (per-tile hash), so stains read as
/// an occasional prospector's sign rather than a biome tint.
pub fn mineral_stain_at(seed: i64, x: i32, y: i32) -> bool {
    richness_at(seed, x, y) > 0.70 && unit(hash(seed, STAIN_SALT, x, y)) < 0.35
}

/// Highland ("tier-2 summit") rock: the upper band of the mountain belt, just below
/// the snow line. Reads the same belt/rough fields as `biome_at`'s Mountains gate —
/// the reuse is correct here, the tier *must* correlate with the biome. Highland rock
/// renders visibly raised, takes double damage to break, and drops extra stone (see
/// `tile/rock.rs`).
pub fn highland_at(seed: i64, x: i32, y: i32) -> bool {
    let (_, rough) = land_parts(seed, x, y);
    rough > 0.55 && fractal(seed, 9, x, y, 320, 2) > 0.75
}

/// Salt of the ocean skerry scatter (raw hash on 2x2 tile cells).
const SKERRY_SALT: u64 = 0x5EA0; // "sea"

/// Skerries: sparse permanent rock outcrops in open Ocean water — sea stacks the tide
/// never covers. Hashed per 2x2 cell so they surface as small stacks, not lone pixels.
fn skerry_at(seed: i64, x: i32, y: i32) -> bool {
    unit(hash(seed, SKERRY_SALT, x.div_euclid(2), y.div_euclid(2))) < 0.0025
}

/// Salts of the pond/pool shoreline raggedness (per-2x2-cell bites + per-tile teeth).
const POND_BITE_SALT: u64 = 0x50_4ED1; // "pond"
const POND_TOOTH_SALT: u64 = 0x50_4ED2;

/// Salt of the wild-beehive scatter on forest broadleaf trees (raw hash).
const BEEHIVE_SALT: u64 = 0xBEE5_0001;
/// Salt of the Badlands mesa/hoodoo cluster field (fractal, period 26).
const BADLANDS_MESA_SALT: u64 = 0xBAD_0001;
/// Salt of the Badlands ore-freckle scatter (raw hash; gated on `richness_at`).
const FRECKLE_SALT: u64 = 0xBAD_0003;

/// Ragged hash-contour for pond/pool outlines: a zero-mean jitter added to the blob
/// field before thresholding, so shorelines grow tile-scale teeth and 2x2-cell bites
/// instead of tracing the smooth noise contour as a hard tile-grid rectangle
/// (ODDITIES O5). Zero-mean and small, so pond count/placement/size stay put; pure
/// `f(seed, x, y)`, so chunk borders stay exact.
fn shore_ragged(seed: i64, x: i32, y: i32) -> f64 {
    // biased toward the chunky 2x2 bites: a heavy per-tile term makes salt-and-pepper
    // checkering along the contour instead of organic notches
    let bite = unit(hash(seed, POND_BITE_SALT, x.div_euclid(2), y.div_euclid(2))) - 0.5;
    let tooth = unit(hash(seed, POND_TOOTH_SALT, x, y)) - 0.5;
    bite * 0.024 + tooth * 0.007
}

/// The bank material ringing an inland pond: mud in grass country, but where the
/// blended biome edge has carried a pond into cold or hot-dry ground, the rim follows
/// — snowy margins toward tundra, sandy banks toward desert — so no warm mud ring
/// ever circles a snowfield pond (ODDITIES O5).
fn pond_rim(seed: i64, x: i32, y: i32, ids: &Ids) -> u8 {
    let climate = climate_at(seed, x, y);
    if climate < 0.33 {
        return ids.snow;
    }
    if climate > 0.67 && fractal(seed, 2, x, y, 448, 2) < 0.42 {
        return ids.sand;
    }
    ids.mud
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
    heath: u8,
    deep_water: u8,
    mud: u8,
    // flora wave
    pine: u8,
    dead_tree: u8,
    willow: u8,
    palm: u8,
    flat_crown: u8,
    berry_bush: u8,
    mushroom: u8,
    fruiting_cactus: u8,
    seaweed: u8,
    coral: u8,
    tidal_flat: u8,
    reeds: u8,
    dry_bush: u8,
    // farming wave
    wild_carrot: u8,
    pumpkin: u8,
    // content wave
    beehive: u8,
    clay: u8,
    ore_freckle: u8,
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
            heath: tiles.get("Heath").id,
            deep_water: tiles.get("Deep Water").id,
            mud: tiles.get("Mud").id,
            pine: tiles.get("Pine Tree").id,
            dead_tree: tiles.get("Dead Tree").id,
            willow: tiles.get("Willow").id,
            palm: tiles.get("Palm Tree").id,
            flat_crown: tiles.get("Flat-Crown Tree").id,
            berry_bush: tiles.get("Berry Bush").id,
            mushroom: tiles.get("Mushroom").id,
            fruiting_cactus: tiles.get("Fruiting Cactus").id,
            seaweed: tiles.get("Seaweed").id,
            coral: tiles.get("Coral").id,
            tidal_flat: tiles.get("Tidal Flat").id,
            reeds: tiles.get("Reeds").id,
            dry_bush: tiles.get("Dry Bush").id,
            wild_carrot: tiles.get("Wild Carrot").id,
            pumpkin: tiles.get("pumpkin").id,
            beehive: tiles.get("Beehive").id,
            clay: tiles.get("Layered Clay").id,
            ore_freckle: tiles.get("Ore Freckle").id,
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
    /// Dry eroded canyon country: the bone-dry core of the hot-dry envelope, carved
    /// from *inside* the Desert gate so the tundra-gap property covers it for free.
    Badlands,
    Marsh,
    Forest,
    Savanna,
    Plains,
}

/// Continental climate — the smooth backbone of the temperature field: octave 0
/// (period 512) of the same salt-6 noise the old two-octave `temperature` read, so
/// climate zones sit where they always did, minus the fine-octave wobble. The
/// extreme-climate gates in `biome_at` threshold THIS field because single-octave
/// value noise has a hard gradient bound (~1.5/512 per tile per axis): the cold gate
/// (`< 0.30`) and the hot gate (`> 0.70`) can never come closer than ~100 tiles, and
/// the warm-dry Savanna gate (`> 0.42`) stays ~30+ tiles from the cold gate — so snow
/// never touches sand, even after `biome_at_blended`'s ±4-tile jitter. (Thresholds
/// are retuned vs the old two-octave 0.35/0.68 pair because the single-octave
/// marginal distribution has fatter tails; these keep tundra ~18% and desert ~5% of
/// climate-classified land, matching the old frequencies.)
/// Public: `core::weather` thresholds this same field for the cold-reach snow gate
/// (`weather::COLD_REACH`), so snowfall and snow accumulation track the biome map
/// exactly — the gradient bound above is what keeps dynamic snow away from sand too.
pub fn climate_at(seed: i64, x: i32, y: i32) -> f64 {
    fractal(seed, 6, x, y, 512, 1)
}

pub fn biome_at(seed: i64, x: i32, y: i32) -> Biome {
    // continental fields: several-hundred-tile features so biomes feel expansive
    let climate = climate_at(seed, x, y);
    let moisture = fractal(seed, 2, x, y, 448, 2);
    // land = continent + local ruggedness (irregular coastlines); rough also
    // refines the mountain belt below
    let (land, rough) = land_parts(seed, x, y);
    if land < 0.36 {
        return Biome::DeepOcean; // open ocean: too deep to swim, raft country
    }
    if land < 0.42 {
        return Biome::Ocean;
    }
    if land < 0.445 {
        return Biome::Beach;
    }
    // Ranges: a broad mountain belt with rough-modulated (ragged) edges. The old
    // gate ANDed `rough > 0.55` instead, which punched grass holes all through the
    // belt interior — the playtest's "green grass + boulder blobs" non-identity.
    // Additive modulation (±0.03) keeps the same outer envelope and ragged
    // coastline-style edges but fills the interior; the rough field still places
    // the rock crags, inside `surface_tile`'s Mountains arm.
    let belt = fractal(seed, 9, x, y, 320, 2);
    if belt + (rough - 0.5) * 0.08 > 0.70 {
        return Biome::Mountains;
    }

    if climate < 0.30 {
        Biome::Tundra
    } else if climate > 0.70 && moisture < 0.42 {
        // the hot-dry envelope: its parched core erodes into Badlands canyon
        // country, the rest is classic dune Desert. Both live strictly inside the
        // `climate > 0.70` gate, so the Tundra buffer bound applies to both.
        if moisture < 0.22 {
            Biome::Badlands
        } else {
            Biome::Desert
        }
    } else if moisture > 0.74 {
        Biome::Marsh
    } else if moisture > 0.48 {
        Biome::Forest
    } else if moisture < 0.34 && climate > 0.42 {
        // warm-dry only: cold-dry country stays Plains, so the sandy Savanna look
        // never runs up against Tundra snow (see `climate_at`)
        Biome::Savanna
    } else {
        Biome::Plains
    }
}

/// Biome lookup with per-tile domain warp: near boundaries the sample point jitters a
/// few tiles, so biomes interleave patchily instead of switching on a hard line —
/// patchy snow on tundra outskirts, sand freckles easing into deserts (user request).
pub fn biome_at_blended(seed: i64, x: i32, y: i32) -> Biome {
    let h = hash(seed, 0xB1E4D, x, y);
    let jx = ((h >> 4) % 9) as i32 - 4;
    let jy = ((h >> 12) % 9) as i32 - 4;
    biome_at(seed, x + jx, y + jy)
}

/// The surface tile at a global position (before stairs/gates are stamped).
fn surface_tile(seed: i64, x: i32, y: i32, ids: &Ids) -> u8 {
    let detail = unit(hash(seed, 3, x, y));
    let tuft = |salt: u64| {
        let which = (hash(seed, salt, x, y) % 3) as usize;
        ids.tall_grass[which]
    };

    match biome_at_blended(seed, x, y) {
        Biome::Ocean => {
            // the upper ocean strip is the intertidal band (tidal.rs), and the
            // shallow-water life clings to the permanently wet shelf just below it —
            // both read the exact land field biome_at thresholded (land_at)
            let land = land_at(seed, x, y);
            if (tidal::BAND_LOW..tidal::BAND_HIGH).contains(&land) {
                return ids.tidal_flat;
            }
            // skerries: rare rock stacks breaking the open water (never in the
            // intertidal band above, so they always read as permanent)
            if skerry_at(seed, x, y) {
                return ids.rock;
            }
            if land > 0.385 && land < tidal::BAND_LOW {
                if detail < 0.10 {
                    return ids.seaweed;
                }
                if detail < 0.13 {
                    return ids.coral;
                }
            }
            ids.water
        }
        Biome::DeepOcean => ids.deep_water,
        Biome::Beach => {
            // the lower beach edge sits inside the intertidal band (tidal.rs); the
            // upper beach — and its lone palms — stays above the highest tide
            if (tidal::BAND_LOW..tidal::BAND_HIGH).contains(&land_at(seed, x, y)) {
                return ids.tidal_flat;
            }
            if detail < 0.02 { ids.palm } else { ids.sand }
        }
        Biome::Mountains => {
            // Altitude beats climate (user request): the belt field doubles as
            // elevation, so the very highest peaks are snow-capped ANYWHERE — like
            // real mountains — while merely-high slopes only whiten where it's cold.
            let belt = fractal(seed, 9, x, y, 320, 2);
            if belt > 0.80 {
                return ids.snow; // summit line: snow in any climate
            }
            let temperature = fractal(seed, 6, x, y, 512, 2);
            if temperature < 0.42 && belt > 0.76 {
                return ids.snow; // cold ranges whiten further down the slopes
            }
            // crags: the rough field clusters solid rock into ridges and boulder
            // groups (exactly the tiles the old AND-gate classified as Mountains)
            let (_, rough) = land_parts(seed, x, y);
            if rough > 0.55 {
                return ids.rock;
            }
            if detail < 0.010 {
                return ids.rock; // lone boulders out on the open moor
            }
            // open highland between the crags: stony heath, its heather clusters
            // handled by the tile's own render (see tile/heath.rs)
            ids.heath
        }
        Biome::Tundra => {
            // snowfields with scattered pines/firs and the odd bare rock
            if detail < 0.030 {
                ids.pine
            } else if detail < 0.055 {
                ids.snow_tree
            } else if detail < 0.062 {
                ids.rock
            } else {
                ids.snow
            }
        }
        Biome::Badlands => {
            // eroded canyon country: banded clay flats between clustered mesas and
            // lone hoodoos, bone-dry scrub, and — where the shared richness field
            // runs high — exposed ore freckles, fossicking's waterless surface
            // tease. Deliberately NO water arm: the dryness IS the biome.
            let mesa = fractal(seed, BADLANDS_MESA_SALT, x, y, 26, 2);
            if mesa > 0.73 {
                return ids.rock; // mesa walls and hoodoo clusters
            }
            if detail < 0.005 {
                ids.rock // lone hoodoo out on the flat
            } else if detail < 0.011 {
                ids.dead_tree
            } else if detail < 0.034 {
                ids.dry_bush
            } else if richness_at(seed, x, y) > 0.55 && unit(hash(seed, FRECKLE_SALT, x, y)) < 0.04
            {
                ids.ore_freckle
            } else {
                ids.clay
            }
        }
        Biome::Desert => {
            if detail < 0.004 {
                ids.fruiting_cactus
            } else if detail < 0.014 {
                ids.cactus
            } else if detail < 0.018 {
                ids.dead_tree
            } else if detail < 0.026 {
                ids.dry_bush
            } else if detail < 0.032 {
                ids.rock
            } else {
                ids.sand
            }
        }
        Biome::Marsh => {
            // blobby pools (mid-frequency, so no lonely single water tiles), with a
            // boggy mud rim you wade through to reach the water, willows leaning
            // over it, and reed banks on the wet fringe
            // same ragged-contour treatment as the plains ponds; the mud rim is
            // right for a bog in any climate
            let pool = fractal(seed, 7, x, y, 14, 2) + shore_ragged(seed, x, y);
            if pool > 0.66 {
                return ids.water;
            }
            if pool > 0.60 {
                return ids.mud;
            }
            if pool > 0.54 {
                // wet fringe: the pool field doubles as "distance to water"
                if detail < 0.05 {
                    return ids.willow;
                }
                if detail < 0.50 {
                    return ids.reeds;
                }
            }
            // dry interior between pools: lone scraggly willows and reed tussocks
            // keep the bog reading marshy even when no pool is in frame
            if detail < 0.012 {
                ids.willow
            } else if detail < 0.05 {
                ids.reeds
            } else if detail < 0.16 {
                tuft(4)
            } else if detail < 0.175 {
                ids.flower
            } else {
                ids.grass
            }
        }
        Biome::Forest => {
            // dense canopy with clearings; the cold fringe toward tundra turns to
            // pines (same climate field as biome_at's Tundra gate)
            let clearing = fractal(seed, 8, x, y, 24, 2);
            let trees = if clearing > 0.65 { 0.04 } else { 0.30 };
            if detail < trees {
                if climate_at(seed, x, y) < 0.42 {
                    ids.pine
                } else if unit(hash(seed, BEEHIVE_SALT, x, y)) < 0.02 {
                    // bees & honey: the odd broadleaf carries a wild hive
                    ids.beehive
                } else {
                    ids.tree
                }
            } else if detail < trees + 0.008 {
                ids.berry_bush
            } else if detail < trees + 0.016 {
                ids.mushroom
            } else if detail < trees + 0.020 {
                // farming wave: wild carrots on the clearing floor
                ids.wild_carrot
            } else if detail < trees + 0.066 {
                tuft(4)
            } else {
                ids.grass
            }
        }
        Biome::Savanna => {
            // dry open country: lone flat-crown trees, dry bushes, lots of tufts,
            // blobby parched patches (a mid-frequency mask — single scattered sand
            // tiles read as noise)
            let parched = fractal(seed, 10, x, y, 18, 2);
            if parched > 0.74 {
                return ids.sand;
            }
            if detail < 0.008 {
                ids.flat_crown
            } else if detail < 0.016 {
                ids.dry_bush
            } else if detail < 0.10 {
                tuft(4)
            } else {
                ids.grass
            }
        }
        Biome::Plains => {
            // inland ponds with ragged shorelines and climate-aware bank rims
            let pond = fractal(seed, 12, x, y, 40, 2) + shore_ragged(seed, x, y);
            if pond > 0.83 {
                return ids.water;
            }
            if pond > 0.79 {
                return pond_rim(seed, x, y, ids);
            }
            // sweeping tall-grass meadows: dense thicket cores (impassable, fully
            // grown) fringed by younger growth
            let meadow = fractal(seed, 11, x, y, 96, 2);
            if meadow > 0.78 {
                return ids.tall_grass[2];
            }
            if meadow > 0.72 {
                return ids.tall_grass[(hash(seed, 13, x, y) % 2) as usize];
            }
            if detail < 0.015 {
                ids.tree
            } else if detail < 0.020 {
                ids.berry_bush
            } else if detail < 0.026 {
                // farming wave: wild carrots thread the open grass...
                ids.wild_carrot
            } else if detail < 0.028 {
                // ...and the odd volunteer pumpkin squats in it (seed stock for
                // infinite worlds; classic finite maps blob-spawn theirs)
                ids.pumpkin
            } else if detail < 0.060 {
                ids.flower
            } else if detail < 0.105 {
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
        // Solid rock, with ore veins hiding inside. The vein noise is sampled
        // anisotropically (one axis compressed 2x, axis alternating per depth) so
        // veins RUN — long thin seams you chase rather than round blobs — and the
        // shared surface richness field lowers the threshold, so ground under a
        // mineral-seep stain genuinely carries more ore.
        let (vx, vy) = if depth == -2 { (x, y * 2) } else { (x * 2, y) };
        let vein = fractal(seed, salt + 40, vx, vy, 12, 2);
        let vein_gate = 0.80 - 0.05 * richness_at(seed, x, y);
        if vein > vein_gate && detail < 0.6 {
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
    // cave-floor fungus: the same walk-through Mushroom tile as the forest floor
    // (it renders a dirt base underground)
    if detail < 0.012 {
        return ids.mushroom;
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

    // wild features (hot springs, abandoned mine shafts): surface pools/headframes
    // at depth 0, and each shaft's pre-carved gallery at depth -1 — stamped after
    // structures (features win the vanishingly rare overlap), before the gates
    super::features_gen::stamp_chunk(seed, depth, cx, cy, &mut chunk, tiles);

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
    fn ocean_has_skerries() {
        // sparse permanent rock stacks stand in open Ocean water (never in the
        // intertidal band, so they are not tide-dependent)
        let tiles = Tiles::new();
        let ids = Ids::get(&tiles);
        let seed = 20260707;
        let mut n = 0;
        'sweep: for cy in -400..400i32 {
            for cx in -400..400i32 {
                let (x, y) = (cx * 2, cy * 2);
                if !skerry_at(seed, x, y) {
                    continue;
                }
                let land = land_at(seed, x, y);
                if biome_at_blended(seed, x, y) != Biome::Ocean
                    || (tidal::BAND_LOW..tidal::BAND_HIGH).contains(&land)
                {
                    continue;
                }
                assert_eq!(
                    surface_tile(seed, x, y, &ids),
                    ids.rock,
                    "skerry cell at ({x},{y}) did not generate as rock"
                );
                n += 1;
                if n >= 3 {
                    break 'sweep;
                }
            }
        }
        assert!(n >= 3, "only {n} skerries in an 800x800-cell sweep");
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

    #[test]
    fn tundra_never_borders_desert_or_savanna() {
        // Climate coherence: the cold extreme and the hot/dry "sandy" biomes must
        // always be separated by a visibly wide temperate buffer — snow next to sand
        // reads as a worldgen bug (user report). The extreme gates threshold the
        // smooth single-octave climate field (`climate_at`), whose gradient bound
        // keeps Tundra ~80+ tiles from Desert and 30+ from Savanna by construction;
        // this scan guards the property against regressions. Uses `biome_at_blended`
        // (the render-facing lookup) so the ±4-tile domain-warp jitter is included.
        const STEP: i32 = 4;
        const HALF: i32 = 2048;
        const MIN_GAP: i32 = 12; // tiles; > 2x the blend jitter
        let n = (2 * HALF / STEP) as usize;
        let r = MIN_GAP / STEP; // lattice steps to cover MIN_GAP tiles

        for seed in [1i64, 424242, 20260707, -987654321] {
            // classify the lattice once, then check tundra cells' neighborhoods
            let mut grid = vec![0u8; n * n];
            for gy in 0..n {
                for gx in 0..n {
                    let (x, y) = (-HALF + gx as i32 * STEP, -HALF + gy as i32 * STEP);
                    grid[gx + gy * n] = match biome_at_blended(seed, x, y) {
                        Biome::Tundra => 1,
                        Biome::Desert | Biome::Badlands | Biome::Savanna => 2,
                        _ => 0,
                    };
                }
            }
            for gy in 0..n as i32 {
                for gx in 0..n as i32 {
                    if grid[gx as usize + gy as usize * n] != 1 {
                        continue;
                    }
                    for dy in -r..=r {
                        for dx in -r..=r {
                            let (nx, ny) = (gx + dx, gy + dy);
                            if nx < 0 || ny < 0 || nx >= n as i32 || ny >= n as i32 {
                                continue;
                            }
                            assert_ne!(
                                grid[nx as usize + ny as usize * n],
                                2,
                                "seed {seed}: Desert/Savanna within {MIN_GAP} tiles \
                                 of Tundra at tile ({}, {})",
                                -HALF + nx * STEP,
                                -HALF + ny * STEP,
                            );
                        }
                    }
                }
            }
        }
    }
}
