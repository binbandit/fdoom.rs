//! Port of `fdoom.level.tile.ExplodedTile`. TODO(port:tile): full port pending.

use crate::core::game::Game;
use crate::entity::Entity;
use super::dispatch;
use super::{TileDef, TileKind};

/// Java `ExplodedTile` constructor — sprite/config TODO(port:tile).
#[allow(unused_variables)]
pub fn make(name:&str) -> TileDef {
    TileDef::new(name, TileKind::Exploded)
}

#[allow(clippy::too_many_arguments)]
pub fn connects_to(def: &TileDef, other: &TileDef, is_side: bool) -> bool {
    let _ = is_side; // TODO(port:tile)
    dispatch::same_class(def, other)
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(g: &Game, def: &TileDef, lvl: usize, x: i32, y: i32, e: &Entity) -> bool {
    let _ = (g, def, lvl, x, y, e); // TODO(port:tile)
    true
}

#[allow(clippy::too_many_arguments)]
pub fn stepped_on(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32, e: &mut Entity) {
    let _ = (g, def, lvl, xt, yt, e); // TODO(port:tile)
}
