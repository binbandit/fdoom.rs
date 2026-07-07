//! Timber Prop (sandbox era, no Java counterpart): a mine-ceiling support post.
//!
//! Placed on dirt (mine floors) via its tile item; while one stands within
//! `fossick::PROP_RADIUS` tiles, breaking rock never triggers a cave-in there
//! (see `fossick.rs`). Walk-through — you pass under the beams — and one hit
//! knocks it down, refunding the timber.
//!
//! TODO(art): dedicated prop cells (two uprights + a header beam). Until the art
//! agent lands them, this reuses the fence-post cell for the uprights.

use super::{TileDef, TileKind, dispatch};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::{Direction, Entity};
use crate::gfx::{Screen, Sprite, color};
use crate::level::drop_items_counted;

pub fn make(name: &str) -> TileDef {
    // no may_pass override: entities walk under the beams
    TileDef::new(name, TileKind::TimberProp)
}

pub fn render(g: &mut Game, screen: &mut Screen, _def: &TileDef, lvl: usize, x: i32, y: i32) {
    let dirt = g.tiles.get("dirt");
    dispatch::render(g, screen, &dirt, lvl, x, y);

    // two uprights (fence-post cell) and a header-beam shadow across the top
    let post = Sprite::new1x1(6, 4, color::get4(310, 420, 530, -1));
    let (px, py) = (x << 4, y << 4);
    post.render(screen, px, py);
    post.render(screen, px, py + 8);
    post.render(screen, px + 8, py);
    post.render(screen, px + 8, py + 8);
    screen.darken_rect(px, py, 16, 3, 60);
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
    // one hit knocks the prop down; the timber is (mostly) recovered
    let wood = crate::item::registry::get(g, "Wood");
    drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[wood]);
    let stick = crate::item::registry::get(g, "Stick");
    drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, 1, 2, &[stick]);
    let dirt = g.tiles.get("dirt");
    g.set_tile_default(lvl, x, y, &dirt);
    g.play_sound(Sound::MonsterHurt);
    true
}
