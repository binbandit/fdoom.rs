//! Port of `fdoom.level.tile.SnowTile`. TODO(port:tile): full port pending.

use crate::core::game::Game;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::Screen;
use crate::item::Item;
use super::dispatch;
use super::{TileDef, TileKind};

/// Java `SnowTile` constructor — sprite/config TODO(port:tile).
#[allow(unused_variables)]
pub fn make(name:&str) -> TileDef {
    TileDef::new(name, TileKind::Snow)
}

#[allow(clippy::too_many_arguments)]
pub fn connects_to(def: &TileDef, other: &TileDef, is_side: bool) -> bool {
    let _ = is_side; // TODO(port:tile)
    dispatch::same_class(def, other)
}

#[allow(clippy::too_many_arguments)]
pub fn interact(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32, player: &mut Entity, item: &mut Item, attack_dir: Direction) -> bool {
    let _ = (g, def, lvl, xt, yt, player, item, attack_dir); // TODO(port:tile)
    false
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    dispatch::default_render(g, screen, def, lvl, x, y); // TODO(port:tile)
}

#[allow(clippy::too_many_arguments)]
pub fn stepped_on(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32, e: &mut Entity) {
    let _ = (g, def, lvl, xt, yt, e); // TODO(port:tile)
}

#[allow(clippy::too_many_arguments)]
pub fn tick(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    let _ = (g, def, lvl, xt, yt); // TODO(port:tile)
}
