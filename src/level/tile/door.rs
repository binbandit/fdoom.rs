//! Port of `fdoom.level.tile.DoorTile`.

use super::Material;
use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::{Screen, Sprite, color};
use crate::item::{Item, ItemKind, ToolType};

/// Java `closedSprite` with the per-material color applied.
fn closed_sprite(material: Material) -> Sprite {
    let col = match material {
        Material::Wood => color::get4(320, 430, 210, 430),
        Material::Stone => color::get4(444, 333, 222, 333),
        Material::Obsidian => color::get4(203, 102, 203, 102),
    };
    Sprite::new(2, 24, 2, 2, col, 0)
}

/// Java `openSprite` with the per-material color applied.
fn open_sprite(material: Material) -> Sprite {
    let col = match material {
        Material::Wood => color::get4(320, 430, 430, 210),
        Material::Stone => color::get4(444, 333, 333, 222),
        Material::Obsidian => color::get(203, 102),
    };
    Sprite::new(0, 24, 2, 2, col, 0)
}

/// Java `DoorTile` constructor.
pub fn make(material: Material) -> TileDef {
    let mut def = TileDef::new(
        &format!("{} Door", material.name()),
        TileKind::Door { material },
    );
    // Occludes light only while closed — `dispatch::blocks_light` gates this flag on
    // the per-tile open/closed data (0 = closed), same state `may_pass` reads.
    def.blocks_light = true;
    def.sprite = Some(closed_sprite(material));
    def
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let TileKind::Door { material } = def.kind else {
        return;
    };
    let closed = g.level(lvl).get_data(x, y) == 0;
    let cur_sprite = if closed {
        closed_sprite(material)
    } else {
        open_sprite(material)
    };
    cur_sprite.render(screen, x * 16, y * 16);
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
    let TileKind::Door { material } = def.kind else {
        return false;
    };
    if let ItemKind::Tool { ttype, level, .. } = item.kind {
        if ttype == ToolType::Pickaxe
            && crate::entity::mob::player_behavior::pay_stamina(player, 4 - level)
            && item.pay_durability(g.is_mode("creative"))
        {
            // JAVA: Tiles.get(id + 3) — will get the corresponding floor tile.
            let floor = g.tiles.get_id(def.id as i32 + 3);
            g.set_tile_default(lvl, xt, yt, &floor);
            let drop = crate::item::registry::get(g, &format!("{} Door", material.name()));
            crate::level::drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, drop);
            g.play_sound(Sound::MonsterHurt);
            return true;
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
pub fn hurt_by(
    g: &mut Game,
    _def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
    source: &mut Entity,
    _dmg: i32,
    _attack_dir: Direction,
) -> bool {
    if source.is_player() {
        let closed = g.level(lvl).get_data(x, y) == 0;
        g.level_mut(lvl).set_data(x, y, if closed { 1 } else { 0 });
    }
    false
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(g: &Game, _def: &TileDef, lvl: usize, x: i32, y: i32, _e: &Entity) -> bool {
    let closed = g.level(lvl).get_data(x, y) == 0;
    !closed
}
