//! Port of `fdoom.level.tile.TallGrassTile`.

use super::dispatch;
use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::{Screen, Sprite, color};

/// Java static `small` sprite.
fn small() -> Sprite {
    Sprite::new(28, 8, 2, 2, color::get4(-1, 30, 40, -1), 0)
}

/// Java static `medium` sprite.
fn medium() -> Sprite {
    Sprite::new(30, 8, 2, 2, color::get4(-1, 30, 40, -1), 0)
}

/// Java static `tall` sprite.
fn tall() -> Sprite {
    Sprite::new(26, 8, 2, 2, color::get4(-1, 30, 40, -1), 0)
}

/// Java `TallGrassTile` constructor. `on_tile` is always "grass" in this fork.
pub fn make(name: &str, on_tile: &str, kind: i32) -> TileDef {
    let _ = on_tile;
    let mut def = TileDef::new(name, TileKind::TallGrass { kind });
    def.sprite = Some(small());
    // JAVA: connect flags are copied from onType (GrassTile: connectsToGrass only).
    def.connects_to_grass = true;
    def.may_spawn = true;
    def
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let TileKind::TallGrass { kind } = def.kind else {
        return;
    };
    // JAVA: onType.render — onType is always Tiles.get("grass").
    let on_type = g.tiles.get("grass");
    dispatch::render(g, screen, &on_type, lvl, x, y);
    match kind {
        0 => small().render(screen, x * 16, y * 16),
        1 => medium().render(screen, x * 16, y * 16),
        2 => tall().render_color(screen, x * 16, y * 16, color::get4(-1, 210, 530, 550)),
        _ => {}
    }
}

pub fn tick(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    let TileKind::TallGrass { kind } = def.kind else {
        return;
    };
    if kind < 2 && g.random.next_int_bound(10) == 4 {
        let next = match kind {
            0 => g.tiles.get_id(40),
            _ => g.tiles.get_id(41),
        };
        g.set_tile_default(lvl, xt, yt, &next);
    }
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
    let grass = g.tiles.get("grass");
    g.set_tile_default(lvl, x, y, &grass);
    g.play_sound(Sound::MonsterHurt);
    if g.random.next_int_bound(4) == 0 {
        // JAVA: dropItem(x, y, count, item) — exactly 1 stone.
        let stone = crate::item::registry::get(g, "Stone");
        crate::level::drop_item(g, lvl, x * 16 + 8, y * 16 + 8, stone);
    }
    // JAVA: dropItem(x, y, count, item) — exactly 2 grass fibers.
    let fibers = crate::item::registry::get(g, "grass fibers");
    for _ in 0..2 {
        crate::level::drop_item(g, lvl, x * 16 + 8, y * 16 + 8, fibers.clone());
    }

    true
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(_g: &Game, def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
    let TileKind::TallGrass { kind } = def.kind else {
        return true;
    };
    kind != 2
}
