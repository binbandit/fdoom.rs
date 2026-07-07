//! Berry Bush (sandbox era, no Java counterpart): a low shrub on grass that carries
//! ripe berries. Hitting a ripe bush picks the berries (the bush survives and regrows
//! them over a few in-game days via random ticks, same cadence family as tall grass);
//! hitting a bare bush tears it out.
//!
//! Per-tile data byte: 0 = ripe (freshly generated chunks are ripe for free, since
//! chunk data bytes default to 0), 1 = picked/regrowing. Art: dedicated ripe/picked
//! blocks from artgen `flora_cells` ((15,26)/(17,26)).

use super::{TileDef, TileKind, dispatch};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::{Direction, Entity};
use crate::gfx::{Screen, Sprite};
use crate::level::{drop_item, drop_items_counted};

/// Ripe when the data byte is 0 (see module docs).
pub const DATA_RIPE: i32 = 0;
/// Picked; random ticks roll it back to ripe.
pub const DATA_REGROWING: i32 = 1;

/// Ripe bush (artgen `flora_cells` (15,26)): green shrub studded with red berries.
fn bush_ripe() -> Sprite {
    Sprite::new(15, 26, 2, 2, 0, 0)
}

/// Picked bush (artgen `flora_cells` (17,26)): the same shrub, clearly bare.
fn bush_picked() -> Sprite {
    Sprite::new(17, 26, 2, 2, 0, 0)
}

pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::BerryBush);
    def.connects_to_grass = true;
    def
}

pub fn render(g: &mut Game, screen: &mut Screen, _def: &TileDef, lvl: usize, x: i32, y: i32) {
    let grass = g.tiles.get("grass");
    dispatch::render(g, screen, &grass, lvl, x, y);
    if g.level(lvl).get_data(x, y) == DATA_RIPE {
        bush_ripe().render(screen, x * 16, y * 16);
    } else {
        bush_picked().render(screen, x * 16, y * 16);
    }
}

pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
    false
}

/// Regrowth: same odds family as tall grass growth (`next_int_bound(2000)` per random
/// tick ≈ a few in-game days per stage).
pub fn tick(g: &mut Game, _def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    if g.level(lvl).get_data(xt, yt) != DATA_RIPE && g.random.next_int_bound(2000) == 0 {
        g.level_mut(lvl).set_data(xt, yt, DATA_RIPE);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn hurt_by(
    g: &mut Game,
    _def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
    _source: &mut Entity,
    _dmg: i32,
    _attack_dir: Direction,
) -> bool {
    g.play_sound(Sound::MonsterHurt);
    if g.level(lvl).get_data(x, y) == DATA_RIPE {
        // pick: berries off, bush stays
        let berry = crate::item::registry::get(g, "Berry");
        drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[berry]);
        g.level_mut(lvl).set_data(x, y, DATA_REGROWING);
    } else {
        // bare bush: tear it out
        let stick = crate::item::registry::get(g, "Stick");
        drop_item(g, lvl, x * 16 + 8, y * 16 + 8, stick);
        let grass = g.tiles.get("grass");
        g.set_tile_default(lvl, x, y, &grass);
    }
    true
}
