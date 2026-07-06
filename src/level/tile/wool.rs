//! Port of `fdoom.level.tile.WoolTile`. TODO(port:tile): full port pending.

use crate::core::game::Game;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::Screen;
use crate::item::Item;
use super::dispatch;
use super::{TileDef, TileKind};

/// Java `WoolTile` constructor — sprite/config TODO(port:tile).
#[allow(unused_variables)]
pub fn make() -> TileDef {
    TileDef::new("Wool", TileKind::Wool)
}

#[allow(clippy::too_many_arguments)]
pub fn interact(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32, player: &mut Entity, item: &mut Item, attack_dir: Direction) -> bool {
    let _ = (g, def, lvl, xt, yt, player, item, attack_dir); // TODO(port:tile)
    false
}

#[allow(clippy::too_many_arguments)]
pub fn matches(def: &TileDef, this_data: i32, tile_info: &str) -> bool {
    let _ = this_data; // TODO(port:tile)
    def.name == tile_info.split('_').next().unwrap_or("")
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(g: &Game, def: &TileDef, lvl: usize, x: i32, y: i32, e: &Entity) -> bool {
    let _ = (g, def, lvl, x, y, e); // TODO(port:tile)
    true
}

#[allow(clippy::too_many_arguments)]
pub fn get_data_str(def: &TileDef, data: &str) -> i32 {
    let _ = def; // TODO(port:tile)
    data.parse().unwrap_or(0)
}

#[allow(clippy::too_many_arguments)]
pub fn get_name(def: &TileDef, data: i32) -> String {
    let _ = data; // TODO(port:tile)
    def.name.clone()
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    dispatch::default_render(g, screen, def, lvl, x, y); // TODO(port:tile)
}
