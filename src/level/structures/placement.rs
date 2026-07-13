//! Deterministic hash-grid placement and trail machinery for surface structures.

use super::super::infinite_gen::{Biome, RiverZone, biome_at, hash, river_zone_at, unit};
use super::super::tile::Tiles;

/// Largest half-extent of any structure footprint (a village spans up to 49x49).
pub const MAX_RADIUS: i32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureKind {
    Ruins,
    Cemetery,
    StandingStones,
    Camp,
    Hamlet,
    Village,
}

/// Fixed iteration order — stamping order must be identical from every chunk.
/// The towns (hamlets, then villages) stamp last so they win the (rare) overlap
/// with a single structure; villages win even over a hamlet.
pub const ALL_KINDS: [StructureKind; 6] = [
    StructureKind::Ruins,
    StructureKind::Cemetery,
    StructureKind::StandingStones,
    StructureKind::Camp,
    StructureKind::Hamlet,
    StructureKind::Village,
];

/// Half-extent of one kind's footprint: how far its tile writes can reach from the
/// placement origin.
pub fn kind_radius(kind: StructureKind) -> i32 {
    match kind {
        StructureKind::Village => 24,
        // the straggle variant strings houses ~14 tiles out along its lane
        StructureKind::Hamlet => 18,
        // the avenue variant runs 7 stones out along an axis, plus its cleared verge
        StructureKind::StandingStones => 7,
        _ => 6,
    }
}

/// How many deterministic layout variants each kind has (see [`variant_of`]).
pub fn variant_count(kind: StructureKind) -> u32 {
    match kind {
        // square room / L-shaped two-room / round tower
        StructureKind::Ruins => 3,
        // fenced / unfenced overgrown / stone-walled
        StructureKind::Cemetery => 3,
        // ring / straight avenue / dolmen cluster
        StructureKind::StandingStones => 3,
        // lean-to camp / cold camp
        StructureKind::Camp => 2,
        // crossroads / ring around a green / straggle along a lane
        StructureKind::Hamlet => 3,
        // round plaza / crossroads
        StructureKind::Village => 2,
    }
}

/// The layout variant of a placement — a pure function of the placement hash, so every
/// chunk stamping a piece of the structure agrees on the shape (same guarantee as the
/// blueprint itself).
pub fn variant_of(seed: i64, p: Placement) -> u32 {
    let (_, salt, _) = spec(p.kind);
    (hash(seed, salt ^ 0x0A11_7E4A_11A5, p.x, p.y) % u64::from(variant_count(p.kind))) as u32
}

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
        // density wave: ~+55% structures per unit area overall, biased toward the
        // small sites (camps/stones/ruins). Villages stay at their old rarity —
        // they're set pieces, not scenery. Raising odds (not shrinking grids) mostly
        // adds new sites rather than reshuffling the old ones.
        StructureKind::Ruins => (224, 0x5255_494E_5321_0001, 0.70),
        StructureKind::Cemetery => (288, 0x4752_4156_4553_0002, 0.60),
        StructureKind::StandingStones => (320, 0x53_544F_4E45_0003, 0.62),
        StructureKind::Camp => (256, 0x43_414D_5046_0004, 0.80),
        // towns wave: hamlets are the common find between rare set-piece villages —
        // a modest density bump carried entirely by the new small footprint
        StructureKind::Hamlet => (320, 0x484D_4C45_5421_0006, 0.55),
        StructureKind::Village => (512, 0x56_494C_4C41_0005, 0.40),
    }
}

/// Which biomes a structure may spawn in (never ocean/beach/mountains).
fn biome_ok(kind: StructureKind, b: Biome) -> bool {
    match kind {
        StructureKind::Ruins => matches!(b, Biome::Plains | Biome::Forest | Biome::Savanna),
        // deserts bury their dead too (user request) — sun-bleached plots among the dunes
        StructureKind::Cemetery => {
            matches!(
                b,
                Biome::Plains | Biome::Forest | Biome::Marsh | Biome::Desert
            )
        }
        StructureKind::StandingStones => matches!(b, Biome::Plains | Biome::Savanna),
        StructureKind::Camp => matches!(b, Biome::Forest | Biome::Tundra | Biome::Desert),
        StructureKind::Hamlet | StructureKind::Village => {
            matches!(b, Biome::Plains | Biome::Forest | Biome::Savanna)
        }
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
    let margin = kind_radius(kind) + 1;
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
pub(crate) struct StructIds {
    pub(crate) grass: u8,
    pub(crate) dirt: u8,
    pub(crate) sand: u8,
    pub(crate) snow: u8,
    pub(crate) mud: u8,
    pub(crate) tree: u8,
    pub(crate) water: u8,
    pub(crate) rock: u8,
    pub(crate) flower: u8,
    pub(crate) tall_grass: [u8; 3],
    pub(crate) stone_wall: u8,
    pub(crate) stone_floor: u8,
    pub(crate) window: u8,
    pub(crate) grave: u8,
    pub(crate) fence: u8,
    pub(crate) planks: u8,
    pub(crate) wool: u8,
    pub(crate) torch_dirt: u8,
    pub(crate) jack_o: u8,
    /// Settled-town garden plots (towns wave); crop rows are the farming wave's
    /// gone-to-seed village fields.
    pub(crate) farmland: u8,
    pub(crate) berry_bush: u8,
    pub(crate) corn_crop: u8,
    pub(crate) carrot_crop: u8,
    /// Flora-wave scatter tiles trails may wear through (species trees, bushes, reeds).
    pub(crate) soft_flora: [u8; 9],
}

impl StructIds {
    pub(crate) fn get(tiles: &Tiles) -> StructIds {
        StructIds {
            grass: tiles.get("grass").id,
            dirt: tiles.get("dirt").id,
            sand: tiles.get("sand").id,
            snow: tiles.get("snow").id,
            mud: tiles.get("Mud").id,
            tree: tiles.get("tree").id,
            water: tiles.get("water").id,
            rock: tiles.get("rock").id,
            flower: tiles.get("flower").id,
            tall_grass: [
                tiles.get("small grass").id,
                tiles.get("medium grass").id,
                tiles.get("tall grass").id,
            ],
            stone_wall: tiles.get("Stone Wall").id,
            stone_floor: tiles.get("Stone Bricks").id,
            window: tiles.get("Window").id,
            grave: tiles.get("Grave stone").id,
            fence: tiles.get("Fence").id,
            planks: tiles.get("Wood Planks").id,
            wool: tiles.get("Wool").id,
            torch_dirt: tiles.get("torch dirt").id,
            jack_o: tiles.get("Jack-O-Lantern").id,
            farmland: tiles.get("Farmland").id,
            berry_bush: tiles.get("Berry Bush").id,
            corn_crop: tiles.get("Corn Crop").id,
            carrot_crop: tiles.get("Carrot Crop").id,
            soft_flora: [
                tiles.get("Pine Tree").id,
                tiles.get("Dead Tree").id,
                tiles.get("Willow").id,
                tiles.get("Palm Tree").id,
                tiles.get("Flat-Crown Tree").id,
                tiles.get("Berry Bush").id,
                tiles.get("Mushroom").id,
                tiles.get("Reeds").id,
                tiles.get("Dry Bush").id,
            ],
        }
    }

    /// Soft ground the trail pass may wear a path into. Deliberately excludes water,
    /// rock, and every structure tile, so trails ford ponds as gaps and never chew
    /// into a stamped boulder or building.
    pub(crate) fn trail_ground(&self, t: u8) -> bool {
        t == self.grass
            || t == self.dirt
            || t == self.sand
            || t == self.snow
            || t == self.mud
            || t == self.tree
            || t == self.flower
            || self.tall_grass.contains(&t)
            || self.soft_flora.contains(&t)
    }
}

/// Integer Bresenham line, inclusive of both endpoints, appended to `out`
/// (skipping a duplicated joint when chaining segments).
pub(crate) fn raster_line(x0: i32, y0: i32, x1: i32, y1: i32, out: &mut Vec<(i32, i32)>) {
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let (mut x, mut y) = (x0, y0);
    loop {
        if out.last() != Some(&(x, y)) {
            out.push((x, y));
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// Two structures link up with a trail when one is the other's nearest trail-worthy
/// neighbor within this many tiles.
pub const TRAIL_RANGE: i32 = 200;

/// Maximum lateral wander of a trail from the straight line between its endpoints
/// (jitter amplitude caps at `TRAIL_RANGE * 0.22` but never above this, +rounding).
pub const TRAIL_JITTER: i32 = 16;

/// Structure kinds that anchor trails (villages keep their paths internal; hamlets
/// join the trail net — the straggle variant literally lives along one).
fn trail_endpoint(kind: StructureKind) -> bool {
    matches!(
        kind,
        StructureKind::Ruins
            | StructureKind::Cemetery
            | StructureKind::Camp
            | StructureKind::Hamlet
    )
}

/// Every trail whose geometry could touch `[x0, x1] x [y0, y1]`, as canonically ordered
/// endpoint pairs (sorted, deduped). Pure: each trail-worthy structure connects to its
/// nearest trail-worthy neighbor within [`TRAIL_RANGE`]; the candidate search is padded
/// far enough that every chunk derives the identical pair set for the trails crossing
/// it, even when both endpoints lie in other chunks.
pub fn trails_in_rect(
    seed: i64,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
) -> Vec<(Placement, Placement)> {
    // an edge stays within TRAIL_RANGE + TRAIL_JITTER of either endpoint, so only
    // endpoints inside `pad_p` matter — and their partners within another TRAIL_RANGE
    let pad_p = TRAIL_RANGE + TRAIL_JITTER;
    let pad_q = pad_p + TRAIL_RANGE;
    let candidates: Vec<Placement> =
        placements_in_rect(seed, x0 - pad_q, y0 - pad_q, x1 + pad_q, y1 + pad_q)
            .into_iter()
            .filter(|p| trail_endpoint(p.kind))
            .collect();
    let range2 = (TRAIL_RANGE as i64) * (TRAIL_RANGE as i64);
    let mut pairs = Vec::new();
    for p in &candidates {
        if p.x < x0 - pad_p || p.x > x1 + pad_p || p.y < y0 - pad_p || p.y > y1 + pad_p {
            continue;
        }
        let nearest = candidates
            .iter()
            .filter(|q| (q.x, q.y, q.kind) != (p.x, p.y, p.kind))
            .map(|q| {
                let (dx, dy) = ((p.x - q.x) as i64, (p.y - q.y) as i64);
                (dx * dx + dy * dy, q)
            })
            .filter(|&(d2, _)| d2 <= range2)
            .min_by_key(|&(d2, q)| (d2, q.x, q.y));
        if let Some((_, q)) = nearest {
            let (a, b) = if (q.x, q.y) < (p.x, p.y) {
                (*q, *p)
            } else {
                (*p, *q)
            };
            pairs.push((a, b));
        }
    }
    pairs.sort_by_key(|&(a, b)| (a.x, a.y, b.x, b.y));
    pairs.dedup();
    pairs
}

/// The tile writes of one trail: mostly worn dirt 1-2 wide, occasional worn-away gaps,
/// and a chance of a torch stump where the trail meets each site. Pure function of
/// `(seed, endpoints)` — every chunk computes the identical polyline and clips it.
/// The curve avoids transcendental functions (only +,*,/,sqrt — IEEE-exact) so the
/// geometry is bit-identical on every platform.
pub fn trail_writes(seed: i64, a: Placement, b: Placement, tiles: &Tiles) -> Vec<(i32, i32, u8)> {
    let ids = StructIds::get(tiles);
    let (ax, ay) = (a.x as f64, a.y as f64);
    let (dx, dy) = (b.x as f64 - ax, b.y as f64 - ay);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 2.0 {
        return Vec::new();
    }
    // per-pair key drives the wander so parallel trails don't correlate
    let k = hash(seed, 0x7261_494C_0001, a.x, a.y) ^ hash(seed, 0x7261_494C_0002, b.x, b.y);
    let amp = (len * 0.22).clamp(2.0, (TRAIL_JITTER - 3) as f64);
    // smooth 1-D jitter: hashed control values every ~24 tiles, smoothstep-blended,
    // scaled by a 4t(1-t) envelope so both ends stay anchored on their structures
    let n_ctrl = ((len / 24.0).ceil() as i32).max(1);
    let ctrl = |j: i32| unit(hash(seed, k ^ 0x0FF5_E750, j, 0)) - 0.5;
    let offset = |t: f64| {
        let s = t * n_ctrl as f64;
        let j = s.floor();
        let f = s - j;
        let sm = f * f * (3.0 - 2.0 * f);
        let v = ctrl(j as i32) * (1.0 - sm) + ctrl(j as i32 + 1) * sm;
        4.0 * t * (1.0 - t) * amp * 2.0 * v
    };
    // waypoints every ~5 tiles along the straight line, displaced perpendicular
    let steps = ((len / 5.0).ceil() as i32).max(2);
    let (px, py) = (-dy / len, dx / len);
    let mut path: Vec<(i32, i32)> = Vec::new();
    let mut prev: Option<(i32, i32)> = None;
    for i in 0..=steps {
        let t = f64::from(i) / f64::from(steps);
        let off = offset(t);
        let wx = (ax + dx * t + px * off).round() as i32;
        let wy = (ay + dy * t + py * off).round() as i32;
        if let Some((lx, ly)) = prev {
            raster_line(lx, ly, wx, wy, &mut path);
        }
        prev = Some((wx, wy));
    }
    let widen_vertical = dx.abs() >= dy.abs();
    let mut w = Vec::new();
    for &(x, y) in &path {
        // over the river channel the trail becomes a plank footbridge (stamped by
        // `stamp_chunk`), and a bridge deck never wears gaps — it must span bank
        // to bank or the crossing reads broken
        let bridging = matches!(river_zone_at(seed, x, y), Some(RiverZone::Channel));
        // occasional gaps: whole worn-away stretches (coarse) plus lone missing tiles
        if !bridging
            && unit(hash(
                seed,
                0x7261_494C_0003,
                x.div_euclid(5),
                y.div_euclid(5),
            )) < 0.07
        {
            continue;
        }
        if !bridging && unit(hash(seed, 0x7261_494C_0004, x, y)) < 0.06 {
            continue;
        }
        w.push((x, y, ids.dirt));
        // widen to 2 tiles in stretches
        if unit(hash(seed, 0x7261_494C_0005, x, y)) < 0.40 {
            let (wx, wy) = if widen_vertical {
                (x, y + 1)
            } else {
                (x + 1, y)
            };
            w.push((wx, wy, ids.dirt));
        }
    }
    // a torch stump where the trail meets each site (its junction with the route)
    if path.len() >= 20 {
        for &i in &[6, path.len() - 7] {
            let (x, y) = path[i];
            if unit(hash(seed, 0x7261_494C_0006, x, y)) < 0.5 {
                w.push((x, y, ids.torch_dirt));
            }
        }
    }
    w
}

/* ------------------------------------- boulders -------------------------------------- */
