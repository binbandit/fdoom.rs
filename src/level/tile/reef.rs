//! Shallow-water ocean life (sandbox era, no Java counterpart): Seaweed and Coral.
//! Both render over the animated water art, pass like water (swimmers only), and break
//! into a resource: seaweed → Grass Fibers, coral → Stone (calcified skeleton — the
//! sanest existing material for it).

use super::{TileDef, TileKind, dispatch};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::behavior::can_swim;
use crate::entity::{Direction, Entity};
use crate::gfx::{Screen, Sprite, color};
use crate::level::drop_items_counted;

/// Swaying fronds. TODO(art): final cells — reuses the tall-grass cell (26,8) over
/// water for now.
fn fronds() -> Sprite {
    Sprite::new(26, 8, 2, 2, color::get4(-1, 20, 30, 41), 0)
}

/// Coral heads. TODO(art): final cells — reuses the ore-nub cell (17,1) recolored
/// pink/orange for now (shade 0 = ground, transparent here).
fn heads() -> Sprite {
    Sprite::new(17, 1, 2, 2, color::get4(-1, 410, 520, 531), 0)
}

fn make_base(name: &str, kind: TileKind) -> TileDef {
    let mut def = TileDef::new(name, kind);
    // blend into surrounding water and beach sand exactly like the water tile
    def.connects_to_water = true;
    def.connects_to_sand = true;
    def
}

pub fn make_seaweed(name: &str) -> TileDef {
    make_base(name, TileKind::Seaweed)
}

pub fn make_coral(name: &str) -> TileDef {
    make_base(name, TileKind::Coral)
}

pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let water = g.tiles.get("water");
    dispatch::render(g, screen, &water, lvl, x, y);
    match def.kind {
        TileKind::Coral => heads().render(screen, x * 16, y * 16),
        _ => fronds().render(screen, x * 16, y * 16),
    }
}

pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, e: &Entity) -> bool {
    can_swim(e)
}

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
    match def.kind {
        TileKind::Coral => {
            let stone = crate::item::registry::get(g, "Stone");
            drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[stone]);
        }
        _ => {
            let fibers = crate::item::registry::get(g, "Grass Fibers");
            drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[fibers]);
        }
    }
    let water = g.tiles.get("water");
    g.set_tile_default(lvl, x, y, &water);
    g.play_sound(Sound::MonsterHurt);
    true
}
