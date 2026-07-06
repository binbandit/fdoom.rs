//! Port of `fdoom.level.tile.FlowerTile`.

use super::dispatch;
use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::entity::mob::player_behavior::pay_stamina;
use crate::gfx::{Screen, Sprite, color};
use crate::item::{Item, ItemKind, ToolType};
use crate::level::{drop_item, drop_items_counted};

/// Java `FlowerTile.flowersprite`.
fn flower_sprite() -> Sprite {
    Sprite::new1x1(1, 1, color::get4(10, 141, 555, 440))
}

/// Java `FlowerTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Flower);
    def.connects_to_grass = true;
    def.may_spawn = true;
    def
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
    let flower = crate::item::registry::get(g, "Flower");
    drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[flower]);
    let rose = crate::item::registry::get(g, "Rose");
    drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 0, 1, &[rose]);
    let grass = g.tiles.get("grass");
    g.set_tile_default(lvl, x, y, &grass);
    true
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
            && pay_stamina(player, 2 - tool_level)
            && item.pay_durability(g.is_mode("creative"))
        {
            let flower = crate::item::registry::get(g, "Flower");
            drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, flower);
            let rose = crate::item::registry::get(g, "Rose");
            drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, rose);
            let grass = g.tiles.get("grass");
            g.set_tile_default(lvl, xt, yt, &grass);
            return true;
        }
    }
    false
}

pub fn render(g: &mut Game, screen: &mut Screen, _def: &TileDef, lvl: usize, x: i32, y: i32) {
    let grass = g.tiles.get("grass");
    dispatch::render(g, screen, &grass, lvl, x, y);

    let data = g.level(lvl).get_data(x, y);
    let shape = (data / 16) % 2;

    let x = x << 4;
    let y = y << 4;

    let sprite = flower_sprite();
    sprite.render(screen, x + 8 * shape, y);
    sprite.render(screen, x + 8 * (if shape == 0 { 1 } else { 0 }), y + 8);
}
