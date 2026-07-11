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
    screen.darken_rect(x * 16, y * 16, 16, 16, 96);

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
    // deeper stage = darker pit; the last stage shows the rocky bottom
    screen.darken_rect(x * 16, y * 16, 16, 16, 48 + stage * 48);
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
    screen.darken_rect(x * 16, y * 16, 16, 16, 224);
    screen.darken_rect(x * 16 + 2, y * 16 + 2, 12, 12, 255);
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
