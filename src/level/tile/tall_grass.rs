//! Port of `fdoom.level.tile.TallGrassTile`.

use super::dispatch;
use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::{Screen, Sprite, color};

/// Java static `small` sprite.
fn small() -> Sprite {
    Sprite::new(28, 8, 2, 2, color::get4(-1, 30, 40, -1), 0)
}

/// Java static `medium` sprite.
fn medium() -> Sprite {
    Sprite::new(30, 8, 2, 2, color::get4(-1, 30, 40, -1), 0)
}

/// Java static `tall` sprite.
fn tall() -> Sprite {
    Sprite::new(26, 8, 2, 2, color::get4(-1, 30, 40, -1), 0)
}

/// Kind 3 (post-port): Reeds — marsh-edge tufts that reuse the tall-grass mechanics
/// but never grow, never block, and shred into fibers.
pub const KIND_REEDS: i32 = 3;

/// Java `TallGrassTile` constructor. `on_tile` is always "grass" in this fork.
pub fn make(name: &str, on_tile: &str, kind: i32) -> TileDef {
    let _ = on_tile;
    let mut def = TileDef::new(name, TileKind::TallGrass { kind });
    def.sprite = Some(small());
    // connects like the grass it stands on
    def.connects_to_grass = true;
    def.may_spawn = true;
    def.flammable = true; // every stage, reeds included — dry standing fuel
    def
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let TileKind::TallGrass { kind } = def.kind else {
        return;
    };
    // draw the grass ground first, then the tuft over it
    let on_type = g.tiles.get("grass");
    dispatch::render(g, screen, &on_type, lvl, x, y);
    match kind {
        0 => small().render(screen, x * 16, y * 16),
        1 => medium().render(screen, x * 16, y * 16),
        2 => tall().render_color(screen, x * 16, y * 16, color::get4(-1, 210, 530, 550)),
        // TODO(art): final cells — reeds reuse the tall-grass cell with a dry palette
        KIND_REEDS => tall().render_color(screen, x * 16, y * 16, color::get4(-1, 320, 431, 542)),
        _ => {}
    }
}

pub fn tick(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    let TileKind::TallGrass { kind } = def.kind else {
        return;
    };
    // slow growth: ~1-in-2000 per random tick, so a stage takes a few in-game days
    if kind < 2 && g.random.next_int_bound(2000) == 4 {
        let next = match kind {
            0 => g.tiles.get_id(40),
            _ => g.tiles.get_id(41),
        };
        g.set_tile_default(lvl, xt, yt, &next);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn hurt_by(
    g: &mut Game,
    def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
    _source: &mut Entity,
    _dmg: i32,
    _attack_dir: Direction,
) -> bool {
    let TileKind::TallGrass { kind } = def.kind else {
        return false;
    };
    let grass = g.tiles.get("grass");
    g.set_tile_default(lvl, x, y, &grass);
    g.play_sound(Sound::MonsterHurt);

    // Drops scale with growth (kind 0/1 = small/medium, 2 = tall). Tall grass is the
    // reliable fiber source of the bare-handed starter loop; every stage can also
    // uncover a loose stone "pebble" — the no-pickaxe way to get Stone for knapping.
    let fibers = crate::item::registry::get(g, "grass fibers");
    let stone = crate::item::registry::get(g, "Stone");
    if kind == KIND_REEDS {
        // reeds shred into fibers, no pebbles (they grow in soft marsh ground)
        for _ in 0..2 {
            crate::level::drop_item(g, lvl, x * 16 + 8, y * 16 + 8, fibers.clone());
        }
    } else if kind == 2 {
        for _ in 0..2 {
            crate::level::drop_item(g, lvl, x * 16 + 8, y * 16 + 8, fibers.clone());
        }
        if g.random.next_int_bound(4) == 0 {
            crate::level::drop_item(g, lvl, x * 16 + 8, y * 16 + 8, stone);
        }
    } else {
        // Younger growth: fibers only sometimes, pebbles rarely.
        if g.random.next_int_bound(3) == 0 {
            crate::level::drop_item(g, lvl, x * 16 + 8, y * 16 + 8, fibers);
        }
        if g.random.next_int_bound(8) == 0 {
            crate::level::drop_item(g, lvl, x * 16 + 8, y * 16 + 8, stone);
        }
    }

    true
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(g: &Game, def: &TileDef, lvl: usize, x: i32, y: i32, _e: &Entity) -> bool {
    let TileKind::TallGrass { kind } = def.kind else {
        return true;
    };
    if kind != 2 {
        return true; // young growth and reeds never block
    }
    // Fully-grown thicket only blocks deep inside a paddock: one or two stalks are
    // brushed through, but a tile ringed almost entirely by other thicket (6+ of its
    // 8 neighbors) is impenetrable. Meadow cores stay dense, their fringes walkable.
    let mut thicket_neighbors = 0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            if (dx, dy) == (0, 0) {
                continue;
            }
            if g.tile_at(lvl, x + dx, y + dy).id == def.id {
                thicket_neighbors += 1;
            }
        }
    }
    thicket_neighbors < 6
}
