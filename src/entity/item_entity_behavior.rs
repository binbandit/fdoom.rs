//! Behavior of `fdoom.entity.ItemEntity`.

use crate::core::game::Game;
use crate::entity::{Entity, EntityKind, behavior};
use crate::gfx::{Screen, color};
use crate::level::tile::{TileKind, tidal};

/// Java `ItemEntity.tick()`.
pub fn tick(g: &mut Game, e: &mut Entity) {
    let (expected_x, expected_y);
    {
        let EntityKind::ItemEntity(d) = &mut e.kind else {
            return;
        };
        d.time += 1;
        if d.time >= d.life_time {
            behavior::remove_entity(g, e);
            return;
        }
        // moves each coordinate by its acceleration
        d.xx += d.xa;
        d.yy += d.ya;
        d.zz += d.za;
        if d.zz < 0.0 {
            // hitting the ground
            d.zz = 0.0;
            d.za *= -0.5;
            d.xa *= 0.6;
            d.ya *= 0.6;
        }
        d.za -= 0.15;

        let nx = d.xx as i32;
        let ny = d.yy as i32;
        expected_x = nx - e.c.x; // expected movement distance
        expected_y = ny - e.c.y;
    }

    let ox = e.c.x;
    let oy = e.c.y;

    behavior::entity_move(g, e, expected_x, expected_y);

    // accounts for any error in the double-to-int position conversion
    let gotx = e.c.x - ox;
    let goty = e.c.y - oy;
    if let EntityKind::ItemEntity(d) = &mut e.kind {
        d.xx += (gotx - expected_x) as f64;
        d.yy += (goty - expected_y) as f64;
    }
}

/// Java `ItemEntity.render(screen)`, plus the floating-drop treatment: on liquid
/// tiles the drop bobs inside a small ripple ring and casts no shadow — a drop
/// used to sit flat on open water with a hard black sprite-copy under it
/// (ODDITIES O8).
pub fn render(g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    let EntityKind::ItemEntity(d) = &e.kind else {
        return;
    };
    // blinking effect near the end of its life
    if d.time >= d.life_time - 6 * 20 && d.time / 6 % 2 == 0 {
        return;
    }

    // Same liquid families as `behavior::is_swimming`, plus Deep Water (drops
    // drift over it — see `depth::deep_water_may_pass`). Each maps to its
    // (shimmer, rest) ring palettes, matching the player's swim ring.
    let liquid = e.c.level.and_then(|lvl| {
        let (xt, yt) = (e.c.x >> 4, e.c.y >> 4);
        match g.tile_at(lvl, xt, yt).kind {
            TileKind::Lava => Some((
                color::get4(-1, 300, 400, 500),
                color::get4(-1, -1, 500, 300),
            )),
            TileKind::DeepWater => {
                Some((color::get4(-1, 225, 4, 104), color::get4(-1, -1, 104, 225)))
            }
            TileKind::TidalFlat if tidal::is_submerged(g, xt, yt) => {
                Some((color::get4(-1, 324, 4, 114), color::get4(-1, -1, 114, 324)))
            }
            TileKind::Water | TileKind::SpringWater | TileKind::Seaweed | TileKind::Coral => {
                Some((color::get4(-1, 335, 5, 115), color::get4(-1, -1, 115, 335)))
            }
            _ => None,
        }
    });
    if let Some((shine, rest)) = liquid {
        // ripple ring: the swim-ring halves (player_behavior uses the same cell),
        // shimmering on the same 8-tick cadence, in the liquid's own palette
        let ring = if d.time / 8 % 2 == 0 { shine } else { rest };
        screen.render(e.c.x - 8, e.c.y - 2, 5 + 13 * 32, ring, 0);
        screen.render(e.c.x, e.c.y - 2, 5 + 13 * 32, ring, 1);
        // slight bob instead of the bounce height, no shadow on the water
        let bob = i32::from(d.time / 16 % 2 == 0);
        d.item.sprite.render(screen, e.c.x - 4, e.c.y - 4 + bob);
        return;
    }

    d.item
        .sprite
        .render_color(screen, e.c.x - 4, e.c.y - 4, color::BLACK);
    d.item
        .sprite
        .render(screen, e.c.x - 4, e.c.y - 4 - d.zz as i32);
}

/// Java `ItemEntity.touchedBy(entity)` — `this_e` is the item entity, `by` the toucher.
pub fn touched_by(g: &mut Game, this_e: &mut Entity, by: &mut Entity) {
    if !by.is_player() {
        return; // we only care when a player touches an item
    }

    let ready = {
        let EntityKind::ItemEntity(d) = &this_e.kind else {
            return;
        };
        d.time > 30 && !d.picked_up // conditional prevents immediate collection
    };
    if ready {
        if let EntityKind::ItemEntity(d) = &mut this_e.kind {
            d.picked_up = true;
        }
        super::mob::player_behavior::pickup_item(g, by, this_e);
        let removed = this_e.c.removed;
        if let EntityKind::ItemEntity(d) = &mut this_e.kind {
            d.picked_up = removed;
        }
    }
}
