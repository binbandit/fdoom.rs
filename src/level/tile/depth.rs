//! Multi-level terrain tiles (sandbox era, no Java counterpart):
//!
//! - **Deep Water** — open-ocean water too deep to swim; crossing it needs a Raft in the
//!   inventory (the player floats on it; other mobs can't pass).
//! - **Dug Pit** — what shoveling dirt/grass produces now. Tile data holds the dig stage
//!   (0..=MAX_STAGE); each shovel hit digs deeper until the pit bottoms out on rock.
//! - **Chasm** — a pit dug through to the layer below with a pickaxe. Standing on it
//!   drops you down one layer; the dig stamps a matching Ladder on arrival.
//! - **Ladder** — the way back up through a chasm dug from above.
//!
//! Together these replace pre-placed stairwells on infinite worlds: you descend by
//! digging, exactly like the user asked — "dig deep enough and you get down a level".

use super::{TileDef, TileKind, dispatch, tool_use};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::{Direction, Entity, EntityKind};
use crate::gfx::{Screen, color};
use crate::item::{Item, ItemKind, ToolType};
use crate::level::drop_item;
use crate::level::infinite_gen::hash;

/// Dig stages before the pit bottoms out on rock (then a pickaxe opens the chasm).
pub const MAX_STAGE: i32 = 2;

pub fn make_deep_water(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::DeepWater);
    def.connects_to_water = true;
    def
}

pub fn make_dug_pit(name: &str) -> TileDef {
    TileDef::new(name, TileKind::DugPit)
}

pub fn make_chasm(name: &str) -> TileDef {
    TileDef::new(name, TileKind::Chasm)
}

pub fn make_ladder(name: &str) -> TileDef {
    TileDef::new(name, TileKind::Ladder)
}

/* ------------------------------- ragged hole masks ------------------------------- */

/// Hand-drawn ragged hole outlines as per-row spans `(x0, x1)` inclusive; `(1, 0)`
/// marks an empty row. Three shapes x four mirror flips give twelve outline variants,
/// enough that neighboring holes never read as the same stamp.
const HOLE_MASKS: [[(i8, i8); 16]; 3] = [
    // round bowl, wobble on the right flank
    [
        (1, 0),
        (6, 9),
        (4, 11),
        (3, 12),
        (2, 12),
        (2, 13),
        (1, 13),
        (0, 14),
        (1, 14),
        (1, 13),
        (2, 13),
        (3, 12),
        (4, 11),
        (5, 10),
        (7, 9),
        (1, 0),
    ],
    // egg leaning left, notch bitten out of the left wall
    [
        (1, 0),
        (1, 0),
        (5, 10),
        (3, 11),
        (2, 12),
        (1, 12),
        (1, 13),
        (0, 13),
        (0, 14),
        (2, 13),
        (1, 12),
        (2, 12),
        (3, 11),
        (4, 9),
        (1, 0),
        (1, 0),
    ],
    // wide and flat-bottomed, bite on the upper right
    [
        (1, 0),
        (5, 8),
        (3, 10),
        (2, 13),
        (1, 11),
        (1, 14),
        (0, 14),
        (1, 13),
        (0, 15),
        (1, 14),
        (2, 13),
        (2, 12),
        (3, 12),
        (5, 11),
        (6, 9),
        (1, 0),
    ],
];

/// Salt for the per-tile hole-outline pick. Pits and chasms share it on purpose: when
/// a pit breaks through, the chasm keeps the outline of the pit it used to be.
const HOLE_SALT: u64 = 0x0D16_0D16_0D16_0D16;

/// The ragged outline of the hole dug at `(x, y)`: a hashed pick among `HOLE_MASKS`
/// plus mirror flips, shrunk by `inset` pixels on every side (shallow dig stages are
/// narrower). Pure `f(seed, x, y)` — stable frame to frame, no rng state.
fn hole_spans(seed: i64, x: i32, y: i32, inset: i32) -> [(i32, i32); 16] {
    let h = hash(seed, HOLE_SALT, x, y);
    let mask = &HOLE_MASKS[(h % 3) as usize];
    let mirror_x = h & 4 != 0;
    let mirror_y = h & 8 != 0;
    let mut spans = [(16i32, -1i32); 16];
    for (r, span) in spans.iter_mut().enumerate() {
        let (a, b) = mask[if mirror_y { 15 - r } else { r }];
        if a > b {
            continue;
        }
        let (a, b) = if mirror_x {
            (15 - b as i32, 15 - a as i32)
        } else {
            (a as i32, b as i32)
        };
        if a + inset <= b - inset {
            *span = (a + inset, b - inset);
        }
    }
    // vertical inset: peel the same number of rows off the blob's top and bottom
    if inset > 0
        && let Some((first, last)) = span_extent(&spans)
    {
        for (r, span) in spans.iter_mut().enumerate() {
            if (r as i32) < first + inset || (r as i32) > last - inset {
                *span = (16, -1);
            }
        }
    }
    spans
}

/// First and last non-empty rows of a span table, if any.
fn span_extent(spans: &[(i32, i32); 16]) -> Option<(i32, i32)> {
    let first = spans.iter().position(|&(a, b)| a <= b)? as i32;
    let last = spans.iter().rposition(|&(a, b)| a <= b)? as i32;
    Some((first, last))
}

/* --------------------------- merged excavation spaces --------------------------- */

/// Excavation "rank" for connectivity: how deep the excavated space is at `(x, y)`.
/// Pits rank by dig stage; a chasm outranks any pit (its floor already broke
/// through). `None` = not part of an excavation.
fn exc_rank(g: &Game, lvl: usize, x: i32, y: i32) -> Option<i32> {
    match g.tile_at(lvl, x, y).kind {
        TileKind::DugPit => Some(g.level(lvl).get_data(x, y).clamp(0, MAX_STAGE)),
        TileKind::Chasm => Some(MAX_STAGE + 1),
        _ => None,
    }
}

/// Excavation ranks of the four orthogonal neighbors, in N, S, W, E order.
fn exc_sides(g: &Game, lvl: usize, x: i32, y: i32) -> [Option<i32>; 4] {
    [
        exc_rank(g, lvl, x, y - 1),
        exc_rank(g, lvl, x, y + 1),
        exc_rank(g, lvl, x - 1, y),
        exc_rank(g, lvl, x + 1, y),
    ]
}

/// Salts for the passage openings across shared tile boundaries: one stream for
/// horizontal boundaries (between vertical neighbors), one for vertical.
const PASSAGE_SALT_H: u64 = 0x0D16_00AA;
const PASSAGE_SALT_V: u64 = 0x0D16_00BB;

/// Where the excavated space opens across a shared tile boundary: a pixel band
/// `(lo, hi)` inclusive, measured along the edge. Both tiles hash the same boundary
/// key `(bx, by)`, so the two halves of the opening line up seam-free; `inset`
/// narrows the passage where the shallower of the two digs pinches it (both sides
/// derive the same inset, so they still agree).
fn passage_band(seed: i64, salt: u64, bx: i32, by: i32, inset: i32) -> (i32, i32) {
    let h = hash(seed, salt, bx, by);
    let lo = 1 + inset + (h % 2) as i32;
    let hi = 14 - inset - ((h >> 2) % 2) as i32;
    (lo, hi)
}

/// A 16x16 pixel mask (bit `c` of row `r`) of the excavated area of tile `(x, y)`:
/// the tile's own ragged blob, opened up across every boundary shared with a fellow
/// excavated tile (`sides` in N, S, W, E order), plus filled corner quadrants where
/// two open sides meet around an excavated diagonal — so interior tiles of a merged
/// excavation come out as fully open floor while the outer boundary keeps the
/// hand-drawn ragged lip.
fn merged_mask(
    seed: i64,
    x: i32,
    y: i32,
    inset: i32,
    sides: [bool; 4],
    side_insets: [i32; 4],
    corners: [bool; 4],
) -> [u16; 16] {
    let spans = hole_spans(seed, x, y, inset);
    let mut m = [0u16; 16];
    for (r, &(a, b)) in spans.iter().enumerate() {
        for c in a.max(0)..=b.min(15) {
            m[r] |= 1 << c;
        }
    }
    // Passages: half-tile bands running from the shared edge through the center,
    // where they always overlap the blob. The band walls wander +-1px per step
    // inward — hashed on the boundary key and the depth `k`, so both tiles carve
    // the identical wander and the seam stays pixel-exact — keeping the opening
    // ragged instead of ruler-straight.
    let wander = |salt: u64, bx: i32, by: i32, base: (i32, i32), k: i32| -> (i32, i32) {
        let h = hash(
            seed,
            salt.wrapping_add(0x9E37_79B9 * (k as u64 + 1)),
            bx,
            by,
        );
        let lo = (base.0 + (h % 3) as i32 - 1).max(1);
        let hi = (base.1 - ((h >> 3) % 3) as i32 + 1).min(14);
        (lo, hi)
    };
    if sides[0] {
        let base = passage_band(seed, PASSAGE_SALT_H, x, y, side_insets[0]);
        for r in 0..=8i32 {
            let (lo, hi) = wander(PASSAGE_SALT_H, x, y, base, r);
            for c in lo..=hi {
                m[r as usize] |= 1 << c;
            }
        }
    }
    if sides[1] {
        let base = passage_band(seed, PASSAGE_SALT_H, x, y + 1, side_insets[1]);
        for r in 7..=15i32 {
            let (lo, hi) = wander(PASSAGE_SALT_H, x, y + 1, base, 15 - r);
            for c in lo..=hi {
                m[r as usize] |= 1 << c;
            }
        }
    }
    if sides[2] {
        let base = passage_band(seed, PASSAGE_SALT_V, x, y, side_insets[2]);
        for c in 0..=8i32 {
            let (lo, hi) = wander(PASSAGE_SALT_V, x, y, base, c);
            for r in lo..=hi {
                m[r as usize] |= 1 << c;
            }
        }
    }
    if sides[3] {
        let base = passage_band(seed, PASSAGE_SALT_V, x + 1, y, side_insets[3]);
        for c in 7..=15i32 {
            let (lo, hi) = wander(PASSAGE_SALT_V, x + 1, y, base, 15 - c);
            for r in lo..=hi {
                m[r as usize] |= 1 << c;
            }
        }
    }
    let mut fill = |r0: i32, r1: i32, c0: i32, c1: i32| {
        for r in r0..=r1 {
            for c in c0..=c1 {
                m[r as usize] |= 1 << c;
            }
        }
    };
    if corners[0] {
        fill(0, 7, 0, 7);
    }
    if corners[1] {
        fill(0, 7, 8, 15);
    }
    if corners[2] {
        fill(8, 15, 0, 7);
    }
    if corners[3] {
        fill(8, 15, 8, 15);
    }
    m
}

fn mask_at(m: &[u16; 16], r: i32, c: i32) -> bool {
    (0..16).contains(&r) && (0..16).contains(&c) && m[r as usize] & (1 << c) != 0
}

/// A computed excavation floor for one tile: the pixel mask plus where it sits.
struct FloorMask {
    x: i32,
    y: i32,
    m: [u16; 16],
}

/// Column extents of a mask: first and last set row per column (`i32::MAX`/`MIN`
/// for empty columns).
fn mask_extents(m: &[u16; 16]) -> ([i32; 16], [i32; 16]) {
    let mut top = [i32::MAX; 16];
    let mut bot = [i32::MIN; 16];
    for r in 0..16i32 {
        for c in 0..16i32 {
            if mask_at(m, r, c) {
                top[c as usize] = top[c as usize].min(r);
                bot[c as usize] = bot[c as usize].max(r);
            }
        }
    }
    (top, bot)
}

/// Shade an excavated floor mask under top light: dark crescent along the north
/// inner wall, lighter lip along the south inner wall, flat `band` darkness
/// elsewhere. Rim shading is suppressed where the floor runs to an open edge (a
/// shared boundary is a continuation, not a rim); `skip` pixels are left untouched
/// (the chasm paints its void over them anyway).
fn shade_floor(
    screen: &mut Screen,
    floor: &FloorMask,
    sides: [bool; 4],
    band: i32,
    skip: Option<&[u16; 16]>,
) {
    let (top, bot) = mask_extents(&floor.m);
    for r in 0..16i32 {
        for c in 0..16i32 {
            if !mask_at(&floor.m, r, c) || skip.is_some_and(|s| mask_at(s, r, c)) {
                continue;
            }
            let (ct, cb) = (top[c as usize], bot[c as usize]);
            let north_rim = !(sides[0] && ct == 0);
            let south_rim = !(sides[1] && cb == 15);
            let amount = if r == ct && north_rim {
                band + 55 // shadow under the north lip
            } else if r == ct + 1 && north_rim {
                band + 30
            } else if r == cb && south_rim {
                (band - 22).max(8) // south inner wall catches the light
            } else {
                band
            };
            screen.darken_rect(
                floor.x * 16 + c,
                floor.y * 16 + r,
                1,
                1,
                amount.clamp(0, 255),
            );
        }
    }
}

/// Salt for the terrace-step shadow jitter.
const STEP_SALT: u64 = 0x0D16_00CC;

/// Depth terracing: where an open neighbor is *shallower* than this tile, the step
/// drops onto our floor — a short ragged shadow band just inside that edge makes
/// the stage change readable without closing the shared boundary.
fn step_shadows(
    screen: &mut Screen,
    seed: i64,
    floor: &FloorMask,
    ranks: &[Option<i32>; 4],
    my_rank: i32,
    band: i32,
) {
    let (x, y) = (floor.x, floor.y);
    let amount = (band + 48).min(255);
    let stepped = |i: usize| matches!(ranks[i], Some(r) if r < my_rank);
    let mut px = |r: i32, c: i32| {
        if mask_at(&floor.m, r, c) {
            screen.darken_rect(x * 16 + c, y * 16 + r, 1, 1, amount);
        }
    };
    for c in 0..16i32 {
        let w = 2 + (hash(seed, STEP_SALT, x * 16 + c, y) % 2) as i32;
        if stepped(0) {
            for r in 0..w {
                px(r, c);
            }
        }
        if stepped(1) {
            for r in 16 - w..16 {
                px(r, c);
            }
        }
    }
    for r in 0..16i32 {
        let w = 2 + (hash(seed, STEP_SALT, y * 16 + r, x) % 2) as i32;
        if stepped(2) {
            for c in 0..w {
                px(r, c);
            }
        }
        if stepped(3) {
            for c in 16 - w..16 {
                px(r, c);
            }
        }
    }
}

/* ---------------------------------- deep water ---------------------------------- */

/// Only a player carrying a Raft (or a creative player) can cross deep water.
pub fn deep_water_may_pass(g: &Game, e: &Entity) -> bool {
    match &e.kind {
        EntityKind::Player(_) => {
            let inv = &e.player().inventory;
            g.is_mode("creative")
                || inv
                    .items()
                    .iter()
                    .any(|i| i.get_name().eq_ignore_ascii_case("Raft"))
        }
        // items drift over it; everything else is blocked
        EntityKind::ItemEntity(_) => true,
        _ => false,
    }
}

pub fn deep_water_render(g: &mut Game, screen: &mut Screen, lvl: usize, x: i32, y: i32) {
    // ride on the regular water art, darkened — reads as depth on any art style
    let water = g.tiles.get("water");
    dispatch::render(g, screen, &water, lvl, x, y);

    // edges facing shallow water feather out through ragged hashed contour bands
    // instead of ending in a hard square tile seam; interior tiles keep the flat
    // full-strength darken (one rect, the common case across open ocean)
    let shallow = |g: &Game, nx: i32, ny: i32| {
        let t = g.tile_at(lvl, nx, ny);
        t.connects_to_water && !matches!(t.kind, TileKind::DeepWater)
    };
    let open_n = shallow(g, x, y - 1);
    let open_s = shallow(g, x, y + 1);
    let open_w = shallow(g, x - 1, y);
    let open_e = shallow(g, x + 1, y);
    if !(open_n || open_s || open_w || open_e) {
        screen.darken_rect(x * 16, y * 16, 16, 16, 96);
    } else {
        // per-2px-strip jitter, keyed on absolute strip position so the contour runs
        // continuously across neighboring deep tiles that share the same open side
        let seed = g.world_seed;
        let jitter = |salt: u64, along: i32, across: i32| -> i32 {
            (hash(seed, salt, along, across) % 3) as i32 - 1
        };
        for py in 0..16 {
            for px in 0..16 {
                let mut d = 16; // jittered distance to the nearest shallow-facing edge
                if open_n {
                    d = d.min(py + jitter(0xD3E9_0001, x * 8 + px / 2, y));
                }
                if open_s {
                    d = d.min(15 - py + jitter(0xD3E9_0002, x * 8 + px / 2, y));
                }
                if open_w {
                    d = d.min(px + jitter(0xD3E9_0003, y * 8 + py / 2, x));
                }
                if open_e {
                    d = d.min(15 - px + jitter(0xD3E9_0004, y * 8 + py / 2, x));
                }
                let amount = match d {
                    ..=1 => 0,
                    2..=4 => 30,
                    5..=7 => 62,
                    _ => 96,
                };
                if amount > 0 {
                    screen.darken_rect(x * 16 + px, y * 16 + py, 1, 1, amount);
                }
            }
        }
    }

    // rolling waves: a shadow crest drifts across each tile on a phase offset from the
    // tile position, so the open ocean visibly swells instead of sitting flat
    let phase = ((g.tick_count / 6) + (x * 5 + y * 11)) & 31;
    if phase < 3 {
        let row = (x * 3 + y * 7) & 7;
        screen.darken_rect(x * 16, y * 16 + row * 2, 16, 2, 70);
    }
}

/* --------------------------- flooding excavations --------------------------- */

/// What water pouring into `(x, y)` becomes, if that tile is an excavation: the
/// flood assumes the depth of the hole — a shallow dig fills as ordinary water, a
/// bottomed-out pit or a chasm fills as Deep Water. (A flooded chasm no longer
/// drops you: the tile *is* deep water now. The carved pocket below stays dry —
/// the flood seals the breakthrough rather than pouring through it.)
pub fn flood_kind(g: &Game, lvl: usize, x: i32, y: i32) -> Option<&'static str> {
    match g.tile_at(lvl, x, y).kind {
        TileKind::DugPit => Some(if g.level(lvl).get_data(x, y) >= MAX_STAGE {
            "Deep Water"
        } else {
            "water"
        }),
        TileKind::Chasm => Some("Deep Water"),
        _ => None,
    }
}

/// Flood the excavation at `(x, y)` if there is one. Called from the random tile
/// tick of adjacent water (shallow and deep), so a pool spreads through a connected
/// pit network one visible tile at a time — dig a channel to the water and watch
/// the basin fill.
pub fn try_flood(g: &mut Game, lvl: usize, x: i32, y: i32) -> bool {
    let Some(kind) = flood_kind(g, lvl, x, y) else {
        return false;
    };
    let t = g.tiles.get(kind);
    g.set_tile_default(lvl, x, y, &t);
    true
}

/// Deep water spreads into adjacent excavations on the same one-random-neighbor
/// cadence as shallow water (`water::tick`), so a flooded deep basin keeps feeding
/// the network — but unlike shallow water it never creeps into plain holes or lava.
pub fn deep_water_tick(g: &mut Game, lvl: usize, xt: i32, yt: i32) {
    let mut xn = xt;
    let mut yn = yt;
    if g.random.next_boolean() {
        xn += g.random.next_int_bound(2) * 2 - 1;
    } else {
        yn += g.random.next_int_bound(2) * 2 - 1;
    }
    try_flood(g, lvl, xn, yn);
}

/* ----------------------------------- dug pit ------------------------------------ */

pub fn dug_pit_render(g: &mut Game, screen: &mut Screen, lvl: usize, x: i32, y: i32) {
    let stage = g.level(lvl).get_data(x, y).clamp(0, MAX_STAGE);
    let dirt = g.tiles.get("dirt");
    dispatch::render(g, screen, &dirt, lvl, x, y);

    // a ragged bowl, not a square — and not an island: each stage widens the lip
    // and darkens the core, but boundaries shared with a fellow pit or chasm open
    // up so adjacent digs read as ONE excavated space. Concavity comes from
    // per-column rim shading under top light — dark crescent along the north inner
    // wall, lighter lip along the south — suppressed where the floor continues
    // into the neighbor.
    let seed = g.world_seed;
    let ranks = exc_sides(g, lvl, x, y);
    let sides = ranks.map(|r| r.is_some());
    // the passage pinches to the shallower of the two digs it joins
    let side_insets = ranks.map(|r| MAX_STAGE - stage.min(r.unwrap_or(0).min(MAX_STAGE)));
    let corners = [
        sides[0] && sides[2] && exc_rank(g, lvl, x - 1, y - 1).is_some(),
        sides[0] && sides[3] && exc_rank(g, lvl, x + 1, y - 1).is_some(),
        sides[1] && sides[2] && exc_rank(g, lvl, x - 1, y + 1).is_some(),
        sides[1] && sides[3] && exc_rank(g, lvl, x + 1, y + 1).is_some(),
    ];
    let inset = MAX_STAGE - stage;
    let floor = FloorMask {
        x,
        y,
        m: merged_mask(seed, x, y, inset, sides, side_insets, corners),
    };

    let band = 40 + 26 * stage;
    shade_floor(screen, &floor, sides, band, None);

    // per-tile core mottle keeps a wide merged floor from reading flat
    let core = hole_spans(seed, x, y, inset + 4);
    let core_extra = 34 + 36 * stage;
    for (r, &(ca, cb)) in core.iter().enumerate() {
        if ca <= cb {
            screen.darken_rect(x * 16 + ca, y * 16 + r as i32, cb - ca + 1, 1, core_extra);
        }
    }

    step_shadows(screen, seed, &floor, &ranks, stage, band);

    if stage >= MAX_STAGE {
        // rock speckle so the "you need a pickaxe now" state is readable
        let col = color::get(-1, 333);
        screen.render(x * 16 + 4, y * 16 + 4, 2 + 29 * 32, col, 0);
    }
}

pub fn dug_pit_interact(
    g: &mut Game,
    lvl: usize,
    xt: i32,
    yt: i32,
    player: &mut Entity,
    item: &mut Item,
    _attack_dir: Direction,
) -> bool {
    let stage = g.level(lvl).get_data(xt, yt);

    // building in the hole: a dug pit takes floor material exactly like the classic
    // hole (boarding the dig over), and shovel-loads of dirt backfill it stage by
    // stage — a mis-dug pit is reversible, and a basin can be floored for a base.
    // (The item pass already rejected these placements — floor items only list the
    // classic hole/water as valid tiles — so the tile handles them here.)
    if let ItemKind::TileItem { model, .. } = &item.kind {
        let model = model.clone();
        let placed = match model.as_str() {
            "WOOD PLANKS" | "STONE BRICKS" | "OBSIDIAN" => {
                g.set_tile_named(lvl, xt, yt, &model);
                // the item pass pushed a misleading "dig a hole first" note before
                // this handler ran — retract it, the hole is right here
                if g.notifications.last().map(String::as_str) == Some("Dig a hole first!") {
                    g.sync_note_ages();
                    g.notifications.pop();
                    g.note_ages.pop();
                }
                true
            }
            "DIRT" => {
                if stage > 0 {
                    g.level_mut(lvl).set_data(xt, yt, stage - 1);
                } else {
                    let dirt = g.tiles.get("dirt");
                    g.set_tile_default(lvl, xt, yt, &dirt);
                }
                true
            }
            _ => return false,
        };
        if placed && !g.is_mode("creative") {
            if let Some(count) = item.count_mut() {
                *count -= 1;
            }
        }
        return placed;
    }

    let ItemKind::Tool { ttype, .. } = item.kind else {
        return false;
    };

    if ttype == ToolType::Shovel && stage < MAX_STAGE {
        if tool_use(g, player, item, ToolType::Shovel, 4).is_some() {
            g.level_mut(lvl).set_data(xt, yt, stage + 1);
            let dirt = crate::item::registry::get(g, "dirt");
            drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, dirt);
            if stage + 1 == MAX_STAGE {
                g.push_warning("The pit hits solid rock.");
            }
            g.play_sound(Sound::MonsterHurt);
            return true;
        }
        return false;
    }
    if ttype == ToolType::Shovel && stage >= MAX_STAGE {
        g.push_warning("Too rocky - a pickaxe could break through.");
        return false;
    }
    if ttype == ToolType::Pickaxe
        && stage >= MAX_STAGE
        && tool_use(g, player, item, ToolType::Pickaxe, 4).is_some()
    {
        open_chasm(g, lvl, xt, yt);
        let stone = crate::item::registry::get(g, "Stone");
        drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, stone);
        g.play_sound(Sound::MonsterHurt);
        return true;
    }
    false
}

/// Break through the pit floor: this tile becomes a chasm, and the layer below gets a
/// carved pocket with a ladder back up at the same coordinates.
fn open_chasm(g: &mut Game, lvl: usize, xt: i32, yt: i32) {
    let chasm = g.tiles.get("chasm");
    g.set_tile_default(lvl, xt, yt, &chasm);

    if lvl == 0 {
        return; // nothing below the deepest mine (the dungeon is gated, not dug into)
    }
    let below = lvl - 1;
    if g.levels[below].is_none() {
        return;
    }
    // make sure the destination chunk exists before carving into it
    crate::level::ensure_chunks_at(g, below, xt, yt, true);

    let dirt = g.tiles.get("dirt");
    for dy in -1..=1 {
        for dx in -1..=1 {
            g.set_tile_default(below, xt + dx, yt + dy, &dirt);
        }
    }
    let ladder = g.tiles.get("ladder");
    g.set_tile_default(below, xt, yt, &ladder);
}

/* --------------------------------- chasm / ladder --------------------------------- */

pub fn chasm_render(g: &mut Game, screen: &mut Screen, lvl: usize, x: i32, y: i32) {
    let dirt = g.tiles.get("dirt");
    dispatch::render(g, screen, &dirt, lvl, x, y);

    // the chasm sits inside the excavation that broke through: around the black
    // opening the tile is dug floor at full depth, connected to neighboring pits
    // and chasms exactly like a pit — so a breakthrough inside a big dig doesn't
    // punch a square of untouched dirt into the merged floor
    let seed = g.world_seed;
    let ranks = exc_sides(g, lvl, x, y);
    let sides = ranks.map(|r| r.is_some());
    let side_insets = ranks.map(|r| MAX_STAGE - r.unwrap_or(0).min(MAX_STAGE));
    let corners = [
        sides[0] && sides[2] && exc_rank(g, lvl, x - 1, y - 1).is_some(),
        sides[0] && sides[3] && exc_rank(g, lvl, x + 1, y - 1).is_some(),
        sides[1] && sides[2] && exc_rank(g, lvl, x - 1, y + 1).is_some(),
        sides[1] && sides[3] && exc_rank(g, lvl, x + 1, y + 1).is_some(),
    ];

    // the opening itself: a ragged black void (same outline family as the pit that
    // broke through here), merging with adjacent chasm voids into one drop
    let is_chasm = |r: &Option<i32>| matches!(r, Some(v) if *v > MAX_STAGE);
    let void_sides = [
        is_chasm(&ranks[0]),
        is_chasm(&ranks[1]),
        is_chasm(&ranks[2]),
        is_chasm(&ranks[3]),
    ];
    let chasm_diag = |dx: i32, dy: i32| is_chasm(&exc_rank(g, lvl, x + dx, y + dy));
    let void_corners = [
        void_sides[0] && void_sides[2] && chasm_diag(-1, -1),
        void_sides[0] && void_sides[3] && chasm_diag(1, -1),
        void_sides[1] && void_sides[2] && chasm_diag(-1, 1),
        void_sides[1] && void_sides[3] && chasm_diag(1, 1),
    ];
    let void = merged_mask(seed, x, y, 0, void_sides, [0; 4], void_corners);

    if sides.iter().any(|&s| s) {
        let floor = FloorMask {
            x,
            y,
            m: merged_mask(seed, x, y, 0, sides, side_insets, corners),
        };
        let band = 40 + 26 * MAX_STAGE;
        shade_floor(screen, &floor, sides, band, Some(&void));
        step_shadows(screen, seed, &floor, &ranks, MAX_STAGE + 1, band);
    }

    // The bottom-most pixels of each column stay partly lit — the south inner wall
    // catches top light — and the north lip gets a crumble shadow just outside the
    // opening, so it reads as a hole, not a stamp. Both are suppressed where the
    // void continues into a neighboring chasm.
    let (top, bot) = mask_extents(&void);
    for r in 0..16i32 {
        for c in 0..16i32 {
            if !mask_at(&void, r, c) {
                continue;
            }
            let cb = bot[c as usize];
            let south_open = void_sides[1] && cb == 15;
            let amount = if r == cb && !south_open {
                140 // lit rim: the south inner wall, irregular per column
            } else if r == cb - 1 && !south_open {
                225
            } else {
                255
            };
            screen.darken_rect(x * 16 + c, y * 16 + r, 1, 1, amount);
        }
    }
    for (c, &t) in top.iter().enumerate() {
        if t == i32::MAX {
            continue;
        }
        if t > 0 {
            screen.darken_rect(x * 16 + c as i32, y * 16 + t - 1, 1, 1, 80);
        }
        if t > 1 {
            screen.darken_rect(x * 16 + c as i32, y * 16 + t - 2, 1, 1, 35);
        }
    }
}

pub fn ladder_render(g: &mut Game, screen: &mut Screen, lvl: usize, x: i32, y: i32) {
    // reuse the stairs-up glyph over dirt: an unmistakable "up" affordance
    let stairs_up = g.tiles.get("Stairs Up");
    let dirt = g.tiles.get("dirt");
    dispatch::render(g, screen, &dirt, lvl, x, y);
    if let Some(sprite) = stairs_up.sprite.clone() {
        sprite.render(screen, x * 16, y * 16);
    }
}
