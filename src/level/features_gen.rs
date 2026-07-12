//! Wild terrain features for infinite worlds (content wave): **hot springs** and
//! **abandoned mine shafts**. These live apart from `structures_gen` deliberately —
//! they are landforms of the *inhospitable* biomes (Tundra/Mountains), outside the
//! settlement kinds' biome envelope, and the shaft is the one feature that writes to
//! TWO layers (a surface headframe whose pre-carved gallery generates at the same
//! coordinates one mine layer down).
//!
//! Placement follows the `infinite_gen::gate_in_cell` hash-grid pattern: one coarse
//! cell grid per feature, at most one feature per cell at a jittered, biome-gated
//! point — pure `f(seed, cell)`. Blueprints are pure `f(seed, origin)` writes, so a
//! border-straddling feature stamps identically from every chunk, and the gallery a
//! layer below needs no knowledge of which chunk asked.
//!
//! - **Hot spring**: a 2-5 tile ragged pool of Spring Water in Tundra/Mountains
//!   (never freezes, steams, clamps cold to comfort in basking range — see
//!   `tile/spring_water.rs` and `core::temperature`), with the odd sitting stone on
//!   its rim.
//! - **Abandoned mine shaft** (fossicking identity): a weathered headframe on
//!   Mountains rock — timber props, a plank floor remnant, rubble — around a CHASM
//!   mouth. One layer down, the matching gallery: a carved dirt pocket, the ladder
//!   back up, standing props (which genuinely suppress cave-ins — radius 3 covers
//!   the room), weak rubble rocks, an iron-vein bias scaled by the shared
//!   `richness_at` field, and usually a supply crate of mining gear
//!   ([`spawn_chunk_entities`], the scav-container machinery).

use super::chunk::{CHUNK_SIZE, Chunk, chunk_coord};
use super::infinite_gen::{Biome, biome_at, hash, richness_at, unit};
use super::tile::{Tiles, fossick};
use crate::core::game::Game;
use crate::rng::Rng;

/// Largest half-extent of any feature footprint (stamp/query padding).
pub const FEATURE_RADIUS: i32 = 4;

/* ------------------------------------ placement ------------------------------------ */

/// Hot springs: one grid cell in ~2 carries a spring, if the jittered point lands in
/// cold country.
pub const SPRING_GRID: i32 = 192;
const SPRING_SALT: u64 = 0x4807_5350_0001; // "HOT SPring"
const SPRING_POOL_SALT: u64 = 0x4807_5350_0002;
const SPRING_STONE_SALT: u64 = 0x4807_5350_0003;

/// Mine shafts: rarer, and only where the old workings make sense — Mountains rock.
pub const SHAFT_GRID: i32 = 288;
const SHAFT_SALT: u64 = 0x4D1E_5AF7_0001; // "MINE SHAFT"
const SHAFT_DETAIL_SALT: u64 = 0x4D1E_5AF7_0002;
const SHAFT_CRATE_SALT: u64 = 0x4D1E_5AF7_0003;
const SHAFT_LOOT_SALT: u64 = 0x4D1E_5AF7_100D;

fn feature_in_cell(
    seed: i64,
    salt: u64,
    grid: i32,
    odds: f64,
    cell_x: i32,
    cell_y: i32,
    biome_ok: fn(Biome) -> bool,
) -> Option<(i32, i32)> {
    let h = hash(seed, salt, cell_x, cell_y);
    if unit(h) > odds {
        return None;
    }
    let margin = FEATURE_RADIUS + 1;
    let jx = margin + ((h >> 8) as i32).rem_euclid(grid - 2 * margin);
    let jy = margin + ((h >> 24) as i32).rem_euclid(grid - 2 * margin);
    let (x, y) = (cell_x * grid + jx, cell_y * grid + jy);
    biome_ok(biome_at(seed, x, y)).then_some((x, y))
}

/// The hot spring (if any) of a spring-grid cell. Pure.
pub fn spring_in_cell(seed: i64, cell_x: i32, cell_y: i32) -> Option<(i32, i32)> {
    feature_in_cell(seed, SPRING_SALT, SPRING_GRID, 0.55, cell_x, cell_y, |b| {
        matches!(b, Biome::Tundra | Biome::Mountains)
    })
}

/// The mine shaft (if any) of a shaft-grid cell. Pure.
pub fn shaft_in_cell(seed: i64, cell_x: i32, cell_y: i32) -> Option<(i32, i32)> {
    feature_in_cell(seed, SHAFT_SALT, SHAFT_GRID, 0.60, cell_x, cell_y, |b| {
        matches!(b, Biome::Mountains)
    })
}

fn cells_over(
    seed: i64,
    grid: i32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    get: fn(i64, i32, i32) -> Option<(i32, i32)>,
) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for cy in x_div(y0 - grid, grid)..=x_div(y1 + grid, grid) {
        for cx in x_div(x0 - grid, grid)..=x_div(x1 + grid, grid) {
            if let Some((x, y)) = get(seed, cx, cy) {
                if x >= x0 && x <= x1 && y >= y0 && y <= y1 {
                    out.push((x, y));
                }
            }
        }
    }
    out
}

fn x_div(v: i32, grid: i32) -> i32 {
    v.div_euclid(grid)
}

/// Every hot spring with an origin inside the rect. Deterministic order.
pub fn springs_in_rect(seed: i64, x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    cells_over(seed, SPRING_GRID, x0, y0, x1, y1, spring_in_cell)
}

/// Every mine shaft with an origin inside the rect. Deterministic order.
pub fn shafts_in_rect(seed: i64, x0: i32, y0: i32, x1: i32, y1: i32) -> Vec<(i32, i32)> {
    cells_over(seed, SHAFT_GRID, x0, y0, x1, y1, shaft_in_cell)
}

/* ------------------------------------ blueprints ------------------------------------ */

/// The pool tiles of one spring: the origin plus two *orthogonal* cardinals (an
/// L-core, hash-rotated — never a straight canal strip, and the corner gives the
/// water shimmer a full interior quadrant), plus sometimes one more — a 3-4 tile
/// ragged puddle.
pub fn spring_pool_tiles(seed: i64, sx: i32, sy: i32) -> Vec<(i32, i32)> {
    const CARDINALS: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
    let h = hash(seed, SPRING_POOL_SALT, sx, sy);
    let g0 = (h % 4) as usize;
    let g1 = (g0 + 1) % 4; // the clockwise neighbor: always orthogonal to g0
    let mut out = vec![(sx, sy)];
    for (i, (dx, dy)) in CARDINALS.into_iter().enumerate() {
        if out.len() >= 4 {
            break;
        }
        let roll = unit(hash(seed, SPRING_POOL_SALT ^ 0xA5, sx + dx, sy + dy));
        if i == g0 || i == g1 || roll < 0.45 {
            out.push((sx + dx, sy + dy));
        }
    }
    out
}

/// Tile writes of one hot spring, in stamping order. Pure.
pub fn spring_writes(seed: i64, sx: i32, sy: i32, tiles: &Tiles) -> Vec<(i32, i32, u8)> {
    let spring = tiles.get("Spring Water").id;
    let rock = tiles.get("rock").id;
    let mut w = Vec::new();
    // the odd sitting stone on the rim first, so the pool always wins an overlap
    for (i, (dx, dy)) in [(2, 1), (-2, -1), (1, -2)].into_iter().enumerate() {
        if unit(hash(seed, SPRING_STONE_SALT.wrapping_add(i as u64), sx, sy)) < 0.4 {
            w.push((sx + dx, sy + dy, rock));
        }
    }
    for (x, y) in spring_pool_tiles(seed, sx, sy) {
        w.push((x, y, spring));
    }
    w
}

/// Surface tile writes of one mine shaft: spoil apron, plank floor remnant, the
/// headframe props, rubble, and the chasm mouth last. Pure.
pub fn shaft_surface_writes(seed: i64, sx: i32, sy: i32, tiles: &Tiles) -> Vec<(i32, i32, u8)> {
    let dirt = tiles.get("dirt").id;
    let rock = tiles.get("rock").id;
    let planks = tiles.get("Wood Planks").id;
    let prop = tiles.get("Timber Prop").id;
    let chasm = tiles.get("Chasm").id;
    let detail = |x: i32, y: i32| unit(hash(seed, SHAFT_DETAIL_SALT, x, y));
    let mut w = Vec::new();
    // trampled spoil apron, ragged at the edge
    for dy in -2..=2i32 {
        for dx in -2..=2i32 {
            let d2 = dx * dx + dy * dy;
            let (x, y) = (sx + dx, sy + dy);
            if d2 <= 4 || (d2 <= 8 && detail(x, y) < 0.55) {
                w.push((x, y, dirt));
            }
        }
    }
    // what's left of the hoist-shed floor, north of the mouth
    for dx in -1..=1i32 {
        if detail(sx + dx, sy - 2) < 0.80 {
            w.push((sx + dx, sy - 2, planks));
        }
    }
    // the headframe: two standing timber uprights flanking the mouth
    w.push((sx - 1, sy - 1, prop));
    w.push((sx + 1, sy - 1, prop));
    // rubble ring: spoil rocks dumped around the working
    for (dx, dy) in [(2, 1), (-2, 1), (1, 2), (-2, -1), (2, -2)] {
        if detail(sx + dx + 40, sy + dy + 40) < 0.55 {
            w.push((sx + dx, sy + dy, rock));
        }
    }
    // the shaft mouth, last so nothing buries it
    w.push((sx, sy, chasm));
    w
}

/// Gallery writes one mine layer below, as `(x, y, tile, data)` — data carries the
/// rubble flag on the fallen rocks. Pure; the whole pocket is carved
/// unconditionally over whatever `mine_tile` generated, so the room exists no
/// matter how the cave noise rolled.
pub fn shaft_gallery_writes(seed: i64, sx: i32, sy: i32, tiles: &Tiles) -> Vec<(i32, i32, u8, u8)> {
    let dirt = tiles.get("dirt").id;
    let rock = tiles.get("rock").id;
    let prop = tiles.get("Timber Prop").id;
    let ladder = tiles.get("Ladder").id;
    let iron = tiles.get("iron ore").id;
    let lapis = tiles.get("lapis").id;
    let detail = |x: i32, y: i32| unit(hash(seed, SHAFT_DETAIL_SALT ^ 0x6A11, x, y));
    let mut w: Vec<(i32, i32, u8, u8)> = Vec::new();

    // the carved room: a ragged pocket of open floor
    for dy in -3..=3i32 {
        for dx in -3..=3i32 {
            let d2 = dx * dx + dy * dy;
            let (x, y) = (sx + dx, sy + dy);
            if d2 <= 5 || (d2 <= 9 && detail(x, y) < 0.45) {
                w.push((x, y, dirt, 0));
            }
        }
    }
    // the vein the old-timers were chasing: iron pips seeded into the surrounding
    // rock, denser on genuinely rich ground (the shared fossicking field — chase
    // it outward with a pickaxe and the vein_ping sparkles take over)
    let richness = richness_at(seed, sx, sy);
    let want = 2 + (richness * 4.0) as usize;
    let mut placed = 0usize;
    for (i, (dx, dy)) in [
        (-3, 1),
        (3, 0),
        (0, -3),
        (2, 2),
        (-2, -3),
        (3, -2),
        (-3, -2),
        (1, 3),
    ]
    .into_iter()
    .enumerate()
    {
        if placed >= want {
            break;
        }
        if unit(hash(seed, SHAFT_DETAIL_SALT.wrapping_add(i as u64), sx, sy)) < 0.75 {
            // the odd blue fleck among the iron, like the natural depth -1 veins
            let ore = if unit(hash(seed, SHAFT_DETAIL_SALT ^ 0x1AB5, sx + dx, sy + dy)) < 0.12 {
                lapis
            } else {
                iron
            };
            w.push((sx + dx, sy + dy, ore, 0));
            placed += 1;
        }
    }
    // standing props (radius 3 genuinely suppresses cave-ins across the room —
    // the gallery quietly teaches what timber is for)
    w.push((sx - 1, sy + 1, prop, 0));
    if detail(sx + 50, sy + 50) < 0.6 {
        w.push((sx + 2, sy - 1, prop, 0));
    }
    // roof-fall rubble: weak rock, fast to clear (the rubble data flag)
    for (dx, dy) in [(-2, 2), (1, -2)] {
        if detail(sx + dx + 60, sy + dy + 60) < 0.6 {
            w.push((sx + dx, sy + dy, rock, fossick::RUBBLE_FLAG as u8));
        }
    }
    // guaranteed sound floor where the supply crate stands, and the way home
    w.push((sx + 1, sy + 1, dirt, 0));
    w.push((sx, sy, ladder, 0));
    w
}

/* ----------------------------------- chunk stamping ---------------------------------- */

/// Stamp every feature overlapping the chunk. Called from
/// `infinite_gen::generate_chunk` after the structure pass (features win the
/// vanishingly rare overlap) and before the gate set-pieces (gates always win).
/// Depth 0 stamps springs + headframes; depth -1 stamps the shaft galleries.
pub fn stamp_chunk(seed: i64, depth: i32, cx: i32, cy: i32, chunk: &mut Chunk, tiles: &Tiles) {
    let base_x = cx * CHUNK_SIZE;
    let base_y = cy * CHUNK_SIZE;
    let (x0, y0) = (base_x - FEATURE_RADIUS, base_y - FEATURE_RADIUS);
    let (x1, y1) = (
        base_x + CHUNK_SIZE - 1 + FEATURE_RADIUS,
        base_y + CHUNK_SIZE - 1 + FEATURE_RADIUS,
    );
    let put = |chunk: &mut Chunk, x: i32, y: i32, t: u8, d: u8| {
        let (lx, ly) = (x - base_x, y - base_y);
        if (0..CHUNK_SIZE).contains(&lx) && (0..CHUNK_SIZE).contains(&ly) {
            let i = (lx + ly * CHUNK_SIZE) as usize;
            chunk.tiles[i] = t;
            chunk.data[i] = d;
        }
    };
    match depth {
        0 => {
            for (sx, sy) in springs_in_rect(seed, x0, y0, x1, y1) {
                for (x, y, t) in spring_writes(seed, sx, sy, tiles) {
                    put(chunk, x, y, t, 0);
                }
            }
            for (sx, sy) in shafts_in_rect(seed, x0, y0, x1, y1) {
                for (x, y, t) in shaft_surface_writes(seed, sx, sy, tiles) {
                    put(chunk, x, y, t, 0);
                }
            }
        }
        -1 => {
            for (sx, sy) in shafts_in_rect(seed, x0, y0, x1, y1) {
                for (x, y, t, d) in shaft_gallery_writes(seed, sx, sy, tiles) {
                    put(chunk, x, y, t, d);
                }
            }
        }
        _ => {}
    }
}

/* ------------------------------------ supply crate ------------------------------------ */

/// Spawn feature entities for a chunk that generated fresh: the shaft gallery's
/// supply crate (depth -1). Same idempotence contract as
/// `structures_gen::spawn_chunk_entities` — the touched chunk goes dirty, persists,
/// and never runs this path again.
pub fn spawn_chunk_entities(g: &mut Game, lvl: usize, cx: i32, cy: i32) {
    if g.level(lvl).depth != -1 || !g.level(lvl).is_infinite() {
        return;
    }
    let seed = g.world_seed;
    let base_x = cx * CHUNK_SIZE;
    let base_y = cy * CHUNK_SIZE;
    let shafts = shafts_in_rect(
        seed,
        base_x - FEATURE_RADIUS,
        base_y - FEATURE_RADIUS,
        base_x + CHUNK_SIZE - 1 + FEATURE_RADIUS,
        base_y + CHUNK_SIZE - 1 + FEATURE_RADIUS,
    );
    for (sx, sy) in shafts {
        let (tx, ty) = (sx + 1, sy + 1); // the guaranteed-floor corner of the gallery
        if chunk_coord(tx) != cx || chunk_coord(ty) != cy {
            continue; // another chunk owns this crate
        }
        if unit(hash(seed, SHAFT_CRATE_SALT, sx, sy)) > 0.65 {
            continue; // some galleries were stripped long ago
        }
        use crate::entity::furniture::scav_container::{self, ScavKind};
        let mut crate_e = scav_container::new(ScavKind::Crate);
        fill_shaft_crate(g, &mut crate_e, hash(seed, SHAFT_LOOT_SALT, tx, ty));
        g.level_mut(lvl).add_at(crate_e, tx, ty, true, lvl);
        // touch the tile's data byte (same value) purely to set the dirty flag
        let data = g.level(lvl).get_data(tx, ty);
        g.level_mut(lvl).set_data(tx, ty, data);
    }
}

/// A dead miner's kit, deterministic per crate position: props, coal, spare cordage,
/// sometimes a pan — rarely the Vice that unlocks THE BENCH's metalwork.
fn fill_shaft_crate(g: &mut Game, container: &mut crate::entity::Entity, h: u64) {
    use crate::item::registry::get;
    let mut rnd = Rng::new(h as i64);
    // (1-in-chance, item, count) — the structure-chest convention
    let loot: &[(i32, &str, i32)] = &[
        (2, "Timber Prop", 2),
        (2, "Coal", 4),
        (2, "Torch", 3),
        (3, "Stone", 5),
        (3, "Cord", 2),
        (4, "Prospector's Pan", 1),
        (5, "Iron Ore", 3),
        (8, "Iron", 2),
        (9, "Vice", 1),
        (12, "gem", 1),
    ];
    let inventory = &mut container.chest_mut().expect("shaft crate").inventory;
    for &(chance, name, num) in loot {
        let item = get(g, name);
        inventory.try_add_num(&mut rnd, chance, Some(item), num);
    }
    // an abandoned working always leaves SOMETHING on the shelf
    if inventory.inv_size() < 1 {
        inventory.add_num(get(g, "Torch"), 2);
        inventory.add_num(get(g, "Stone"), 3);
    }
}
