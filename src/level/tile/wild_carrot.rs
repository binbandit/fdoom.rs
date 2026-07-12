//! Wild Carrot (farming wave): a lacy-topped root plant scattered through plains and
//! forest clearings — the foraged entry point into carrot farming. One hit pulls it:
//! the root itself plus seed stock for a tilled plot.

use super::{TileDef, TileKind, dispatch};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::{Direction, Entity};
use crate::gfx::{Screen, Sprite};
use crate::level::{drop_item, drop_items_counted};

pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::WildCarrot);
    def.connects_to_grass = true;
    def
}

pub fn render(g: &mut Game, screen: &mut Screen, _def: &TileDef, lvl: usize, x: i32, y: i32) {
    let grass = g.tiles.get("grass");
    dispatch::render(g, screen, &grass, lvl, x, y);
    let c = crate::assets::sprite_cell("tiles/wild_carrot");
    Sprite::new(c.x, c.y, 2, 2, 0, 0).render(screen, x * 16, y * 16);
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
    let carrot = crate::item::registry::get(g, "Carrot");
    drop_item(g, lvl, x * 16 + 8, y * 16 + 8, carrot);
    let seeds = crate::item::registry::get(g, "Carrot Seeds");
    drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[seeds]);
    let grass = g.tiles.get("grass");
    g.set_tile_default(lvl, x, y, &grass);
    g.play_sound(Sound::MonsterHurt);
    true
}
