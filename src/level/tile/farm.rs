//! Port of `fdoom.level.tile.FarmTile`.

use super::{TileDef, TileKind, tool_use};
use crate::core::game::Game;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::{Sprite, color};
use crate::item::{Item, ToolType};

/// Java `FarmTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Farm);
    def.sprite = Some(Sprite::with_mirrors(
        2,
        1,
        2,
        2,
        color::get4(301, 411, 422, 533),
        true,
        &[vec![1, 0], vec![0, 1]],
    ));
    def
}

#[allow(clippy::too_many_arguments)]
pub fn interact(
    g: &mut Game,
    _def: &TileDef,
    lvl: usize,
    xt: i32,
    yt: i32,
    player: &mut Entity,
    item: &mut Item,
    _attack_dir: Direction,
) -> bool {
    if tool_use(g, player, item, ToolType::Shovel, 4).is_some() {
        let dirt = g.tiles.get("dirt");
        g.set_tile_default(lvl, xt, yt, &dirt);
        return true;
    }
    false
}

pub fn tick(g: &mut Game, _def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    let age = g.level(lvl).get_data(xt, yt);
    if age < 5 {
        g.level_mut(lvl).set_data(xt, yt, age + 1);
    }
}

pub fn stepped_on(g: &mut Game, _def: &TileDef, lvl: usize, xt: i32, yt: i32, _e: &mut Entity) {
    // rain waters the fields: crops advance twice as fast while it pours
    let odds = if crate::core::weather::growth_boost(g) {
        30
    } else {
        60
    };
    if g.random.next_int_bound(odds) != 0 {
        return;
    }
    if g.level(lvl).get_data(xt, yt) < 5 {
        return;
    }
    let dirt = g.tiles.get("dirt");
    g.set_tile_default(lvl, xt, yt, &dirt);
}
