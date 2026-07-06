//! Port of `fdoom.level.tile.FlowerTile`. TODO(port:tile): full port pending.

use crate::core::game::Game;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::Screen;
use crate::item::Item;
use super::dispatch;
use super::{TileDef, TileKind};

/// Java `FlowerTile` constructor — sprite/config TODO(port:tile).
#[allow(unused_variables)]
pub fn make(name:&str) -> TileDef {
    TileDef::new(name, TileKind::Flower)
}

#[allow(clippy::too_many_arguments)]
pub fn hurt_by(g: &mut Game, def: &TileDef, lvl: usize, x: i32, y: i32, source: &mut Entity, dmg: i32, attack_dir: Direction) -> bool {
    let _ = (g, def, lvl, x, y, source, dmg, attack_dir); // TODO(port:tile)
    false
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
