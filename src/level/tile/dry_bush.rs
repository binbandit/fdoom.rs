//! Dry Bush (sandbox era, no Java counterpart): a tumbleweed-style dead shrub scattered
//! through deserts and savannas. Walk-through; one bare-handed hit snaps it into 1-2
//! Sticks — the early desert stick source.
//!
//! It renders a parched sand patch under itself (deliberate in savanna too, where the
//! ring of dry ground reads as the bush having killed the grass). Breaking it restores
//! sand when any orthogonal neighbor is sandy, grass otherwise.

use super::{TileDef, TileKind, dispatch};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::{Direction, Entity};
use crate::gfx::{Screen, color};
use crate::level::drop_items_counted;

pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::DryBush);
    def.connects_to_sand = true;
    def.connects_to_grass = true;
    def.flammable = true; // tinder-dry by definition
    def
}

pub fn render(g: &mut Game, screen: &mut Screen, _def: &TileDef, lvl: usize, x: i32, y: i32) {
    let sand = g.tiles.get("sand");
    dispatch::render(g, screen, &sand, lvl, x, y);
    // Dedicated tumbleweed skeleton (artgen `flora_cells` (17,28)) — true color, the
    // palette is ignored.
    let col = color::get4(-1, -1, 321, 210);
    screen.render(x * 16, y * 16, 17 + 28 * 32, col, 0);
    screen.render(x * 16 + 8, y * 16, 18 + 28 * 32, col, 0);
    screen.render(x * 16, y * 16 + 8, 17 + 29 * 32, col, 0);
    screen.render(x * 16 + 8, y * 16 + 8, 18 + 29 * 32, col, 0);
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
    let stick = crate::item::registry::get(g, "Stick");
    drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[stick]);
    // restore ground to match the surroundings
    let sandy = [(0, -1), (0, 1), (-1, 0), (1, 0)]
        .iter()
        .any(|&(dx, dy)| g.tile_at(lvl, x + dx, y + dy).name == "SAND");
    let ground = g.tiles.get(if sandy { "sand" } else { "grass" });
    g.set_tile_default(lvl, x, y, &ground);
    g.play_sound(Sound::MonsterHurt);
    true
}
