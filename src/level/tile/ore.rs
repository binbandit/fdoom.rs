//! Port of `fdoom.level.tile.OreTile`. TODO(port:tile): full port pending.

use crate::core::game::Game;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::Screen;
use crate::item::Item;
use super::dispatch;
use super::OreType;
use super::{TileDef, TileKind};

/// Java `OreTile` constructor — sprite/config TODO(port:tile).
#[allow(unused_variables)]
pub fn make(ore_type:OreType) -> TileDef {
    TileDef::new(&format!("{:?} Ore", ore_type), TileKind::Ore { ore_type })
}

#[allow(clippy::too_many_arguments)]
pub fn hurt_by(g: &mut Game, def: &TileDef, lvl: usize, x: i32, y: i32, source: &mut Entity, dmg: i32, attack_dir: Direction) -> bool {
    let _ = (g, def, lvl, x, y, source, dmg, attack_dir); // TODO(port:tile)
    false
}

#[allow(clippy::too_many_arguments)]
pub fn hurt_dmg(g: &mut Game, def: &TileDef, lvl: usize, x: i32, y: i32, dmg: i32) {
    let _ = (g, def, lvl, x, y, dmg); // TODO(port:tile)
}

#[allow(clippy::too_many_arguments)]
pub fn interact(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32, player: &mut Entity, item: &mut Item, attack_dir: Direction) -> bool {
    let _ = (g, def, lvl, xt, yt, player, item, attack_dir); // TODO(port:tile)
    false
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(g: &Game, def: &TileDef, lvl: usize, x: i32, y: i32, e: &Entity) -> bool {
    let _ = (g, def, lvl, x, y, e); // TODO(port:tile)
    true
}

#[allow(clippy::too_many_arguments)]
pub fn bumped_into(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32, e: &mut Entity) {
    let _ = (g, def, lvl, xt, yt, e); // TODO(port:tile)
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    dispatch::default_render(g, screen, def, lvl, x, y); // TODO(port:tile)
}
