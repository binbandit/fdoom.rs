//! Port of `fdoom.level.tile.TorchTile`. TODO(port:tile): full port pending.

use crate::core::game::Game;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::Screen;
use crate::item::Item;
use super::dispatch;
use super::{TileDef, TileKind};

/// Java `TorchTile` constructor — sprite/config TODO(port:tile).
#[allow(unused_variables)]
pub fn make(on:&TileDef) -> TileDef {
    TileDef::new(&format!("Torch {}", on.name), TileKind::Torch { on_type: on.name.clone() })
}

#[allow(clippy::too_many_arguments)]
pub fn interact(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32, player: &mut Entity, item: &mut Item, attack_dir: Direction) -> bool {
    let _ = (g, def, lvl, xt, yt, player, item, attack_dir); // TODO(port:tile)
    false
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
