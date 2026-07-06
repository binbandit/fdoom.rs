//! Port of `fdoom.level.tile.LavaBrickTile`.

use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::entity::behavior::{can_wool, mob_hurt_tile};
use crate::entity::mob::player_behavior::pay_stamina;
use crate::gfx::{Sprite, color};
use crate::item::{Item, ItemKind, ToolType};

/// Java `LavaBrickTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::LavaBrick);
    def.sprite = Some(Sprite::new(19, 2, 2, 2, color::get4(300, 300, 400, 400), 0));
    def
}

#[allow(clippy::too_many_arguments)]
pub fn interact(
    g: &mut Game,
    _def: &TileDef,
    lvl: usize,
    xt: i32,
    yt: i32,
    player: &mut Entity,
    item: &mut Item,
    _attack_dir: Direction,
) -> bool {
    if let ItemKind::Tool {
        ttype,
        level: tool_level,
        ..
    } = &item.kind
    {
        let (ttype, tool_level) = (*ttype, *tool_level);
        if ttype == ToolType::Pickaxe
            && pay_stamina(player, 4 - tool_level)
            && item.pay_durability(g.is_mode("creative"))
        {
            let lava = g.tiles.get("lava");
            g.set_tile_default(lvl, xt, yt, &lava);
            g.play_sound(Sound::MonsterHurt);
            return true;
        }
    }
    false
}

pub fn bumped_into(g: &mut Game, def: &TileDef, lvl: usize, x: i32, y: i32, e: &mut Entity) {
    let _ = lvl;
    if e.mob().is_some() {
        mob_hurt_tile(g, e, def, x, y, 3);
    }
}

pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, e: &Entity) -> bool {
    can_wool(e)
}
