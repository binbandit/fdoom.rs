//! Ruin, camp, cemetery, standing-stone, and boulder tile stamping.

use super::super::chunk::{CHUNK_SIZE, Chunk};
use super::super::infinite_gen::{Biome, RiverZone, biome_at, hash, river_zone_at, unit};
use super::super::tile::Tiles;
use super::loot::ruins_chest_offset;
use super::placement::{
    MAX_RADIUS, Placement, StructIds, StructureKind, placements_in_rect, trail_writes,
    trails_in_rect, variant_of,
};
use super::towns::town_writes;

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
            // the chest and container tiles are always sound floor, whatever the
            // shape rolled (the container offsets mirror `container_positions`)
            let (cdx, cdy) = ruins_chest_offset(variant);
            w.push((ox + cdx, oy + cdy, ids.stone_floor));
            let (sdx, sdy) = if variant == 1 { (-2, -3) } else { (1, -1) };
            w.push((ox + sdx, oy + sdy, ids.stone_floor));
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
        StructureKind::Village | StructureKind::Hamlet => {
            return town_writes(seed, p, ids);
        }
    }
    w
}

/* -------------------------------------- trails --------------------------------------- */

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
                } else if chunk.tiles[i] == ids.water
                    && matches!(river_zone_at(seed, x, y), Some(RiverZone::Channel))
                {
                    // the trail crosses the river on a plank footbridge (ponds and
                    // marsh pools stay forded as gaps, as before)
                    chunk.tiles[i] = ids.planks;
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
