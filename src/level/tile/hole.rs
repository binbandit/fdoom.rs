//! Port of `fdoom.level.tile.HoleTile`. TODO(port:tile): full port pending.

use crate::core::game::Game;
use crate::entity::Entity;
use crate::gfx::Screen;
use super::dispatch;
use super::{TileDef, TileKind};

/// Java `HoleTile` constructor — sprite/config TODO(port:tile).
#[allow(unused_variables)]
pub fn make(name:&str) -> TileDef {
    TileDef::new(name, TileKind::Hole)
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
pub fn get_sparse_color(def: &TileDef, tile: &TileDef, orig_col: i32) -> i32 {
    let _ = (def, tile); // TODO(port:tile)
    orig_col
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    dispatch::default_render(g, screen, def, lvl, x, y); // TODO(port:tile)
}
