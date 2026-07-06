//! Behavior of `fdoom.entity.ItemEntity`.

use crate::core::game::Game;
use crate::entity::{Entity, EntityKind, behavior};
use crate::gfx::{Screen, color};

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

/// Java `ItemEntity.render(screen)`.
pub fn render(_g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    let EntityKind::ItemEntity(d) = &e.kind else {
        return;
    };
    // blinking effect near the end of its life
    if d.time >= d.life_time - 6 * 20 && d.time / 6 % 2 == 0 {
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
