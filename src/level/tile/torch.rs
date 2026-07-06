//! Port of `fdoom.level.tile.TorchTile`. (`getTorchTile` lives on the registry as
//! `Tiles::get_torch_tile`.)

use super::dispatch;
use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::{Screen, color};
use crate::item::{Item, ItemKind};

/// Java `TorchTile` (private) constructor.
pub fn make(on: &TileDef) -> TileDef {
    let mut def = TileDef::new(
        &format!("Torch {}", on.name),
        TileKind::Torch {
            on_type: on.name.clone(),
        },
    );
    def.sprite = Some(crate::gfx::Sprite::new1x1(
        12,
        3,
        color::get4(320, 500, 520, -1),
    ));
    // JAVA: connectsToSnow is not copied.
    def.connects_to_sand = on.connects_to_sand;
    def.connects_to_grass = on.connects_to_grass;
    def.connects_to_water = on.connects_to_water;
    def.connects_to_lava = on.connects_to_lava;
    def
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let TileKind::Torch { on_type } = &def.kind else {
        return;
    };
    let on = g.tiles.get(on_type);
    dispatch::render(g, screen, &on, lvl, x, y);
    if let Some(sprite) = &def.sprite {
        sprite.render(screen, x * 16 + 4, y * 16 + 4);
    }
}

pub fn get_light_radius(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32) -> i32 {
    4
}

#[allow(clippy::too_many_arguments)]
pub fn interact(
    g: &mut Game,
    def: &TileDef,
    lvl: usize,
    xt: i32,
    yt: i32,
    _player: &mut Entity,
    item: &mut Item,
    _attack_dir: Direction,
) -> bool {
    let TileKind::Torch { on_type } = &def.kind else {
        return false;
    };
    if matches!(item.kind, ItemKind::PowerGlove) {
        let on = g.tiles.get(on_type);
        g.set_tile_default(lvl, xt, yt, &on);
        let torch = crate::item::registry::get(g, "Torch");
        crate::level::drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, torch);
        true
    } else {
        false
    }
}
