//! Dry Bush (sandbox era, no Java counterpart): a tumbleweed-style dead shrub scattered
//! through deserts and savannas. Walk-through; one bare-handed hit snaps it into 1-2
//! Sticks — the early desert stick source.
//!
//! It stands on whatever ground actually surrounds it (`tile::ground_beneath`), like
//! every other prop since the flora-base sweep. It used to hard-render a sand patch
//! everywhere: on a savanna meadow that tan disc — lit by the sand-side blend factor
//! and ringed by the seam carry it invited — read as a glowing neon-yellow ball
//! floating on the grass (ODDITIES O23). Breaking it restores sand when any orthogonal
//! neighbor is sandy, grass otherwise.

use super::{TileDef, TileKind, dispatch};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::{Direction, Entity};
use crate::gfx::{Screen, color};
use crate::item::ids as iname;
use crate::level::drop_items_counted;
use crate::level::tile::{ids, tile_id_at};

pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::DryBush);
    def.connects_to_sand = true;
    def.connects_to_grass = true;
    def.flammable = true; // tinder-dry by definition
    def
}

pub fn render(g: &mut Game, screen: &mut Screen, _def: &TileDef, lvl: usize, x: i32, y: i32) {
    // in Badlands clay country the parched patch is clay, not sand (content wave)
    let base = if super::clay::clay_country(g, lvl, x, y) {
        g.tiles.by_id(ids::LAYERED_CLAY)
    } else {
        g.tiles
            .by_id(super::ground_beneath(g, lvl, x, y, ids::SAND))
    };
    dispatch::render(g, screen, &base, lvl, x, y);
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
    let stick = crate::item::registry::by_name(g, iname::STICK);
    drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[stick]);
    // restore ground to match the surroundings (clay country wins over sand)
    let ground = if super::clay::clay_country(g, lvl, x, y) {
        g.tiles.by_id(ids::LAYERED_CLAY)
    } else {
        let sandy = [(0, -1), (0, 1), (-1, 0), (1, 0)]
            .iter()
            .any(|&(dx, dy)| tile_id_at(g, lvl, x + dx, y + dy) == ids::SAND);
        g.tiles.by_id(if sandy { ids::SAND } else { ids::GRASS })
    };
    g.set_tile_default(lvl, x, y, &ground);
    g.play_sound(Sound::MonsterHurt);
    true
}
