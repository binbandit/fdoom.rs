//! Port of `fdoom.level.tile.GrassTile`.

use super::{ConnectorSprite, TileDef, TileKind};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::entity::mob::player_behavior::pay_stamina;
use crate::gfx::{Sprite, color};
use crate::item::{Item, ItemKind, ToolType};
use crate::level::drop_item;

/// Java `GrassTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Grass);
    // JAVA: the constructor sets `csprite.sides = csprite.sparse`, which the two-sprite
    // ConnectorSprite constructor already did.
    def.csprite = Some(ConnectorSprite::simple(
        Sprite::new(11, 0, 3, 3, color::get4(141, 141, 252, 321), 3),
        Sprite::dots(color::get4(141, 141, 252, 321)),
    ));
    def.connects_to_grass = true;
    def.may_spawn = true;
    def
}

pub fn tick(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    // JAVA: "TODO revise this method."
    if g.random.next_int_bound(40) != 0 {
        return;
    }

    let mut xn = xt;
    let mut yn = yt;

    if g.random.next_boolean() {
        xn += g.random.next_int_bound(2) * 2 - 1;
    } else {
        yn += g.random.next_int_bound(2) * 2 - 1;
    }

    if g.tile_at(lvl, xn, yn).same_tile(&g.tiles.get("dirt")) {
        g.set_tile_default(lvl, xn, yn, def);
    }

    // JAVA: `random.nextInt((10 - 1) + 1) + 1 == 4` — 1 in 10 chance, oddly written.
    if g.random.next_int_bound((10 - 1) + 1) + 1 == 4 {
        g.set_tile_named(lvl, xt, yt, "Small Grass");
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
            g.play_sound(Sound::MonsterHurt);
            // Digging up turf occasionally frees usable fibers — the rare plain-grass
            // counterpart to the reliable Tall Grass drop.
            if g.random.next_int_bound(4) == 0 {
                let fibers = crate::item::registry::get(g, "Grass Fibers");
                drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, fibers);
            }
            if g.random.next_int_bound(5) == 0 {
                // JAVA: dropItem(x, y, 2, seeds) — the count overload, two drops.
                let seeds = crate::item::registry::get(g, "seeds");
                for _ in 0..2 {
                    drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, seeds.clone());
                }
                return true;
            }
            // JAVA: falls through (returning false) when no seeds drop.
        }
        if ttype == ToolType::Hoe
            && pay_stamina(player, 4 - tool_level)
            && item.pay_durability(g.is_mode("creative"))
        {
            g.play_sound(Sound::MonsterHurt);
            if g.random.next_int_bound(5) == 0 {
                let seeds = crate::item::registry::get(g, "seeds");
                drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, seeds);
                return true;
            }
            let farmland = g.tiles.get("farmland");
            g.set_tile_default(lvl, xt, yt, &farmland);
            return true;
        }
    }
    false
}

/// Java anonymous `ConnectorSprite.connectsTo` override.
pub fn connects_to(_def: &TileDef, other: &TileDef, is_side: bool) -> bool {
    if !is_side {
        return true;
    }
    other.connects_to_grass
}
