//! Port of `fdoom.level.tile.FarmTile`. TODO(port:tile): full port pending.

use crate::core::game::Game;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::item::Item;
use super::dispatch;
use super::{TileDef, TileKind};

/// Java `FarmTile` constructor — sprite/config TODO(port:tile).
#[allow(unused_variables)]
pub fn make(name:&str) -> TileDef {
    TileDef::new(name, TileKind::Farm)
}

#[allow(clippy::too_many_arguments)]
pub fn interact(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32, player: &mut Entity, item: &mut Item, attack_dir: Direction) -> bool {
    let _ = (g, def, lvl, xt, yt, player, item, attack_dir); // TODO(port:tile)
    false
}

#[allow(clippy::too_many_arguments)]
pub fn stepped_on(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32, e: &mut Entity) {
    let _ = (g, def, lvl, xt, yt, e); // TODO(port:tile)
}

#[allow(clippy::too_many_arguments)]
pub fn tick(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    let _ = (g, def, lvl, xt, yt); // TODO(port:tile)
}
