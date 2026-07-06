//! Port of `fdoom.level.tile.QuickSandTile`.

use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::gfx::{Screen, Sprite, color};

/// Java `QuickSandTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::QuickSand);
    def.sprite = Some(Sprite::new(22, 1, 2, 2, color::get4(222, 0, 333, 444), 0));
    def.connects_to_sand = true;
    def
}

/// Java `QuickSandTile.getDirtColor(depth)`.
fn get_dirt_color(depth: i32) -> i32 {
    match depth {
        0 => 550,
        1 => 444,
        -4 => 59,
        _ => 222,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let depth = g.level(lvl).depth;
    let col = if depth < 0 {
        color::get4(get_dirt_color(depth), 0, 333, color::hex("#f4a460"))
    } else {
        color::get4(
            get_dirt_color(depth),
            color::rgb(217, 218, 104),
            color::rgb(188, 190, 78),
            color::rgb(159, 163, 51),
        )
    };

    if let Some(sprite) = &def.sprite {
        sprite.render_color(screen, x * 16, y * 16, col);
    }
}
