//! Port of `fdoom.level.tile.FloorTile`.

use super::Material;
use super::{TileDef, TileKind, tool_use};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::{Sprite, color};
use crate::item::{Item, ToolType};

/// Java `FloorTile` constructor.
pub fn make(material: Material) -> TileDef {
    let name = match material {
        Material::Wood => "Wood Planks".to_string(),
        Material::Obsidian => "Obsidian".to_string(),
        _ => format!("{} Bricks", material.name()),
    };
    let mut def = TileDef::new(&name, TileKind::Floor { material });
    def.may_spawn = true;
    def.flammable = material == Material::Wood;
    let col = match material {
        Material::Wood => color::get4(210, 210, 430, 320),
        Material::Stone => color::get4(333, 333, 444, 444),
        Material::Obsidian => color::get4(102, 102, 203, 203),
    };
    def.sprite = Some(Sprite::new_onepixel(19, 2, 2, 2, col, 0, true));
    def
}

#[allow(clippy::too_many_arguments)]
pub fn interact(
    g: &mut Game,
    def: &TileDef,
    lvl: usize,
    xt: i32,
    yt: i32,
    player: &mut Entity,
    item: &mut Item,
    _attack_dir: Direction,
) -> bool {
    let TileKind::Floor { material } = def.kind else {
        return false;
    };
    if tool_use(g, player, item, ToolType::Pickaxe, 4).is_some() {
        let hole = g.tiles.get("hole");
        g.set_tile_default(lvl, xt, yt, &hole);
        let drop = match material {
            Material::Wood => crate::item::registry::get(g, "Plank"),
            _ => crate::item::registry::get(g, &format!("{} Brick", material.name())),
        };
        crate::level::drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, drop);
        g.play_sound(Sound::MonsterHurt);
        return true;
    }
    false
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
    true
}
