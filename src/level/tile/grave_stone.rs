//! Port of `fdoom.level.tile.GraveStoneTile`.

use super::dispatch;
use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::core::updater::Time;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::{Screen, Sprite, color};
use crate::item::Item;

// Per-grave night state lives in the tile's per-position data byte (grave stones have
// no other use for it), so each grave tracks its own state and it round-trips through
// saves for free. For an unbroken grave the flag means "already rolled the crumble
// chance tonight"; for a broken grave it means "already spawned its night zombie".
const FLAG_SET: i32 = 1;

/// Standing marker shapes (artgen `gravestone_cells`, all 2x2 true-color blocks on
/// sheet rows 11..=12): slab, rounded headstone, stone cross, cracked slab, wooden
/// cross — cemeteries mix stone and wood markers.
const STANDING: [(i32, i32); 5] = [(11, 11), (15, 11), (17, 11), (19, 11), (23, 11)];
/// Crumbled shapes: two stone rubble piles + the collapsed wooden cross.
const BROKEN_STONE: [(i32, i32); 2] = [(13, 11), (21, 11)];
const BROKEN_WOOD: (i32, i32) = (25, 11);

/// Deterministic per-position shape pick, stable across frames and saves. A broken
/// grave keeps its standing shape's material: the position that picked the wooden
/// cross crumbles into the collapsed wooden cross.
fn shape(x: i32, y: i32, broken: bool) -> (i32, i32) {
    let h = (x.wrapping_mul(73_856_093) ^ y.wrapping_mul(19_349_663)).unsigned_abs();
    let standing_ix = (h % STANDING.len() as u32) as usize;
    if !broken {
        STANDING[standing_ix]
    } else if standing_ix == STANDING.len() - 1 {
        BROKEN_WOOD
    } else {
        BROKEN_STONE[(h / 7 % BROKEN_STONE.len() as u32) as usize]
    }
}

/// Java static `sprite` (unbroken). The item/tile-list icon keeps the classic slab;
/// in-world rendering varies the shape per position (see `render`).
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

    let (cx, cy) = shape(x, y, broken);
    // the marker art is true color; the palette word is inert and kept only for the
    // render call shape
    Sprite::new(cx, cy, 2, 2, 0, 0).render(screen, x * 16, y * 16);
}

pub fn tick(g: &mut Game, def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    let TileKind::GraveStone { broken: is_broken } = def.kind else {
        return;
    };
    let flag = g.level(lvl).get_data(xt, yt);

    if is_broken {
        if flag == 0 && g.get_time() == Time::Night {
            let mut new_mob = crate::entity::mob::zombie::new(g, 1);
            // pixel coordinates: the center of this grave's tile
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
        Time::Night => {
            if crate::core::events::hollow_night_active(g) {
                // HOLLOW NIGHT (core::events): the once-per-night flag is ignored, so
                // every random tile tick re-rolls at 1-in-3 — the whole cemetery caves
                // in before dawn instead of decaying over weeks.
                if g.random.next_int_bound(3) == 0 {
                    let broken = g.tiles.get_id(44);
                    g.set_tile_default(lvl, xt, yt, &broken);
                }
            } else if flag == 0 && !crate::core::events::grave_decay_suppressed(g) {
                // Quiet week after a Hollow Night: no crumble roll at all (the guard
                // above). Otherwise one crumble roll per grave per night, at ~17% so a
                // cemetery decays (and leaks zombies) over one or two in-game weeks
                // instead of collapsing almost entirely on the first couple of nights.
                if g.random.next_int_bound(6) == 0 {
                    let broken = g.tiles.get_id(44);
                    // set_tile_default resets the data byte, so the fresh broken grave
                    // starts with its "spawned zombie" flag clear.
                    g.set_tile_default(lvl, xt, yt, &broken);
                } else {
                    g.level_mut(lvl).set_data(xt, yt, FLAG_SET);
                }
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
        // disturbing a grave rouses its occupant at the tile's center
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
        // smashing a grave rouses its occupant at the tile's center
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
