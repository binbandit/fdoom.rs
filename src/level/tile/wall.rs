//! Port of `fdoom.level.tile.WallTile`. TODO(port:tile): full port pending.

use crate::core::game::Game;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::item::Item;
use super::dispatch;
use super::Material;
use super::{TileDef, TileKind};

/// Java `WallTile` constructor — sprite/config TODO(port:tile).
#[allow(unused_variables)]
pub fn make(material:Material) -> TileDef {
    TileDef::new(&format!("{} Wall", material.name()), TileKind::Wall { material })
}

#[allow(clippy::too_many_arguments)]
pub fn hurt_by(g: &mut Game, def: &TileDef, lvl: usize, x: i32, y: i32, source: &mut Entity, dmg: i32, attack_dir: Direction) -> bool {
    let _ = (g, def, lvl, x, y, source, dmg, attack_dir); // TODO(port:tile)
    false
}

#[allow(clippy::too_many_arguments)]
pub fn hurt_dmg(g: &mut Game, def: &TileDef, lvl: usize, x: i32, y: i32, dmg: i32) {
    let _ = (g, def, lvl, x, y, dmg); // TODO(port:tile)
}

#[allow(clippy::too_many_arguments)]
pub fn interact(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32, player: &mut Entity, item: &mut Item, attack_dir: Direction) -> bool {
    let _ = (g, def, lvl, xt, yt, player, item, attack_dir); // TODO(port:tile)
    false
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(g: &Game, def: &TileDef, lvl: usize, x: i32, y: i32, e: &Entity) -> bool {
    let _ = (g, def, lvl, x, y, e); // TODO(port:tile)
    true
}

#[allow(clippy::too_many_arguments)]
pub fn tick(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    let _ = (g, def, lvl, xt, yt); // TODO(port:tile)
}

#[allow(clippy::too_many_arguments)]
pub fn get_name(def: &TileDef, data: i32) -> String {
    let _ = data; // TODO(port:tile)
    def.name.clone()
}
