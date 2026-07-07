//! Port of `fdoom.level.tile.SaplingTile`.

use super::{TileDef, TileKind, dispatch};
use crate::core::game::Game;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::{Screen, Sprite, color};

/// Java `SaplingTile` constructor.
pub fn make(name: &str, on_type: &str, grows_to: &str) -> TileDef {
    let mut def = TileDef::new(
        name,
        TileKind::Sapling {
            on_type: on_type.to_string(),
            grows_to: grows_to.to_string(),
        },
    );
    def.sprite = Some(Sprite::new1x1(11, 3, color::get4(20, 40, 50, -1)));
    // Mirror the connects-to flags of the ground the sapling stands on; resolved
    // statically because make() runs while the registry is still being built (on_type
    // is only ever "Grass" or "Sand").
    match on_type.to_uppercase().as_str() {
        "GRASS" => def.connects_to_grass = true,
        "SAND" => def.connects_to_sand = true,
        _ => {}
    }
    def.may_spawn = true;
    def
}

pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let TileKind::Sapling { on_type, .. } = &def.kind else {
        return;
    };
    let on_def = g.tiles.get(on_type);
    dispatch::render(g, screen, &on_def, lvl, x, y);

    if let Some(sprite) = &def.sprite {
        sprite.render(screen, x * 16, y * 16);
    }
}

pub fn tick(g: &mut Game, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let TileKind::Sapling { grows_to, .. } = &def.kind else {
        return;
    };
    let age = g.level(lvl).get_data(x, y) + 1;
    if age > 100 {
        let grows_to = g.tiles.get(grows_to);
        g.set_tile_default(lvl, x, y, &grows_to);
    } else {
        g.level_mut(lvl).set_data(x, y, age);
    }
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
    let TileKind::Sapling { on_type, .. } = &def.kind else {
        return false;
    };
    let on_def = g.tiles.get(on_type);
    g.set_tile_default(lvl, x, y, &on_def);
    true
}
