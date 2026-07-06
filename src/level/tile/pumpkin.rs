//! Port of `fdoom.level.tile.PumpkinTile`. TODO(port:tile): full port pending.

use crate::core::game::Game;
use crate::entity::Entity;
use crate::gfx::Screen;
use super::dispatch;
use super::{TileDef, TileKind};

/// Java `PumpkinTile` constructor — sprite/config TODO(port:tile).
#[allow(unused_variables)]
pub fn make(name:&str, _lit:bool) -> TileDef {
    TileDef::new(name, TileKind::Pumpkin)
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(g: &Game, def: &TileDef, lvl: usize, x: i32, y: i32, e: &Entity) -> bool {
    let _ = (g, def, lvl, x, y, e); // TODO(port:tile)
    true
}

#[allow(clippy::too_many_arguments)]
pub fn get_light_radius(g: &Game, def: &TileDef, lvl: usize, x: i32, y: i32) -> i32 {
    let _ = (g, def, lvl, x, y); // TODO(port:tile)
    0
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    dispatch::default_render(g, screen, def, lvl, x, y); // TODO(port:tile)
}
