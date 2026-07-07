//! Mushroom (sandbox era, no Java counterpart): a walk-through fungus cluster scattered
//! on forest floors and mine cave floors. One hit picks it (drops a Mushroom).
//!
//! Its ground follows the level: grass on the surface, dirt underground — so the one
//! tile id serves both spawns.

use super::{TileDef, TileKind, dispatch};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::{Direction, Entity};
use crate::gfx::{Screen, Sprite, color};
use crate::level::drop_item;

/// Cap cluster. TODO(art): final cells — reuses the ore-nub cell (17,1) recolored
/// red/white for now (shade 0 = ground, transparent here).
fn caps() -> Sprite {
    Sprite::new(17, 1, 2, 2, color::get4(-1, 300, 500, 554), 0)
}

fn base_name(g: &Game, lvl: usize) -> &'static str {
    if g.level(lvl).depth < 0 {
        "dirt"
    } else {
        "grass"
    }
}

pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Mushroom);
    def.connects_to_grass = true;
    def
}

pub fn render(g: &mut Game, screen: &mut Screen, _def: &TileDef, lvl: usize, x: i32, y: i32) {
    let base = g.tiles.get(base_name(g, lvl));
    dispatch::render(g, screen, &base, lvl, x, y);
    caps().render(screen, x * 16, y * 16);
}

#[allow(clippy::too_many_arguments)]
pub fn hurt_by(
    g: &mut Game,
    _def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
    _source: &mut Entity,
    _dmg: i32,
    _attack_dir: Direction,
) -> bool {
    let mushroom = crate::item::registry::get(g, "Mushroom");
    drop_item(g, lvl, x * 16 + 8, y * 16 + 8, mushroom);
    let base = g.tiles.get(base_name(g, lvl));
    g.set_tile_default(lvl, x, y, &base);
    g.play_sound(Sound::MonsterHurt);
    true
}
