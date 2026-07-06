//! Port of `fdoom.level.tile.FenceTile`.
//!
//! JAVA: the Java class declares eight static sprites (`su`, `sd`, `sr`, `sl`, `sul`,
//! `sdl`, `sur`, `sdr`) and a `col` local that are only referenced from commented-out
//! rendering code; they are omitted here along with that dead code.

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

    // JAVA: every branch renders the same 1x1 sprite at the raw tile coordinates
    // (x, y), not pixel coordinates — preserved as-is.
    if ul {
        Sprite::new1x1(6, 4, transition_color).render(screen, x, y);
    } else {
        if u {
            Sprite::new1x1(6, 4, transition_color).render(screen, x, y);
        }
        if l {
            Sprite::new1x1(6, 4, transition_color).render(screen, x, y);
        }
    }
    if dl {
        Sprite::new1x1(6, 4, transition_color).render(screen, x, y);
    } else {
        if d {
            Sprite::new1x1(6, 4, transition_color).render(screen, x, y);
        }
        if l {
            Sprite::new1x1(6, 4, transition_color).render(screen, x, y);
        }
    }
    if ur {
        Sprite::new1x1(6, 4, transition_color).render(screen, x, y);
    } else {
        if u {
            Sprite::new1x1(6, 4, transition_color).render(screen, x, y);
        }
        if r {
            Sprite::new1x1(6, 4, transition_color).render(screen, x, y);
        }
    }
    if dr {
        Sprite::new1x1(6, 4, transition_color).render(screen, x, y);
    } else {
        if d {
            Sprite::new1x1(6, 4, transition_color).render(screen, x, y);
        }
        if r {
            Sprite::new1x1(6, 4, transition_color).render(screen, x, y);
        }
    }

    let dirt = g.tiles.get("dirt");
    dispatch::render(g, screen, &dirt, lvl, x, y);
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
    false
}
