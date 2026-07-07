//! Port of `fdoom.level.tile.InfiniteFallTile`.

use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::entity::Entity;
use crate::gfx::Screen;

/// Java `InfiniteFallTile` constructor — `super(name, (Sprite)null)`.
pub fn make(name: &str) -> TileDef {
    TileDef::new(name, TileKind::InfiniteFall)
}

/// The void renders as pure black — no sprite at all.
#[allow(clippy::too_many_arguments)]
pub fn render(_g: &mut Game, _screen: &mut Screen, _def: &TileDef, _lvl: usize, _x: i32, _y: i32) {}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, e: &Entity) -> bool {
    // Only a player wearing the skin suit can step out over the void; flying kinds
    // are exempted globally in `dispatch::may_pass`.
    e.is_player() && e.player().skinon
}
