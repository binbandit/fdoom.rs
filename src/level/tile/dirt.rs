//! Port of `fdoom.level.tile.DirtTile`.

use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::entity::mob::player_behavior::pay_stamina;
use crate::gfx::{Screen, Sprite, color};
use crate::item::{Item, ItemKind, ToolType};
use crate::level::drop_item;

/// Java `DirtTile.dCol(depth)` — the readable dirt color for a level depth.
pub fn d_col(depth: i32) -> i32 {
    match depth {
        1 => 444,  // sky.
        0 => 321,  // surface.
        -4 => 203, // dungeons.
        _ => 222,  // caves.
    }
}

/// Java `DirtTile.getColor(depth)`.
fn get_color(depth: i32) -> i32 {
    let dcol = d_col(depth);
    color::get4(dcol, dcol, dcol - 111, dcol - 111)
}

/// Java `DirtTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Dirt);
    def.sprite = Some(Sprite::dots(get_color(0)));
    def.may_spawn = true;
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
        if ttype == ToolType::Shovel
            && pay_stamina(player, 4 - tool_level)
            && item.pay_durability(g.is_mode("creative"))
        {
            let hole = g.tiles.get("hole");
            g.set_tile_default(lvl, xt, yt, &hole);
            let dirt = crate::item::registry::get(g, "dirt");
            drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, dirt);
            g.play_sound(Sound::MonsterHurt);
            return true;
        }
        if ttype == ToolType::Hoe
            && pay_stamina(player, 4 - tool_level)
            && item.pay_durability(g.is_mode("creative"))
        {
            let farmland = g.tiles.get("farmland");
            g.set_tile_default(lvl, xt, yt, &farmland);
            g.play_sound(Sound::MonsterHurt);
            return true;
        }
    }
    false
}

pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let col = get_color(g.level(lvl).depth);
    if let Some(sprite) = &def.sprite {
        sprite.render_color(screen, x * 16, y * 16, col);
    }
}
