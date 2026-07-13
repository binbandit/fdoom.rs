//! Deterministic hamlet and village layouts, decay, and house stamping.

use super::super::infinite_gen::{hash, unit};
use super::placement::{Placement, StructIds, StructureKind, raster_line, variant_of};

/// How far gone a town is — a third generation axis for the two town kinds, pure
/// like the layout variant. OVERGROWN towns are the oldest (walls mostly down,
/// floors reclaimed by grass, lanterns burnt out, but the untouched holds carry
/// time-capsule loot); WEATHERED is the classic razed look; SETTLED reads freshly
/// kept — sound walls, tended plots, every lamp still burning — just nobody home.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TownAge {
    Overgrown,
    Weathered,
    Settled,
}

/// The age of a town placement (only meaningful for `Hamlet`/`Village`). Pure, so
/// every chunk stamping a piece of the town agrees on its state of decay.
pub fn town_age(seed: i64, p: Placement) -> TownAge {
    match hash(seed, 0xA6ED_70B1_0007, p.x, p.y) % 3 {
        0 => TownAge::Overgrown,
        1 => TownAge::Weathered,
        _ => TownAge::Settled,
    }
}

/// The per-tile decay dials one [`TownAge`] rolls with. `Weathered` is pinned to the
/// pre-age-axis constants, so classic villages generate byte-identically.
pub(crate) struct AgeParams {
    /// A perimeter wall tile crumbles when its detail roll lands under this.
    pub(crate) crumble: f64,
    /// Odds a standing (non-corner) wall run keeps a glazed pane.
    pub(crate) window: f64,
    /// Interior rubble / floor-worn-through-to-earth odds.
    pub(crate) rubble: f64,
    pub(crate) worn: f64,
    /// Odds an interior floor tile is reclaimed by flora (Overgrown only).
    pub(crate) overgrow: f64,
    /// Odds a plaza/road ground tile keeps its paving stones.
    pub(crate) paving: f64,
    /// Odds a stretch of door-path is worn away entirely.
    pub(crate) path_gap: f64,
}

pub(crate) fn age_params(age: TownAge) -> AgeParams {
    match age {
        TownAge::Overgrown => AgeParams {
            crumble: 0.62,
            window: 0.10,
            rubble: 0.08,
            worn: 0.30,
            overgrow: 0.26,
            paving: 0.04,
            path_gap: 0.42,
        },
        TownAge::Weathered => AgeParams {
            crumble: 0.35,
            window: 0.25,
            rubble: 0.05,
            worn: 0.18,
            overgrow: 0.0,
            paving: 0.15,
            path_gap: 0.15,
        },
        TownAge::Settled => AgeParams {
            crumble: 0.08,
            window: 0.30,
            rubble: 0.01,
            worn: 0.05,
            overgrow: 0.0,
            paving: 0.30,
            path_gap: 0.04,
        },
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
pub(crate) const QUADRANT_DIRS: [(i32, i32); 4] = [(3, 3), (-3, 3), (-3, -3), (3, -3)];

/// The buildings of a village as `(center x, center y, half width, half height)` —
/// 3-5 on hashed compass slots around the round plaza (variant 0), or 3-4 in the road
/// quadrants (crossroads, variant 1). Pure; shared by the blueprint and by
/// [`chest_positions`] so chests always land on a building's floor.
pub(crate) fn village_buildings(
    seed: i64,
    ox: i32,
    oy: i32,
    variant: u32,
) -> Vec<(i32, i32, i32, i32)> {
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
pub(crate) fn village_door_offset(
    ox: i32,
    oy: i32,
    bx: i32,
    by: i32,
    hw: i32,
    hh: i32,
) -> (i32, i32) {
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
pub(crate) fn village_lantern_offset(
    ox: i32,
    oy: i32,
    bx: i32,
    by: i32,
    hw: i32,
    hh: i32,
) -> (i32, i32) {
    let (ddx, ddy) = village_door_offset(ox, oy, bx, by, hw, hh);
    if ddy == 0 {
        (-ddx.signum() * (hw - 1), hh - 1)
    } else {
        (hw - 1, -ddy.signum() * (hh - 1))
    }
}

/// Where a house keeps its scavenge containers, relative to the building center:
/// the cupboard in the interior corner diagonally opposite the lantern (never the
/// chest's center tile, never the door line), and a rain barrel *outside*, flanking
/// the doorway against the wall — one step out and one step aside, so it never
/// blocks the walk-in tile.
pub(crate) fn house_container_offsets(
    ox: i32,
    oy: i32,
    bx: i32,
    by: i32,
    hw: i32,
    hh: i32,
) -> ((i32, i32), (i32, i32)) {
    let (lx, ly) = village_lantern_offset(ox, oy, bx, by, hw, hh);
    let cupboard = (-lx, -ly);
    let (ddx, ddy) = village_door_offset(ox, oy, bx, by, hw, hh);
    let barrel = if ddy == 0 {
        (ddx + ddx.signum(), 1)
    } else {
        (1, ddy + ddy.signum())
    };
    (cupboard, barrel)
}

/// The four integer lane directions a straggle hamlet can string out along.
pub(crate) const LANE_DIRS: [(i32, i32); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];

/// The houses of a hamlet as `(center x, center y, half width, half height)`.
/// Three layouts (crossroads / ring green / straggle) at two sizes (a coin-flip of
/// the placement hash picks compact or sprawling). Pure; shared by the blueprint,
/// [`lantern_positions`] and [`container_positions`], like [`village_buildings`].
pub(crate) fn hamlet_buildings(
    seed: i64,
    ox: i32,
    oy: i32,
    variant: u32,
) -> Vec<(i32, i32, i32, i32)> {
    let h = hash(seed, 0x484D_0001, ox, oy);
    let big = h & (1 << 50) != 0; // the two size classes
    let mut out = Vec::new();
    let mut house = |k: i32, bx: i32, by: i32| {
        let bh = hash(seed, 0x484D_0002_u64.wrapping_add(k as u64), ox, oy);
        let hw = 2 + ((bh >> 32) % 2) as i32; // half-extents 2..=3 (5x5 .. 7x7)
        let hh = 2 + ((bh >> 40) % 2) as i32;
        let jx = ((bh >> 16) % 3) as i32 - 1;
        let jy = ((bh >> 24) % 3) as i32 - 1;
        out.push((bx + jx, by + jy, hw, hh));
    };
    match variant {
        // crossroads: one house per road quadrant, close in
        0 => {
            let n = if big { 4 } else { 2 };
            let rot = ((h >> 8) % 4) as i32;
            for k in 0..n {
                let (qx, qy) = QUADRANT_DIRS[(rot + k).rem_euclid(4) as usize];
                let dist = 7 + (hash(seed, 0x484D_0003, ox + k, oy) % 2) as i32;
                house(k, ox + qx * dist / 4, oy + qy * dist / 4);
            }
        }
        // ring around a green: compass slots at an even spread
        1 => {
            let n = if big { 5 } else { 3 };
            let len = VILLAGE_DIRS.len() as i32;
            let rot = ((h >> 8) % 8) as i32;
            for k in 0..n {
                let slot = (rot + k * len / n).rem_euclid(len) as usize;
                let (dx4, dy4) = VILLAGE_DIRS[slot];
                let dist = 9 + (hash(seed, 0x484D_0003, ox + k, oy) % 2) as i32;
                house(k, ox + dx4 * dist / 4, oy + dy4 * dist / 4);
            }
        }
        // straggle: houses strung along a lane, alternating sides
        _ => {
            let n = if big { 4 } else { 2 };
            let (sx, sy) = LANE_DIRS[((h >> 8) % 4) as usize];
            let (px, py) = (-sy, sx); // lane perpendicular
            for k in 0..n {
                let off = k * 7 - 7 * (n - 1) / 2; // spacing 7, centered on the origin
                let side = if k % 2 == 0 { 4 } else { -4 };
                house(k, ox + sx * off + px * side, oy + sy * off + py * side);
            }
        }
    }
    out
}

/// The houses of either town kind (dispatch shared by the blueprint and the entity
/// position functions).
pub(crate) fn town_buildings(seed: i64, p: Placement) -> Vec<(i32, i32, i32, i32)> {
    match p.kind {
        StructureKind::Village => village_buildings(seed, p.x, p.y, variant_of(seed, p)),
        StructureKind::Hamlet => hamlet_buildings(seed, p.x, p.y, variant_of(seed, p)),
        _ => Vec::new(),
    }
}

/// Stamp one town house shell: perimeter walls (age-dependent standing odds, some
/// runs keeping a glazed pane), a doorway facing the town center, and an interior
/// floor that decays with age — sound planks when Settled, worn and rubbly when
/// Weathered, and reclaimed by grass and tufts when Overgrown. `keep` lists interior
/// offsets guaranteed sound plank floor (loot chest, lantern, cupboard spots).
#[allow(clippy::too_many_arguments)]
pub(crate) fn stamp_house(
    w: &mut Vec<(i32, i32, u8)>,
    seed: i64,
    ids: &StructIds,
    ap: &AgeParams,
    (ox, oy): (i32, i32),
    (bx, by, hw, hh): (i32, i32, i32, i32),
    keep: &[(i32, i32)],
) {
    let detail = |salt: u64, x: i32, y: i32| unit(hash(seed, salt, x, y));
    let door = village_door_offset(ox, oy, bx, by, hw, hh);
    for dy in -hh..=hh {
        for dx in -hw..=hw {
            let (x, y) = (bx + dx, by + dy);
            let perimeter = dx.abs() == hw || dy.abs() == hh;
            let corner = dx.abs() == hw && dy.abs() == hh;
            let doorway = (dx, dy) == door;
            let standing = detail(0x56C4_0003, x, y) >= ap.crumble;
            let t = if perimeter && !doorway && standing {
                // some standing wall runs keep a glazed pane — at night the house
                // lantern glows through it (never a corner: wall runs stay solid
                // where they turn)
                if !corner && detail(0x56C4_000F, x, y) < ap.window {
                    ids.window
                } else {
                    ids.stone_wall
                }
            } else if keep.contains(&(dx, dy)) {
                // sound plank floor under the loot chest, the house lantern and
                // any scavenge container — never rubble under the furniture
                ids.planks
            } else if !perimeter && detail(0x6F76_0001, x, y) < ap.overgrow {
                // Overgrown: the floor lost to grass pushing through the boards
                if detail(0x6F76_0002, x, y) < 0.30 {
                    ids.tall_grass[(hash(seed, 0x6F76_0002, x, y) % 3) as usize]
                } else {
                    ids.grass
                }
            } else if detail(0x56C4_0004, x, y) < ap.rubble {
                ids.rock // rubble
            } else if detail(0x56C4_0005, x, y) < ap.worn {
                // floor worn through — bare earth, or turf once truly Overgrown
                if ap.overgrow > 0.0 {
                    ids.grass
                } else {
                    ids.dirt
                }
            } else {
                ids.planks
            };
            w.push((x, y, t));
        }
    }
}

/// Stamp a Settled house's kitchen garden: a small fenced plot of tended farmland
/// off the wall opposite the doorway, a berry bush at the gap. The freshest age
/// marker there is — Weathered and Overgrown towns lost theirs long ago.
pub(crate) fn stamp_garden(
    w: &mut Vec<(i32, i32, u8)>,
    seed: i64,
    ids: &StructIds,
    (ox, oy): (i32, i32),
    (bx, by, hw, hh): (i32, i32, i32, i32),
) {
    let detail = |salt: u64, x: i32, y: i32| unit(hash(seed, salt, x, y));
    let (ddx, ddy) = village_door_offset(ox, oy, bx, by, hw, hh);
    // plot center: 3 tiles out from the back wall (the side away from the door)
    let (gx, gy) = if ddy == 0 {
        (bx - ddx.signum() * (hw + 3), by)
    } else {
        (bx, by - ddy.signum() * (hh + 3))
    };
    for dy in -1..=1i32 {
        for dx in -2..=2i32 {
            let (x, y) = (gx + dx, gy + dy);
            let edge = dx.abs() == 2 || dy.abs() == 1;
            let t = if edge {
                // a picket ring that mostly still stands, one bush at the SE gap
                if (dx, dy) == (2, 1) {
                    ids.berry_bush
                } else if detail(0x6F76_0003, x, y) < 0.90 {
                    ids.fence
                } else {
                    ids.dirt
                }
            } else {
                ids.farmland
            };
            w.push((x, y, t));
        }
    }
}

/// The town-specific blueprint writes for a hamlet or village.
pub(crate) fn town_writes(seed: i64, p: Placement, ids: &StructIds) -> Vec<(i32, i32, u8)> {
    let mut w = Vec::new();
    let (ox, oy) = (p.x, p.y);
    let detail = |salt: u64, x: i32, y: i32| unit(hash(seed, salt, x, y));

    match p.kind {
        StructureKind::Village => {
            // a village around a well: buildings ring a round plaza (variant 0) or
            // sit in the quadrants of two crossing worn roads (variant 1); paths
            // link the center to every doorway. How far gone it all is — walls,
            // paving, paths, flora — comes from the town's age axis ([`town_age`]).
            let variant = variant_of(seed, p);
            let age = town_age(seed, p);
            let ap = age_params(age);
            let ground = |x: i32, y: i32| {
                if detail(0x56C4_0006, x, y) < ap.paving {
                    ids.stone_floor // surviving paving stones
                } else if ap.overgrow > 0.0 && detail(0x6F76_0001, x, y) < ap.overgrow * 0.6 {
                    // Overgrown: tufts reclaiming the plaza and roads
                    ids.tall_grass[(hash(seed, 0x6F76_0002, x, y) % 3) as usize]
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
            // farming wave: the village field, gone to seed — a fenced plot between
            // plaza and building ring where corn rows (and the odd carrot) still
            // volunteer; breaking them yields the seed stock that starts a player's
            // own farm. Only aged villages keep one — a Settled village's tended
            // plot is its kitchen garden (`stamp_garden`). Stamped before paths and
            // buildings so anything later worn or built wins overlaps.
            if age != TownAge::Settled {
                // decay dials: Weathered is the classic look the plot was tuned
                // for; Overgrown is one bad summer from being meadow again
                let (fence_keep, revert) = match age {
                    TownAge::Overgrown => (0.22, 0.45),
                    _ => (0.45, 0.20),
                };
                let fh = hash(seed, 0x56C4_0010, ox, oy);
                // one of the four diagonals, nudged clear of the 12..15-tile
                // building ring; 6..8 tiles out
                let (qx, qy) = QUADRANT_DIRS[(fh % 4) as usize];
                let dist = 9 + ((fh >> 8) % 3) as i32;
                let (fx, fy) = (ox + qx * dist / 4, oy + qy * dist / 4);
                let (hw, hh) = (2 + ((fh >> 16) % 2) as i32, 2); // 5..7 x 5 tiles
                for dy in -hh..=hh {
                    for dx in -hw..=hw {
                        let (x, y) = (fx + dx, fy + dy);
                        let perimeter = dx.abs() == hw || dy.abs() == hh;
                        if perimeter {
                            // mostly-collapsed fence line
                            if detail(0x56C4_0011, x, y) < fence_keep {
                                w.push((x, y, ids.fence));
                            }
                            continue;
                        }
                        let t = if detail(0x56C4_0012, x, y) < revert {
                            // patch gone back to bare earth — or, Overgrown, to
                            // the same reclaiming tufts as the rest of the town
                            if ap.overgrow > 0.0 && detail(0x6F76_0001, x, y) < ap.overgrow * 0.6 {
                                ids.tall_grass[(hash(seed, 0x6F76_0002, x, y) % 3) as usize]
                            } else {
                                ids.dirt
                            }
                        } else if dx.rem_euclid(2) == 0 {
                            if detail(0x56C4_0013, x, y) < 0.15 {
                                ids.carrot_crop
                            } else {
                                ids.corn_crop
                            }
                        } else {
                            ids.farmland
                        };
                        w.push((x, y, t));
                    }
                }
            }
            let buildings = village_buildings(seed, ox, oy, variant);
            // paths before buildings, so the shells stamp cleanly over the path ends
            for &(bx, by, _, _) in &buildings {
                let mut line = Vec::new();
                raster_line(ox, oy, bx, by, &mut line);
                for (x, y) in line {
                    if detail(0x56C4_0009, x, y) < ap.path_gap {
                        continue; // worn away
                    }
                    w.push((x, y, ids.dirt));
                }
            }
            for &b in &buildings {
                let (bx, by, hw, hh) = b;
                let lantern = village_lantern_offset(ox, oy, bx, by, hw, hh);
                let (cupboard, barrel) = house_container_offsets(ox, oy, bx, by, hw, hh);
                // sound floor under the loot chest (center), lantern and cupboard
                stamp_house(
                    &mut w,
                    seed,
                    ids,
                    &ap,
                    (ox, oy),
                    b,
                    &[(0, 0), lantern, cupboard],
                );
                // packed ground where the rain barrel stands, flanking the door
                w.push((bx + barrel.0, by + barrel.1, ids.dirt));
            }
            // a Settled village keeps a tended kitchen garden by its first house,
            // and solid footing for its plaza lamp (entity via `lantern_positions`)
            if ap.overgrow == 0.0 && ap.crumble < 0.1 {
                stamp_garden(&mut w, seed, ids, (ox, oy), buildings[0]);
                w.push((ox - 3, oy - 2, ids.dirt));
            }
            // rarely, a Jack-O-Lantern still burns at the plaza edge of a razed
            // village — someone (or something) keeps lighting it (outside the 3x3
            // well footprint, inside the plaza circle, far from every building)
            if unit(hash(seed, 0x56C4_000A, ox, oy)) < 0.20 {
                w.push((ox + 3, oy + 2, ids.jack_o));
            }
            // the rubble well, last so it always crowns the plaza center; how much
            // of the ring has collapsed tracks the town's age
            let well_rubble = match age {
                TownAge::Overgrown => 0.70,
                TownAge::Weathered => 0.40,
                TownAge::Settled => 0.10,
            };
            for dy in -1..=1i32 {
                for dx in -1..=1i32 {
                    let (x, y) = (ox + dx, oy + dy);
                    let t = if dx == 0 && dy == 0 {
                        ids.water
                    } else if detail(0x56C4_0007, x, y) < well_rubble {
                        ids.rock // collapsed ring
                    } else {
                        ids.stone_wall
                    };
                    w.push((x, y, t));
                }
            }
        }
        StructureKind::Hamlet => {
            // the little towns between the set-piece villages: 2-5 houses in one of
            // three footprints — crossroads, ring around a green, or a straggle
            // along a lane — again on the age axis from time-lost to freshly kept
            let variant = variant_of(seed, p);
            let ap = age_params(town_age(seed, p));
            let buildings = hamlet_buildings(seed, ox, oy, variant);
            let ground = |x: i32, y: i32| {
                if detail(0x484D_0004, x, y) < ap.paving * 0.5 {
                    ids.stone_floor // hamlets were never as grandly paved
                } else if ap.overgrow > 0.0 && detail(0x6F76_0001, x, y) < ap.overgrow * 0.6 {
                    ids.tall_grass[(hash(seed, 0x6F76_0002, x, y) % 3) as usize]
                } else {
                    ids.dirt
                }
            };
            match variant {
                // two short worn roads crossing at the center
                0 => {
                    for d in -9..=9i32 {
                        let (hx, hy) = (ox + d, oy);
                        if detail(0x484D_0005, hx, hy) >= ap.path_gap * 0.8 {
                            w.push((hx, hy, ground(hx, hy)));
                        }
                        let (vx, vy) = (ox, oy + d);
                        if detail(0x484D_0006, vx, vy) >= ap.path_gap * 0.8 {
                            w.push((vx, vy, ground(vx, vy)));
                        }
                    }
                }
                // the green: a grassy round with a flower heart, tufts on the edge
                1 => {
                    for dy in -3..=3i32 {
                        for dx in -3..=3i32 {
                            if dx * dx + dy * dy > 11 {
                                continue;
                            }
                            let (x, y) = (ox + dx, oy + dy);
                            let t = if dx == 0 && dy == 0 {
                                ids.flower
                            } else if detail(0x484D_0007, x, y) < 0.12 {
                                ids.tall_grass[(hash(seed, 0x484D_0007, x, y) % 3) as usize]
                            } else {
                                ids.grass
                            };
                            w.push((x, y, t));
                        }
                    }
                }
                // the straggle: a winding lane through the strung-out houses
                _ => {
                    let h = hash(seed, 0x484D_0001, ox, oy);
                    let (sx, sy) = LANE_DIRS[((h >> 8) % 4) as usize];
                    for d in -14..=14i32 {
                        let (x, y) = (ox + sx * d, oy + sy * d);
                        if detail(0x484D_0005, x, y) >= ap.path_gap * 0.8 {
                            w.push((x, y, ground(x, y)));
                        }
                    }
                }
            }
            // paths from the center to every doorway, then the houses over them
            for &(bx, by, _, _) in &buildings {
                let mut line = Vec::new();
                raster_line(ox, oy, bx, by, &mut line);
                for (x, y) in line {
                    if detail(0x56C4_0009, x, y) < ap.path_gap {
                        continue; // worn away
                    }
                    w.push((x, y, ids.dirt));
                }
            }
            for &b in &buildings {
                let (bx, by, hw, hh) = b;
                let lantern = village_lantern_offset(ox, oy, bx, by, hw, hh);
                let (cupboard, barrel) = house_container_offsets(ox, oy, bx, by, hw, hh);
                // hamlets keep no loot chest — their cupboards and barrels are the
                // find — so only the lantern and cupboard tiles are guaranteed
                stamp_house(&mut w, seed, ids, &ap, (ox, oy), b, &[lantern, cupboard]);
                // packed ground where the rain barrel stands, flanking the door
                w.push((bx + barrel.0, by + barrel.1, ids.dirt));
            }
            // a Settled hamlet tends a garden and keeps its center lamp on solid
            // ground (the lamp entity spawns via `lantern_positions`)
            if ap.overgrow == 0.0 && ap.crumble < 0.1 {
                stamp_garden(&mut w, seed, ids, (ox, oy), buildings[0]);
                w.push((ox - 2, oy - 2, ids.dirt));
            }
            // the green's flower heart goes last, so the door-path pass never
            // tramples it (the village does the same with its well)
            if variant == 1 {
                w.push((ox, oy, ids.flower));
            }
        }
        _ => unreachable!("town_writes only accepts town placements"),
    }
    w
}
