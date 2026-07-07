//! Port of `fdoom.level.tile.WheatTile`.

use super::{TileDef, TileKind, tool_use};
use crate::core::game::Game;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::{Screen, color};
use crate::item::{Item, ToolType};

/// Java `WheatTile` constructor — `super(name, (Sprite)null)`.
pub fn make(name: &str) -> TileDef {
    TileDef::new(name, TileKind::Wheat)
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, _def: &TileDef, lvl: usize, x: i32, y: i32) {
    let age = g.level(lvl).get_data(x, y);
    let mut icon = age / 10;

    let mut col = color::get4(301, 411, 321, 50);
    let col1 = color::get4(301, 411, 50 + icon * 100, 40 + (icon - 3) * 2 * 100);
    let col2 = color::get4(0, 0, 50 + icon * 100, 40 + (icon - 3) * 2 * 100);

    if icon >= 3 {
        col = col1;
        if age == 50 {
            col = col2;
        }
        icon = 3;
    }

    screen.render(x * 16, y * 16, 4 + 3 * 32 + icon, col, 0);
    screen.render(x * 16 + 8, y * 16, 4 + 3 * 32 + icon, col, 0);
    screen.render(x * 16, y * 16 + 8, 4 + 3 * 32 + icon, col, 1);
    screen.render(x * 16 + 8, y * 16 + 8, 4 + 3 * 32 + icon, col, 1);
}

/// Java `WheatTile.IfWater(level, xs, ys)`.
fn if_water(g: &Game, lvl: usize, xs: i32, ys: i32) -> bool {
    let area_tiles = crate::level::get_area_tiles(g, lvl, xs, ys, 1, 1);
    for t in area_tiles {
        if t.name == "WATER" {
            return true;
        }
    }
    false
}

pub fn tick(g: &mut Game, _def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    if g.random.next_int_bound(2) == 0 {
        return;
    }

    // wheat next to water grows twice as fast
    let age = g.level(lvl).get_data(xt, yt);
    if age < 50 {
        let step = if if_water(g, lvl, xt, yt) { 2 } else { 1 };
        g.level_mut(lvl).set_data(xt, yt, age + step);
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
    if tool_use(g, player, item, ToolType::Shovel, 4).is_some() {
        let dirt = g.tiles.get("dirt");
        g.set_tile_default(lvl, xt, yt, &dirt);
        return true;
    }
    false
}

pub fn stepped_on(g: &mut Game, _def: &TileDef, lvl: usize, xt: i32, yt: i32, e: &mut Entity) {
    if g.random.next_int_bound(60) != 0 {
        return;
    }
    if g.level(lvl).get_data(xt, yt) < 2 {
        return;
    }
    harvest(g, lvl, xt, yt, e);
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
    harvest(g, lvl, x, y, source);
    true
}

/// Java `WheatTile.harvest(level, x, y, entity)`.
fn harvest(g: &mut Game, lvl: usize, x: i32, y: i32, entity: &mut Entity) {
    let age = g.level(lvl).get_data(x, y);

    let seeds = crate::item::registry::get(g, "seeds");
    crate::level::drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[seeds]);

    let mut count = 0;
    if age >= 50 {
        count = g.random.next_int_bound(3) + 2;
    } else if age >= 40 {
        count = g.random.next_int_bound(2) + 1;
    }

    let wheat = crate::item::registry::get(g, "Wheat");
    for _ in 0..count {
        crate::level::drop_item(g, lvl, x * 16 + 8, y * 16 + 8, wheat.clone());
    }

    if age >= 50 && entity.is_player() {
        let points = g.random.next_int_bound(5) + 1;
        let score_mode = g.is_mode("score");
        entity.player_mut().add_score(points, score_mode);
    }
    let dirt = g.tiles.get("dirt");
    g.set_tile_default(lvl, x, y, &dirt);
}
