//! Port of `fdoom.level.tile.FloorTile`. TODO(port:tile): full port pending.

use crate::core::game::Game;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::item::Item;
use super::dispatch;
use super::Material;
use super::{TileDef, TileKind};

/// Java `FloorTile` constructor — sprite/config TODO(port:tile).
#[allow(unused_variables)]
pub fn make(material:Material) -> TileDef {
    TileDef::new(&format!("{} Planks", material.name()), TileKind::Floor { material })
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
