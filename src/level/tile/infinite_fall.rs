//! Port of `fdoom.level.tile.InfiniteFallTile`.

use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::entity::{Entity, EntityKind};
use crate::gfx::Screen;

/// Java `InfiniteFallTile` constructor — `super(name, (Sprite)null)`.
pub fn make(name: &str) -> TileDef {
    TileDef::new(name, TileKind::InfiniteFall)
}

/// Java `render` — renders nothing.
#[allow(clippy::too_many_arguments)]
pub fn render(_g: &mut Game, _screen: &mut Screen, _def: &TileDef, _lvl: usize, _x: i32, _y: i32) {}

/// Java `tick` — does nothing.
pub fn tick(_g: &mut Game, _def: &TileDef, _lvl: usize, _xt: i32, _yt: i32) {}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, e: &Entity) -> bool {
    matches!(e.kind, EntityKind::AirWizard(_)) || e.is_player() && e.player().skinon
}
