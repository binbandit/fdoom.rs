//! Port of `fdoom.level.tile.SandTile`.

use super::{ConnectorSprite, TileDef, TileKind, dispatch};
use crate::core::game::Game;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::entity::mob::player_behavior::pay_stamina;
use crate::gfx::sprite::Px;
use crate::gfx::{Screen, Sprite, color};
use crate::item::{Item, ItemKind, ToolType};
use crate::level::drop_item;

/// Java `SandTile.steppedOn` (the static footprint sprite).
fn stepped_on_sprite() -> Sprite {
    let pixels = vec![
        vec![Px::new(3, 1, 0), Px::new(27, 0, 0)],
        vec![Px::new(28, 0, 0), Px::new(3, 1, 0)],
    ];
    Sprite::from_pixels(pixels, color::get4(552, 550, 440, 440))
}

/// Java `SandTile.normal`.
fn normal_sprite() -> Sprite {
    // dedicated dune-ripple texture (artgen `sand_texture`, cells 26..29,0):
    // 0 = sunlit crest, 1 = sand base, 2/3 = ripple shadow
    Sprite::dots_at(26, 0, color::get4(552, 550, 440, 440))
}

/// Java `SandTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Sand);
    def.csprite = Some(ConnectorSprite::simple(
        Sprite::new(11, 0, 3, 3, color::get4(440, 550, 440, 321), 3),
        normal_sprite(),
    ));
    def.connects_to_sand = true;
    def.may_spawn = true;
    def
}

/// Java anonymous `ConnectorSprite.connectsTo` override.
pub fn connects_to(_def: &TileDef, other: &TileDef, is_side: bool) -> bool {
    if !is_side {
        return true;
    }
    other.connects_to_sand
}

pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let stepped_on = g.level(lvl).get_data(x, y) > 0;

    let mut tmp = def.clone();
    let cs = tmp.csprite.as_mut().expect("sand has a csprite");
    cs.full = if stepped_on {
        stepped_on_sprite()
    } else {
        normal_sprite()
    };

    dispatch::csprite_render(g, screen, &tmp, lvl, x, y, None);
}

pub fn tick(g: &mut Game, _def: &TileDef, lvl: usize, x: i32, y: i32) {
    let d = g.level(lvl).get_data(x, y);
    if d > 0 {
        g.level_mut(lvl).set_data(x, y, d - 1);
    }
}

pub fn stepped_on(g: &mut Game, _def: &TileDef, lvl: usize, x: i32, y: i32, e: &mut Entity) {
    if e.mob().is_some() {
        g.level_mut(lvl).set_data(x, y, 10);
    }
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
            let dirt = g.tiles.get("dirt");
            g.set_tile_default(lvl, xt, yt, &dirt);
            let sand = crate::item::registry::get(g, "sand");
            drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, sand);
            return true;
        }
    }
    false
}
