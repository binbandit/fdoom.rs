//! Port of `fdoom.level.tile.PumpkinTile`, extended post-port: the `lit` flag (Java's
//! never-read `isJacko`) now actually registers the Jack-O-Lantern — a carved pumpkin
//! that casts real light and drops its own item.

use super::dispatch;
use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::entity::{Direction, Entity};
use crate::gfx::{Screen, Sprite, color};
use crate::level::drop_item;

/// Java `PumpkinTile` constructor; `lit = true` is the Jack-O-Lantern.
pub fn make(name: &str, lit: bool) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Pumpkin { lit });
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
    // TODO(art): final cells — the Jack-O-Lantern needs a carved-face variant of the
    // pumpkin cells (22,8); until then it renders as a plain pumpkin (its light radius
    // is what currently tells them apart at night).
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
    false
}

pub fn get_light_radius(_g: &Game, def: &TileDef, _lvl: usize, _x: i32, _y: i32) -> i32 {
    match def.kind {
        TileKind::Pumpkin { lit: true } => 7,
        _ => 3,
    }
}

/// Smashing a pumpkin yields its item (Pumpkin, or Jack-O-Lantern when lit).
#[allow(clippy::too_many_arguments)]
pub fn hurt_by(
    g: &mut Game,
    def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
    _source: &mut Entity,
    _dmg: i32,
    _attack_dir: Direction,
) -> bool {
    let item_name = match def.kind {
        TileKind::Pumpkin { lit: true } => "Jack-O-Lantern",
        _ => "Pumpkin",
    };
    let item = crate::item::registry::get(g, item_name);
    drop_item(g, lvl, x * 16 + 8, y * 16 + 8, item);
    let grass = g.tiles.get("grass");
    g.set_tile_default(lvl, x, y, &grass);
    g.play_sound(crate::core::io::sound::Sound::MonsterHurt);
    true
}
