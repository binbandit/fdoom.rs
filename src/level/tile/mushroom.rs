//! Mushroom (sandbox era, no Java counterpart): a walk-through fungus cluster scattered
//! on forest floors and mine cave floors. One hit picks it (drops a Mushroom).
//!
//! Its ground follows the level: grass on the surface, dirt underground — so the one
//! tile id serves both spawns.

use super::{TileDef, TileKind, dispatch};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::{Direction, Entity};
use crate::gfx::{Screen, Sprite};
use crate::level::drop_item;

/// Dedicated cluster art (`tiles/mushroom_cluster`): five tiny button caps, three
/// huddled low, two strays upper-right — true color, the palette is ignored.
/// `mirror` (0/1) flips each quadrant for cheap per-tile variety; caps stay inside
/// their 8x8 cells, so per-cell flipping never tears one apart.
fn caps(mirror: i32) -> Sprite {
    let c = crate::assets::sprite_cell("tiles/mushroom_cluster");
    Sprite::new(c.x, c.y, 2, 2, 0, mirror)
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
    let mirror = (crate::level::infinite_gen::hash(g.world_seed, 0x5348_5230, x, y) & 1) as i32;
    caps(mirror).render(screen, x * 16, y * 16);
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
