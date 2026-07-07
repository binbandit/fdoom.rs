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
    // 0 = lit clod tops, 1 = soil base, 2 = clod under-shadow, 3 = stones
    color::get4(dcol + 111, dcol, dcol - 111, dcol - 111)
}

/// Java `DirtTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Dirt);
    // dedicated clods-and-stones texture (artgen `dirt_texture`, cells 21..24,3)
    def.sprite = Some(Sprite::dots_at(21, 3, get_color(0)));
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
    // fossicking: dirt pans only where the water works it (a wet bank)
    if super::fossick::water_adjacent(g, lvl, xt, yt)
        && super::fossick::try_pan(g, lvl, xt, yt, player, item)
    {
        return true;
    }
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
            // multi-level terrain: shoveling starts a pit you can keep digging deeper
            let pit = g.tiles.get("Dug Pit");
            g.set_tile_default(lvl, xt, yt, &pit);
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

/// Random tile tick: fires an armed cave-in fuse (see `fossick::collapse_check`).
pub fn tick(g: &mut Game, _def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    super::fossick::fuse_tick(g, lvl, xt, yt);
}

pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let col = get_color(g.level(lvl).depth);
    if let Some(sprite) = &def.sprite {
        sprite.render_color(screen, x * 16, y * 16, col);
    }
}
