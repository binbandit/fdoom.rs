//! Fence: a solid post that visually joins up with its neighbors.

use super::dispatch;
use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::entity::Entity;
use crate::gfx::{Screen, Sprite, color};

/// Java `FenceTile` constructor — `super(name)` (no sprite).
pub fn make(name: &str) -> TileDef {
    TileDef::new(name, TileKind::Fence)
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let transition_color = color::get4(310, 420, 530, -1);

    let u = g.tile_at(lvl, x, y - 1).id == def.id;
    let d = g.tile_at(lvl, x, y + 1).id == def.id;
    let l = g.tile_at(lvl, x - 1, y).id == def.id;
    let r = g.tile_at(lvl, x + 1, y).id == def.id;

    let ul = g.tile_at(lvl, x - 1, y - 1).id == def.id;
    let dl = g.tile_at(lvl, x - 1, y + 1).id == def.id;
    let ur = g.tile_at(lvl, x + 1, y - 1).id == def.id;
    let dr = g.tile_at(lvl, x + 1, y + 1).id == def.id;

    // ground first, then one 8x8 fence sprite per connected quadrant
    let dirt = g.tiles.get("dirt");
    dispatch::render(g, screen, &dirt, lvl, x, y);

    let sprite = Sprite::new1x1(6, 4, transition_color);
    let (px, py) = (x << 4, y << 4);
    if ul || u || l {
        sprite.render(screen, px, py);
    }
    if dl || d || l {
        sprite.render(screen, px, py + 8);
    }
    if ur || u || r {
        sprite.render(screen, px + 8, py);
    }
    if dr || d || r {
        sprite.render(screen, px + 8, py + 8);
    }
    if !(u || d || l || r || ul || dl || ur || dr) {
        // Lone fence post: render the sprite centered so the tile isn't empty.
        sprite.render(screen, px + 4, py + 4);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
    false
}
