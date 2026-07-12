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

/* ----------------------------------- dug pit ------------------------------------ */

pub fn dug_pit_render(g: &mut Game, screen: &mut Screen, lvl: usize, x: i32, y: i32) {
    let stage = g.level(lvl).get_data(x, y).clamp(0, MAX_STAGE);
    let dirt = g.tiles.get("dirt");
    dispatch::render(g, screen, &dirt, lvl, x, y);

    // a ragged bowl, not a square: each stage widens the lip and darkens the core.
    // Concavity comes from per-column rim shading under top light — a dark crescent
    // along the north inner wall, a lighter lip along the south inner wall — while
    // the corners stay untouched dirt.
    let seed = g.world_seed;
    let inset = MAX_STAGE - stage;
    let outer = hole_spans(seed, x, y, inset);
    let core = hole_spans(seed, x, y, inset + 4);
    let mut top = [i32::MAX; 16];
    let mut bot = [i32::MIN; 16];
    for r in 0..16i32 {
        let (a, b) = outer[r as usize];
        for c in a.max(0)..=b.min(15) {
            top[c as usize] = top[c as usize].min(r);
            bot[c as usize] = bot[c as usize].max(r);
        }
    }
    let band = 40 + 26 * stage;
    let core_extra = 34 + 36 * stage;
    for r in 0..16i32 {
        let (a, b) = outer[r as usize];
        for c in a.max(0)..=b.min(15) {
            let (ct, cb) = (top[c as usize], bot[c as usize]);
            let amount = if r == ct {
                band + 55 // shadow under the north lip
            } else if r == ct + 1 {
                band + 30
            } else if r == cb {
                (band - 22).max(8) // south inner wall catches the light
            } else {
                band
            };
            screen.darken_rect(x * 16 + c, y * 16 + r, 1, 1, amount.clamp(0, 255));
        }
        let (ca, cb) = core[r as usize];
        if ca <= cb {
            screen.darken_rect(x * 16 + ca, y * 16 + r, cb - ca + 1, 1, core_extra);
        }
    }
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
    let ItemKind::Tool { ttype, .. } = item.kind else {
        return false;
    };
    let stage = g.level(lvl).get_data(xt, yt);

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

    // a ragged black opening through the floor (same outline family as the pit that
    // broke through here). The bottom-most pixels of each column stay partly lit —
    // the south inner wall catches top light — and the north lip gets a crumble
    // shadow just outside the opening, so it reads as a hole, not a stamp.
    let seed = g.world_seed;
    let spans = hole_spans(seed, x, y, 0);
    let mut top = [i32::MAX; 16];
    let mut bot = [i32::MIN; 16];
    for r in 0..16i32 {
        let (a, b) = spans[r as usize];
        for c in a.max(0)..=b.min(15) {
            top[c as usize] = top[c as usize].min(r);
            bot[c as usize] = bot[c as usize].max(r);
        }
    }
    for r in 0..16i32 {
        let (a, b) = spans[r as usize];
        for c in a.max(0)..=b.min(15) {
            let amount = if r == bot[c as usize] {
                140 // lit rim: the south inner wall, irregular per column
            } else if r == bot[c as usize] - 1 {
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
