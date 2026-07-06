//! Port of `fdoom.level.tile.PumpkinTile`.

use super::dispatch;
use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::entity::Entity;
use crate::gfx::{Screen, Sprite, color};

/// Java `PumpkinTile` constructor. (`isJacko` is stored but never read in Java.)
pub fn make(name: &str, _lit: bool) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Pumpkin);
    def.sprite = Some(Sprite::new(22, 8, 2, 2, color::get4(-1, 210, 530, 550), 0));
    def.connects_to_grass = true;
    def
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let grass = g.tiles.get("grass");
    dispatch::render(g, screen, &grass, lvl, x, y);
    if let Some(sprite) = &def.sprite {
        sprite.render_color(screen, x * 16, y * 16, color::get4(-1, 210, 530, 550));
    }
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
    false
}

pub fn get_light_radius(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32) -> i32 {
    3
}
