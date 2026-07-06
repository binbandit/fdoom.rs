//! Port of `fdoom.level.tile.StairsTile`. TODO(port:tile): full port pending.

use crate::core::game::Game;
use crate::gfx::Screen;
use super::dispatch;
use super::{TileDef, TileKind};

/// Java `StairsTile` constructor — sprite/config TODO(port:tile).
#[allow(unused_variables)]
pub fn make(name:&str, leads_up:bool) -> TileDef {
    TileDef::new(name, TileKind::Stairs { leads_up })
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    dispatch::default_render(g, screen, def, lvl, x, y); // TODO(port:tile)
}
