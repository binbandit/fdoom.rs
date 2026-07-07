//! Port of `fdoom.level.tile.GraveStoneTile`.

use super::dispatch;
use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::core::updater::Time;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::{Screen, Sprite, color};
use crate::item::Item;

// JAVA: hasRunTonight/hasSpawnedZombie were instance fields on the tile-class singleton,
// shared by every grave stone tile on every level and never reset — the state leaked
// across worlds (one grave crumbling stopped every other grave from ever crumbling).
// FIX: the flag lives in the tile's per-position data byte instead (grave stones never
// used their data value), so each grave tracks its own state, it is world-scoped, and it
// round-trips through saves for free. For an unbroken grave the flag means "already
// rolled the crumble chance tonight"; for a broken grave it means "already spawned its
// night zombie".
const FLAG_SET: i32 = 1;

/// Java static `sprite` (unbroken).
fn sprite() -> Sprite {
    Sprite::new(
        11,
        11,
        2,
        2,
        color::get4(-1, 300, color::rgb(60, 63, 65), 550),
        0,
    )
}

/// Java static `broken` sprite.
fn broken_sprite() -> Sprite {
    Sprite::new(13, 11, 2, 2, color::get4(-1, 300, 300, 300), 0)
}

/// Java `GraveStoneTile` constructor.
pub fn make(name: &str, broken: bool) -> TileDef {
    let mut def = TileDef::new(name, TileKind::GraveStone { broken });
    def.sprite = Some(sprite());
    def.connects_to_grass = true;
    def
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let TileKind::GraveStone { broken } = def.kind else {
        return;
    };
    let grass = g.tiles.get("grass");
    dispatch::render(g, screen, &grass, lvl, x, y);

    if !broken {
        sprite().render_color(
            screen,
            x * 16,
            y * 16,
            color::get4(
                -1,
                color::hex("#515151"),
                color::hex("#808080"),
                color::hex("#515151"),
            ),
        );
    } else {
        broken_sprite().render_color(
            screen,
            x * 16,
            y * 16,
            color::get4(-1, color::hex("#515151"), color::hex("#808080"), 321),
        );
    }
}

pub fn tick(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    let TileKind::GraveStone { broken: is_broken } = def.kind else {
        return;
    };
    let flag = g.level(lvl).get_data(xt, yt);

    if is_broken {
        if flag == 0 && g.get_time() == Time::Night {
            let mut new_mob = crate::entity::mob::zombie::new(g, 1);
            // JAVA: set the mob's *pixel* coordinates to the *tile* coordinates, dumping
            // the zombie near the map origin. FIX: spawn at the grave tile's center.
            new_mob.c.x = xt * 16 + 8;
            new_mob.c.y = yt * 16 + 8;
            g.level_mut(lvl).add(new_mob, lvl);

            g.level_mut(lvl).set_data(xt, yt, FLAG_SET);
        }
        return;
    }

    match g.get_time() {
        Time::Morning => {
            // Night is over — allow this grave to roll its crumble chance again tonight.
            if flag != 0 {
                g.level_mut(lvl).set_data(xt, yt, 0);
            }
        }
        Time::Night if flag == 0 => {
            // One crumble roll per grave per night.
            if g.random.next_boolean() {
                let broken = g.tiles.get_id(44);
                // set_tile_default resets the data byte, so the fresh broken grave
                // starts with its "spawned zombie" flag clear.
                g.set_tile_default(lvl, xt, yt, &broken);
            } else {
                g.level_mut(lvl).set_data(xt, yt, FLAG_SET);
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
    false
}

pub fn get_light_radius(_g: &Game, def: &TileDef, _lvl: usize, _x: i32, _y: i32) -> i32 {
    let TileKind::GraveStone { broken } = def.kind else {
        return 0;
    };
    if broken { 2 } else { 0 }
}

#[allow(clippy::too_many_arguments)]
pub fn interact(
    g: &mut Game,
    def: &TileDef,
    lvl: usize,
    xt: i32,
    yt: i32,
    _player: &mut Entity,
    _item: &mut Item,
    _attack_dir: Direction,
) -> bool {
    let TileKind::GraveStone { broken } = def.kind else {
        return false;
    };
    if !broken {
        // JAVA: the zombie was added without a position (spawned at 0,0). FIX: spawn it
        // at the center of the grave tile being disturbed.
        let mut zombie = crate::entity::mob::zombie::new(g, 5);
        zombie.c.x = xt * 16 + 8;
        zombie.c.y = yt * 16 + 8;
        g.level_mut(lvl).add(zombie, lvl);
        let broken_tile = g.tiles.get_id(44);
        g.set_tile_default(lvl, xt, yt, &broken_tile);
        g.change_time_of_day(Time::Evening);
    }
    false
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
    let TileKind::GraveStone { broken } = def.kind else {
        return true;
    };
    if !broken {
        // JAVA: the zombie was added without a position (spawned at 0,0). FIX: spawn it
        // at the center of the grave tile being disturbed.
        let mut zombie = crate::entity::mob::zombie::new(g, 1);
        zombie.c.x = x * 16 + 8;
        zombie.c.y = y * 16 + 8;
        g.level_mut(lvl).add(zombie, lvl);
        let broken_tile = g.tiles.get_id(44);
        g.set_tile_default(lvl, x, y, &broken_tile);
        g.change_time_of_day(Time::Evening);
    }
    true
}
