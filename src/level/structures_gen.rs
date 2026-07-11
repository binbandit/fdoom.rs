//! Deterministic surface structures for infinite worlds: ruins, cemeteries, standing
//! stones, abandoned camps, destroyed villages — plus the connective tissue between
//! them: worn trails, and boulder scatter in the open biomes.
//!
//! Placement follows the same hash-grid pattern as `infinite_gen::gate_in_cell`: each
//! structure type gets its own coarse cell grid, and each cell holds at most one
//! structure at a jittered, biome-gated position — a pure function of
//! `(world seed, structure kind, cell)`. Each kind also rolls a layout variant from
//! the placement hash ([`variant_of`]): ruins come as square rooms, L-shaped two-room
//! builds, or round towers; cemeteries are fenced, overgrown, or stone-walled; standing
//! stones form rings, straight avenues, or dolmen clusters; camps pitch a lean-to or go
//! cold (fire ring + bedroll); villages center on a round plaza or a crossroads.
//! Chunks stamp every structure whose footprint
//! could overlap them (rect query padded by [`MAX_RADIUS`]), so a structure straddling a
//! chunk border comes out identical from both sides.
//!
//! Three stamping passes run per chunk, all pure, in a fixed order so overlaps resolve
//! identically everywhere:
//!
//! 1. **Boulders** ([`boulder_at`]): sparse per-tile hash scatter of 1x1/2x2 rock
//!    outcrops in Plains/Savanna/Tundra. Breakable like any rock tile.
//! 2. **Trails** ([`trails_in_rect`], [`trail_writes`]): each trail-worthy structure
//!    (ruins/cemetery/camp) links to its nearest neighbor within [`TRAIL_RANGE`] tiles
//!    with a winding worn-dirt path — hash-jittered waypoint chains with occasional
//!    worn-away gaps and a torch stump where the trail meets the site. Trails only
//!    replace soft ground (grass/sand/snow/trees/...), never water or rock, so they
//!    fade out at fords and outcrops like real old routes.
//! 3. **Structures** ([`structure_writes`]): the blueprints proper, stamped last so
//!    their footprints always win. Villages come last in [`ALL_KINDS`] so a rare
//!    single-structure overlap resolves in the village's favor.
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

/// Largest half-extent of any structure footprint (a village spans up to 49x49).
pub const MAX_RADIUS: i32 = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureKind {
    Ruins,
    Cemetery,
    StandingStones,
    Camp,
    Village,
}

/// Fixed iteration order — stamping order must be identical from every chunk.
/// Villages stamp last so they win the (rare) overlap with a single structure.
pub const ALL_KINDS: [StructureKind; 5] = [
    StructureKind::Ruins,
    StructureKind::Cemetery,
    StructureKind::StandingStones,
    StructureKind::Camp,
    StructureKind::Village,
];

/// Half-extent of one kind's footprint: how far its tile writes can reach from the
/// placement origin.
pub fn kind_radius(kind: StructureKind) -> i32 {
    match kind {
        StructureKind::Village => 24,
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
        StructureKind::Village => matches!(b, Biome::Plains | Biome::Forest | Biome::Savanna),
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
struct StructIds {
    grass: u8,
    dirt: u8,
    sand: u8,
    snow: u8,
    mud: u8,
    tree: u8,
    water: u8,
    rock: u8,
    flower: u8,
    tall_grass: [u8; 3],
    stone_wall: u8,
    stone_floor: u8,
    window: u8,
    grave: u8,
    fence: u8,
    planks: u8,
    wool: u8,
    torch_dirt: u8,
    jack_o: u8,
    /// Flora-wave scatter tiles trails may wear through (species trees, bushes, reeds).
    soft_flora: [u8; 9],
}

impl StructIds {
    fn get(tiles: &Tiles) -> StructIds {
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
    fn trail_ground(&self, t: u8) -> bool {
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
fn raster_line(x0: i32, y0: i32, x1: i32, y1: i32, out: &mut Vec<(i32, i32)>) {
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

/// Eight compass directions scaled by 4, so building offsets stay pure integer math
/// (float trig could differ across platforms and break cross-machine determinism).
const VILLAGE_DIRS: [(i32, i32); 8] = [
    (4, 0),
    (3, 3),
    (0, 4),
    (-3, 3),
    (-4, 0),
    (-3, -3),
    (0, -4),
    (3, -3),
];

/// The four diagonal slots of [`VILLAGE_DIRS`] — the crossroads variant puts its
/// buildings in the quadrants between the two roads.
const QUADRANT_DIRS: [(i32, i32); 4] = [(3, 3), (-3, 3), (-3, -3), (3, -3)];

/// The buildings of a village as `(center x, center y, half width, half height)` —
/// 3-5 on hashed compass slots around the round plaza (variant 0), or 3-4 in the road
/// quadrants (crossroads, variant 1). Pure; shared by the blueprint and by
/// [`chest_positions`] so chests always land on a building's floor.
fn village_buildings(seed: i64, ox: i32, oy: i32, variant: u32) -> Vec<(i32, i32, i32, i32)> {
    let h = hash(seed, 0x56C4_0001, ox, oy);
    let (n, slots): (i32, &[(i32, i32)]) = if variant == 0 {
        (3 + (h % 3) as i32, &VILLAGE_DIRS) // 3..=5 buildings
    } else {
        (3 + (h % 2) as i32, &QUADRANT_DIRS) // 3..=4, one per quadrant
    };
    let len = slots.len() as i32;
    let rot = ((h >> 8) % slots.len() as u64) as i32;
    let mut out = Vec::new();
    for k in 0..n {
        let bh = hash(seed, 0x56C4_0002_u64.wrapping_add(k as u64), ox, oy);
        let slot = (rot + k * len / n).rem_euclid(len) as usize;
        let (dx4, dy4) = slots[slot];
        let dist = 12 + (bh % 4) as i32; // 12..=15 tiles from the plaza center
        let jx = ((bh >> 16) % 3) as i32 - 1;
        let jy = ((bh >> 24) % 3) as i32 - 1;
        let bx = ox + dx4 * dist / 4 + jx;
        let by = oy + dy4 * dist / 4 + jy;
        let hw = 2 + ((bh >> 32) % 2) as i32; // half-extents 2..=3 (5x5 .. 7x7)
        let hh = 2 + ((bh >> 40) % 2) as i32;
        out.push((bx, by, hw, hh));
    }
    out
}

/// Doorway offset of a village building (on its perimeter, facing the plaza) —
/// shared by the blueprint and [`lantern_positions`] so the lantern never sits in
/// the doorway path.
fn village_door_offset(ox: i32, oy: i32, bx: i32, by: i32, hw: i32, hh: i32) -> (i32, i32) {
    let (tx, ty) = (ox - bx, oy - by);
    if tx.abs() >= ty.abs() {
        (if tx > 0 { hw } else { -hw }, 0)
    } else {
        (0, if ty > 0 { hh } else { -hh })
    }
}

/// Where a village house keeps its lit lantern: the interior corner away from the
/// doorway. Off the door-to-center walking line, never the center tile (that is the
/// loot chest's spot), and deep enough inside that its light has to leave through
/// the windows and wall gaps — the occlusion showcase.
fn village_lantern_offset(ox: i32, oy: i32, bx: i32, by: i32, hw: i32, hh: i32) -> (i32, i32) {
    let (ddx, ddy) = village_door_offset(ox, oy, bx, by, hw, hh);
    if ddy == 0 {
        (-ddx.signum() * (hw - 1), hh - 1)
    } else {
        (hw - 1, -ddy.signum() * (hh - 1))
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
            // broken stone walls over a brick floor with rubble, in one of three
            // shapes: square room, L-shaped two-room, or a round tower footprint
            let h = hash(seed, 0xB1DE_0001, ox, oy);
            // interior floor mix shared by every shape
            let floor = |x: i32, y: i32| {
                if detail(0xB1DE_0003, x, y) < 0.06 {
                    ids.rock // rubble
                } else if detail(0xB1DE_0004, x, y) < 0.12 {
                    ids.dirt // floor worn through to earth
                } else {
                    ids.stone_floor
                }
            };
            let variant = variant_of(seed, p);
            match variant {
                // the classic: one square room with an always-open south doorway
                0 => {
                    let hw = 3 + (h % 3) as i32; // half-extents 3..=5 (7x7 .. 11x11)
                    let hh = 3 + ((h >> 16) % 3) as i32;
                    for dy in -hh..=hh {
                        for dx in -hw..=hw {
                            let (x, y) = (ox + dx, oy + dy);
                            let perimeter = dx.abs() == hw || dy.abs() == hh;
                            let doorway = dx == 0 && dy == hh;
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
                // L-shape: a wide hall (north) with a side wing running south, the
                // wall traced around the union so the inner corner reads as one build
                1 => {
                    let in_l = |dx: i32, dy: i32| {
                        ((-5..=5).contains(&dx) && (-5..=-1).contains(&dy))
                            || ((-5..=-1).contains(&dx) && (-3..=5).contains(&dy))
                    };
                    for dy in -5..=5i32 {
                        for dx in -5..=5i32 {
                            if !in_l(dx, dy) {
                                continue;
                            }
                            let (x, y) = (ox + dx, oy + dy);
                            let edge = !(in_l(dx - 1, dy)
                                && in_l(dx + 1, dy)
                                && in_l(dx, dy - 1)
                                && in_l(dx, dy + 1));
                            let doorway = dx == -3 && dy == 5; // south door of the wing
                            let crumbled = detail(0xB1DE_0002, x, y) < 0.30;
                            let t = if edge && !doorway && !crumbled {
                                ids.stone_wall
                            } else {
                                floor(x, y)
                            };
                            w.push((x, y, t));
                        }
                    }
                }
                // round tower footprint: a circular wall ring, south entrance gap
                _ => {
                    let r = 4 + (h % 2) as i32; // radius 4 or 5
                    for dy in -(r + 1)..=(r + 1) {
                        for dx in -(r + 1)..=(r + 1) {
                            let d2 = dx * dx + dy * dy;
                            if d2 > r * r + r {
                                continue;
                            }
                            let (x, y) = (ox + dx, oy + dy);
                            let on_ring = (d2 - r * r).abs() <= r;
                            let doorway = dx == 0 && dy > 0; // south entrance
                            let crumbled = detail(0xB1DE_0002, x, y) < 0.25;
                            let t = if on_ring && !doorway && !crumbled {
                                ids.stone_wall
                            } else {
                                floor(x, y)
                            };
                            w.push((x, y, t));
                        }
                    }
                }
            }
            // the chest tile is always sound floor, whatever the shape rolled
            let (cdx, cdy) = ruins_chest_offset(variant);
            w.push((ox + cdx, oy + cdy, ids.stone_floor));
        }
        StructureKind::Cemetery => {
            // dirt plot with graves spaced 2 apart; the edge comes in three states:
            // broken fence, no edge at all (overgrown), or a stone-wall perimeter
            let h = hash(seed, 0xCE4E_0001, ox, oy);
            let rx = 4 + (h % 3) as i32; // half-extents 4..=6 (9x9 .. 13x13)
            let ry = 4 + ((h >> 16) % 3) as i32;
            let variant = variant_of(seed, p);
            for dy in -ry..=ry {
                for dx in -rx..=rx {
                    let (x, y) = (ox + dx, oy + dy);
                    let perimeter = dx.abs() == rx || dy.abs() == ry;
                    let gate = dx == 0 && dy == ry;
                    let t = match variant {
                        // fenced plot, gaps where pickets rotted away
                        0 if perimeter && !gate && detail(0xCE4E_0002, x, y) < 0.60 => ids.fence,
                        // overgrown: no edge, tall-grass tufts reclaiming the plot
                        1 if detail(0xCE4E_0005, x, y) < 0.22 => {
                            ids.tall_grass[(hash(seed, 0xCE4E_0006, x, y) % 3) as usize]
                        }
                        // walled plot: a stone perimeter that mostly still stands
                        2 if perimeter && !gate && detail(0xCE4E_0002, x, y) < 0.80 => {
                            ids.stone_wall
                        }
                        _ => ids.dirt,
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
            // some cemeteries keep a lit Jack-O-Lantern by the gate — a warning, or a
            // welcome (off the grave lattice: graves never reach the |dx| = rx-1 ring)
            if unit(hash(seed, 0xCE4E_0004, ox, oy)) < 0.30 {
                w.push((ox - rx + 1, oy + ry - 1, ids.jack_o));
            }
        }
        StructureKind::StandingStones => {
            let h = hash(seed, 0x57ED_0001, ox, oy);
            match variant_of(seed, p) {
                // a ring of stones on cleared grass with a flower at the center
                0 => {
                    let r = 3 + (h % 2) as i32; // radius 3 or 4
                    for dy in -(r + 1)..=(r + 1) {
                        for dx in -(r + 1)..=(r + 1) {
                            let d2 = dx * dx + dy * dy;
                            if d2 > (r + 1) * (r + 1) {
                                continue;
                            }
                            let (x, y) = (ox + dx, oy + dy);
                            // ring band: |d2 - r²| <= r, with a few fallen stones
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
                // a processional avenue: 5-7 stones in a straight line (one of four
                // integer directions), each on a small cleared verge
                1 => {
                    const DIRS: [(i32, i32); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];
                    let n = 5 + (h % 3) as i32; // 5..=7 stones
                    let (sx, sy) = DIRS[((h >> 8) % 4) as usize];
                    // verge first, then the stones, so stones always win the overlap
                    for k in 0..n {
                        let off = 2 * k - (n - 1); // spacing 2, centered on the origin
                        for dy in -1..=1i32 {
                            for dx in -1..=1i32 {
                                w.push((ox + sx * off + dx, oy + sy * off + dy, ids.grass));
                            }
                        }
                    }
                    for k in 0..n {
                        let off = 2 * k - (n - 1);
                        let (x, y) = (ox + sx * off, oy + sy * off);
                        if detail(0x57ED_0003, x, y) < 0.88 {
                            w.push((x, y, ids.rock)); // a few have fallen
                        }
                    }
                }
                // dolmen cluster: a tight 2x2 capstone block on a small clearing,
                // fallen outliers around it, an offering flower beside it
                _ => {
                    for dy in -3..=3i32 {
                        for dx in -3..=3i32 {
                            if dx * dx + dy * dy > 11 {
                                continue;
                            }
                            w.push((ox + dx, oy + dy, ids.grass));
                        }
                    }
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        w.push((ox + dx, oy + dy, ids.rock));
                    }
                    for (i, (dx, dy)) in
                        [(-2, -1), (2, -2), (-1, 2), (3, 1)].into_iter().enumerate()
                    {
                        if unit(hash(seed, 0x57ED_0004_u64.wrapping_add(i as u64), ox, oy)) < 0.55 {
                            w.push((ox + dx, oy + dy, ids.rock));
                        }
                    }
                    w.push((ox - 1, oy - 1, ids.flower));
                }
            }
        }
        StructureKind::Camp => {
            // trampled clearing and a still-burning torch, with or without shelter
            for dy in -3..=3 {
                for dx in -3..=3 {
                    if dx * dx + dy * dy > 10 {
                        continue;
                    }
                    w.push((ox + dx, oy + dy, ids.dirt));
                }
            }
            if variant_of(seed, p) == 0 {
                // a plank lean-to beside the fire
                for (dx, dy) in [(-2, -1), (-1, -1), (-2, 0), (-1, 0)] {
                    w.push((ox + dx, oy + dy, ids.planks));
                }
            } else {
                // cold camp: a rock fire ring with one gap, and a wool bedroll strip
                let gap = (hash(seed, 0xC01D_0001, ox, oy) % 4) as usize;
                for (i, (dx, dy)) in [(1, 0), (0, 1), (-1, 0), (0, -1)].into_iter().enumerate() {
                    if i != gap {
                        w.push((ox + dx, oy + dy, ids.rock));
                    }
                }
                for (dx, dy) in [(-2, 1), (-1, 1)] {
                    w.push((ox + dx, oy + dy, ids.wool));
                }
            }
            // lean-to camps keep a still-burning torch; cold camps get a burnt-out
            // campfire *entity* instead (see `campfire_positions`), so the center
            // stays plain dirt for the ember ring to sit on
            if variant_of(seed, p) == 0 {
                w.push((ox, oy, ids.torch_dirt));
            }
        }
        StructureKind::Village => {
            // a razed hamlet around a rubble well: broken buildings ring a round
            // plaza (variant 0) or sit in the quadrants of two crossing worn roads
            // (variant 1); worn paths link the center to every doorway
            let variant = variant_of(seed, p);
            let ground = |x: i32, y: i32| {
                if detail(0x56C4_0006, x, y) < 0.15 {
                    ids.stone_floor // surviving paving stones
                } else {
                    ids.dirt
                }
            };
            if variant == 0 {
                // the open round plaza
                for dy in -5..=5i32 {
                    for dx in -5..=5i32 {
                        if dx * dx + dy * dy > 26 {
                            continue;
                        }
                        let (x, y) = (ox + dx, oy + dy);
                        w.push((x, y, ground(x, y)));
                    }
                }
            } else {
                // two worn roads crossing at the well, with worn-away stretches
                for d in -17..=17i32 {
                    let (hx, hy) = (ox + d, oy); // east-west arm, widened south
                    if detail(0x56C4_000B, hx, hy) >= 0.12 {
                        w.push((hx, hy, ground(hx, hy)));
                        if detail(0x56C4_000C, hx, hy) < 0.50 {
                            w.push((hx, hy + 1, ground(hx, hy + 1)));
                        }
                    }
                    let (vx, vy) = (ox, oy + d); // north-south arm, widened east
                    if detail(0x56C4_000D, vx, vy) >= 0.12 {
                        w.push((vx, vy, ground(vx, vy)));
                        if detail(0x56C4_000E, vx, vy) < 0.50 {
                            w.push((vx + 1, vy, ground(vx + 1, vy)));
                        }
                    }
                }
                // packed-earth apron around the well so the crossing reads as a yard
                for dy in -2..=2i32 {
                    for dx in -2..=2i32 {
                        if dx * dx + dy * dy > 6 {
                            continue;
                        }
                        let (x, y) = (ox + dx, oy + dy);
                        w.push((x, y, ground(x, y)));
                    }
                }
            }
            let buildings = village_buildings(seed, ox, oy, variant);
            // paths before buildings, so the shells stamp cleanly over the path ends
            for &(bx, by, _, _) in &buildings {
                let mut line = Vec::new();
                raster_line(ox, oy, bx, by, &mut line);
                for (x, y) in line {
                    if detail(0x56C4_0009, x, y) < 0.15 {
                        continue; // worn away
                    }
                    w.push((x, y, ids.dirt));
                }
            }
            for &(bx, by, hw, hh) in &buildings {
                // doorway on the wall facing the plaza
                let door = village_door_offset(ox, oy, bx, by, hw, hh);
                let lantern = village_lantern_offset(ox, oy, bx, by, hw, hh);
                for dy in -hh..=hh {
                    for dx in -hw..=hw {
                        let (x, y) = (bx + dx, by + dy);
                        let perimeter = dx.abs() == hw || dy.abs() == hh;
                        let corner = dx.abs() == hw && dy.abs() == hh;
                        let doorway = (dx, dy) == door;
                        let standing = detail(0x56C4_0003, x, y) >= 0.35;
                        let t = if perimeter && !doorway && standing {
                            // some standing wall runs keep a glazed pane — at night
                            // the house lantern glows through it (never a corner:
                            // wall runs stay solid where they turn)
                            if !corner && detail(0x56C4_000F, x, y) < 0.25 {
                                ids.window
                            } else {
                                ids.stone_wall
                            }
                        } else if (dx == 0 && dy == 0) || (dx, dy) == lantern {
                            // sound plank floor where the loot chest (center) and
                            // the house lantern stand — never rubble under either
                            ids.planks
                        } else if detail(0x56C4_0004, x, y) < 0.05 {
                            ids.rock // rubble
                        } else if detail(0x56C4_0005, x, y) < 0.18 {
                            ids.dirt // floor worn through to earth
                        } else {
                            ids.planks
                        };
                        w.push((x, y, t));
                    }
                }
            }
            // rarely, a Jack-O-Lantern still burns at the plaza edge of a razed
            // village — someone (or something) keeps lighting it (outside the 3x3
            // well footprint, inside the plaza circle, far from every building)
            if unit(hash(seed, 0x56C4_000A, ox, oy)) < 0.20 {
                w.push((ox + 3, oy + 2, ids.jack_o));
            }
            // the rubble well, last so it always crowns the plaza center
            for dy in -1..=1i32 {
                for dx in -1..=1i32 {
                    let (x, y) = (ox + dx, oy + dy);
                    let t = if dx == 0 && dy == 0 {
                        ids.water
                    } else if detail(0x56C4_0007, x, y) < 0.40 {
                        ids.rock // collapsed ring
                    } else {
                        ids.stone_wall
                    };
                    w.push((x, y, t));
                }
            }
        }
    }
    w
}

/* -------------------------------------- trails --------------------------------------- */

/// Two structures link up with a trail when one is the other's nearest trail-worthy
/// neighbor within this many tiles.
pub const TRAIL_RANGE: i32 = 200;

/// Maximum lateral wander of a trail from the straight line between its endpoints
/// (jitter amplitude caps at `TRAIL_RANGE * 0.22` but never above this, +rounding).
pub const TRAIL_JITTER: i32 = 16;

/// Structure kinds that anchor trails (villages keep their paths internal).
fn trail_endpoint(kind: StructureKind) -> bool {
    matches!(
        kind,
        StructureKind::Ruins | StructureKind::Cemetery | StructureKind::Camp
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
        // occasional gaps: whole worn-away stretches (coarse) plus lone missing tiles
        if unit(hash(
            seed,
            0x7261_494C_0003,
            x.div_euclid(5),
            y.div_euclid(5),
        )) < 0.07
        {
            continue;
        }
        if unit(hash(seed, 0x7261_494C_0004, x, y)) < 0.06 {
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

/// Boulder anchored at `(x, y)`: `Some(true)` for a 2x2 (covering `x..=x+1, y..=y+1`),
/// `Some(false)` for a single rock tile. Sparse hash scatter, only in open biomes
/// (Plains/Savanna/Tundra); stamped as plain `rock`, so breakable like any outcrop.
pub fn boulder_at(seed: i64, x: i32, y: i32) -> Option<bool> {
    let h = hash(seed, 0xB07D_E520_0009, x, y);
    if unit(h) > 0.0008 {
        return None;
    }
    if !matches!(
        biome_at(seed, x, y),
        Biome::Plains | Biome::Savanna | Biome::Tundra
    ) {
        return None;
    }
    Some(h & (1 << 40) != 0)
}

/* ----------------------------------- chunk stamping ---------------------------------- */

/// Stamp everything overlapping the chunk, in fixed pass order (boulders, then
/// trails, then structures — see the module docs). Called from
/// `infinite_gen::generate_chunk`; pure, surface only.
pub fn stamp_chunk(seed: i64, depth: i32, cx: i32, cy: i32, chunk: &mut Chunk, tiles: &Tiles) {
    if depth != 0 {
        return;
    }
    let ids = StructIds::get(tiles);
    let base_x = cx * CHUNK_SIZE;
    let base_y = cy * CHUNK_SIZE;

    // pass 1: boulders — pad by 1 so a 2x2 anchored just outside still stamps its share
    for y in (base_y - 1)..(base_y + CHUNK_SIZE) {
        for x in (base_x - 1)..(base_x + CHUNK_SIZE) {
            let Some(big) = boulder_at(seed, x, y) else {
                continue;
            };
            let ext = if big { 1 } else { 0 };
            for dy in 0..=ext {
                for dx in 0..=ext {
                    let (lx, ly) = (x + dx - base_x, y + dy - base_y);
                    if (0..CHUNK_SIZE).contains(&lx) && (0..CHUNK_SIZE).contains(&ly) {
                        chunk.tiles[(lx + ly * CHUNK_SIZE) as usize] = ids.rock;
                    }
                }
            }
        }
    }

    // pass 2: trails — only wear paths into soft ground (never water/rock/boulders)
    for (a, b) in trails_in_rect(
        seed,
        base_x,
        base_y,
        base_x + CHUNK_SIZE - 1,
        base_y + CHUNK_SIZE - 1,
    ) {
        for (x, y, t) in trail_writes(seed, a, b, tiles) {
            let (lx, ly) = (x - base_x, y - base_y);
            if (0..CHUNK_SIZE).contains(&lx) && (0..CHUNK_SIZE).contains(&ly) {
                let i = (lx + ly * CHUNK_SIZE) as usize;
                if ids.trail_ground(chunk.tiles[i]) {
                    chunk.tiles[i] = t;
                }
            }
        }
    }

    // pass 3: structures — stamped last so their footprints always win
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

/// Where a ruins chest sits relative to the origin — interior floor in every shape
/// (the L-shape's origin lies outside the L, in the notch, so its chest moves into
/// the room overlap).
fn ruins_chest_offset(variant: u32) -> (i32, i32) {
    if variant == 1 { (-3, -3) } else { (0, 0) }
}

/// The global tiles the structure's loot chests sit on (empty for chestless kinds).
/// Pure, so exactly one chunk (the one containing each tile) owns each spawn.
pub fn chest_positions(seed: i64, p: Placement) -> Vec<(i32, i32)> {
    match p.kind {
        // ~60% of ruins hide a chest on the room floor
        StructureKind::Ruins => {
            if unit(hash(seed, 0xB1DE_0005, p.x, p.y)) < 0.60 {
                let (dx, dy) = ruins_chest_offset(variant_of(seed, p));
                vec![(p.x + dx, p.y + dy)]
            } else {
                Vec::new()
            }
        }
        // every camp has one, beside the fire
        StructureKind::Camp => vec![(p.x + 2, p.y)],
        // villages hold 1-2, centered in the first buildings (always plank floor)
        StructureKind::Village => {
            let b = village_buildings(seed, p.x, p.y, variant_of(seed, p));
            let mut out = vec![(b[0].0, b[0].1)];
            if unit(hash(seed, 0x56C4_0008, p.x, p.y)) < 0.5 {
                out.push((b[1].0, b[1].1));
            }
            out
        }
        _ => Vec::new(),
    }
}

/// Where a placement spawns a burnt-out (ember) campfire entity: the fire-ring
/// center of every *cold-camp* variant (lean-to camps keep their torch instead).
/// Pure, like [`chest_positions`], so exactly one chunk owns the spawn.
pub fn campfire_positions(seed: i64, p: Placement) -> Vec<(i32, i32)> {
    match p.kind {
        StructureKind::Camp if variant_of(seed, p) != 0 => vec![(p.x, p.y)],
        _ => Vec::new(),
    }
}

/// Where a placement spawns lit lantern entities: one per village house, in the
/// interior corner away from the doorway (see [`village_lantern_offset`] — same
/// lore as the plaza Jack-O-Lantern: someone, or something, keeps them burning).
/// At night the glow leaves through the window panes and wall gaps, which is what
/// makes village houses read as destinations instead of dead shells (playtest #8).
/// Pure, like [`chest_positions`], so exactly one chunk owns each spawn.
pub fn lantern_positions(seed: i64, p: Placement) -> Vec<(i32, i32)> {
    match p.kind {
        StructureKind::Village => village_buildings(seed, p.x, p.y, variant_of(seed, p))
            .into_iter()
            .map(|(bx, by, hw, hh)| {
                let (dx, dy) = village_lantern_offset(p.x, p.y, bx, by, hw, hh);
                (bx + dx, by + dy)
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Spawn structure entities (loot chests, cold-camp ember campfires, village house
/// lanterns) for a chunk that was just generated fresh. Marks the chunk dirty so it
/// persists to disk and never generates fresh again — that's what prevents duplicate
/// spawns. Chunks explored before a structure feature shipped are NOT retrofitted:
/// they were saved to disk and never re-run through this path.
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
    // touch the tile's data byte (same value) purely to set the chunk's dirty flag
    let touch = |g: &mut Game, tx: i32, ty: i32| {
        let data = g.level(lvl).get_data(tx, ty);
        g.level_mut(lvl).set_data(tx, ty, data);
    };
    for p in placements {
        for (tx, ty) in chest_positions(seed, p) {
            if chunk_coord(tx) != cx || chunk_coord(ty) != cy {
                continue; // another chunk owns this chest
            }
            let mut chest = crate::entity::furniture::chest::new();
            fill_structure_chest(g, &mut chest, p.kind, hash(seed, 0x100D_0006, tx, ty));
            g.level_mut(lvl).add_at(chest, tx, ty, true, lvl);
            touch(g, tx, ty);
        }
        for (tx, ty) in campfire_positions(seed, p) {
            if chunk_coord(tx) != cx || chunk_coord(ty) != cy {
                continue; // another chunk owns this campfire
            }
            let ember = crate::entity::furniture::campfire::new_ember();
            g.level_mut(lvl).add_at(ember, tx, ty, true, lvl);
            touch(g, tx, ty);
        }
        for (tx, ty) in lantern_positions(seed, p) {
            if chunk_coord(tx) != cx || chunk_coord(ty) != cy {
                continue; // another chunk owns this lantern
            }
            let lantern = crate::entity::furniture::lantern::new(
                crate::entity::furniture::lantern::LanternType::Norm,
            );
            g.level_mut(lvl).add_at(lantern, tx, ty, true, lvl);
            touch(g, tx, ty);
        }
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
        // a sacked village is the richest find of the four
        StructureKind::Village => &[
            (2, "Torch", 3),
            (2, "Stone", 6),
            (2, "Bread", 2),
            (3, "Wood", 6),
            (3, "Cord", 2),
            (4, "Apple", 2),
            (5, "Coal", 4),
            (8, "Iron", 2),
            (12, "Gold", 1),
        ],
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
