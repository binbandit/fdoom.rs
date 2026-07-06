//! Port of `fdoom.level.tile.StairsTile`.

use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::gfx::{Screen, Sprite, color};

/// Java `StairsTile` constructor.
pub fn make(name: &str, leads_up: bool) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Stairs { leads_up });
    let col = color::get4(222, 0, 333, 444);
    def.sprite = Some(if leads_up {
        Sprite::new(2, 2, 2, 2, col, 0) // Java `up`
    } else {
        Sprite::new(0, 2, 2, 2, col, 0) // Java `down`
    });
    def.may_spawn = false;
    def
}

/// Java `StairsTile.getDirtColor(depth)`.
// JAVA: differs from DirtTile.dCol at depth -4 (59 instead of 203).
fn get_dirt_color(depth: i32) -> i32 {
    match depth {
        0 => 321,
        1 => 444,
        -4 => 59,
        _ => 222,
    }
}

pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let depth = g.level(lvl).depth;
    let col = if depth < 0 {
        color::get4(get_dirt_color(depth), 0, 333, 444)
    } else {
        color::get4(get_dirt_color(depth), 0, 444, 555)
    };

    if let Some(sprite) = &def.sprite {
        sprite.render_color(screen, x * 16, y * 16, col);
    }
}
