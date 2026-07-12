//! Beehive (content wave — ROADMAP's "beehives in forests"): a wild hive hanging on
//! a broadleaf forest tree. Harvest it for Honeycomb:
//!
//! - **Bare-handed** (any hit): always yields Honeycomb 1-2, but roughly one pull in
//!   three costs a sting — 1 damage and a "Bees!" cue. Approachable, never a
//!   swarm-chase.
//! - **Torch held** (tile interact): smoke the bees calm first — always safe. The
//!   torch is not consumed; smoking is knowledge, not a toll.
//!
//! Per-tile data byte: 0 = full hive, 1 = harvested/regrowing. Random ticks re-fill
//! a harvested hive over a few in-game days (the berry-bush timer family, slower).
//! Hitting a *harvested* hive knocks the husk down, leaving the plain Tree — so a
//! hive tree chops like any other once the bees are dealt with, and the data byte
//! never doubles as tree damage.
//!
//! TODO(art): a dedicated hive-lump cell — the render reuses the nugget item cell
//! under amber (full) / husk (harvested) palettes, hung low on the canopy.

use super::{TileDef, TileKind, dispatch};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::behavior::mob_hurt_tile;
use crate::entity::particle::new_smoke_particle;
use crate::entity::{Direction, Entity};
use crate::gfx::{Screen, Sprite, color};
use crate::item::{Item, ItemKind};
use crate::level::{drop_item, drop_items_counted};

/// Full hive (fresh chunks generate full for free — data bytes default to 0).
pub const DATA_FULL: i32 = 0;
/// Harvested; random ticks roll it back to full.
pub const DATA_REGROWING: i32 = 1;

pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Beehive);
    def.connects_to_grass = true;
    def.flammable = true; // it hangs in a tree
    def
}

pub fn render(g: &mut Game, screen: &mut Screen, _def: &TileDef, lvl: usize, x: i32, y: i32) {
    // the hive's tree renders exactly like the broadleaf (canopy connects to real
    // tree neighbors), then the hive lump hangs low on the trunk line
    let tree = g.tiles.get("tree");
    dispatch::render(g, screen, &tree, lvl, x, y);
    let full = g.level(lvl).get_data(x, y) == DATA_FULL;
    let col = if full {
        color::get4(-1, 100, 430, 540) // honey-gold comb over a dark-brown rind
    } else {
        color::get4(-1, 100, 221, 332) // spent gray-brown husk
    };
    Sprite::new1x1(10, 4, col).render(screen, x * 16 + 4, y * 16 + 7);
}

pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
    false
}

/// Regrowth: the berry-bush timer family, slower — bees take days to rebuild comb.
pub fn tick(g: &mut Game, _def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    if g.level(lvl).get_data(xt, yt) != DATA_FULL
        && g.random
            .next_int_bound(if crate::core::weather::growth_boost(g) {
                1500
            } else {
                3000
            })
            == 0
    {
        g.level_mut(lvl).set_data(xt, yt, DATA_FULL);
    }
}

fn drop_honeycomb(g: &mut Game, lvl: usize, x: i32, y: i32) {
    let comb = crate::item::registry::get(g, "Honeycomb");
    drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[comb]);
    g.level_mut(lvl).set_data(x, y, DATA_REGROWING);
    g.play_sound(Sound::MonsterHurt);
}

#[allow(clippy::too_many_arguments)]
pub fn hurt_by(
    g: &mut Game,
    def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
    source: &mut Entity,
    _dmg: i32,
    _attack_dir: Direction,
) -> bool {
    if g.level(lvl).get_data(x, y) == DATA_FULL {
        // the grab-and-run harvest: comb always, a sting sometimes
        drop_honeycomb(g, lvl, x, y);
        if source.is_player() && g.random.next_int_bound(3) == 0 {
            g.notifications.push("Bees!".to_string());
            mob_hurt_tile(g, source, def, x, y, 1);
        } else {
            g.notifications
                .push("You pull sticky honeycomb free.".to_string());
        }
    } else {
        // spent husk: one hit knocks it down, leaving the plain tree to chop
        let stick = crate::item::registry::get(g, "Stick");
        drop_item(g, lvl, x * 16 + 8, y * 16 + 8, stick);
        let tree = g.tiles.get("tree");
        g.set_tile_default(lvl, x, y, &tree);
        g.play_sound(Sound::MonsterHurt);
    }
    true
}

/// Torch interact: smoke the bees calm — a guaranteed, sting-free harvest. The calm
/// counter to the bare-handed gamble; any other item falls through to the attack
/// path (knocking the husk / harvesting with the sting risk).
#[allow(clippy::too_many_arguments)]
pub fn interact(
    g: &mut Game,
    _def: &TileDef,
    lvl: usize,
    xt: i32,
    yt: i32,
    _player: &mut Entity,
    item: &mut Item,
    _attack_dir: Direction,
) -> bool {
    if !matches!(item.kind, ItemKind::Torch { .. }) || g.level(lvl).get_data(xt, yt) != DATA_FULL {
        return false;
    }
    for _ in 0..3 {
        let jx = g.random.next_int_bound(9);
        let smoke = new_smoke_particle(xt * 16 + jx, yt * 16 + 6, false, &mut g.random);
        g.level_mut(lvl).add(smoke, lvl);
    }
    drop_honeycomb(g, lvl, xt, yt);
    g.notifications.push("You smoke the bees calm.".to_string());
    true
}
