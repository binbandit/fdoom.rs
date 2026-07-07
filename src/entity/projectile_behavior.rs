//! Behaviors of `fdoom.entity.Arrow` and `fdoom.entity.Spark`.

use crate::core::game::Game;
use crate::entity::projectile::ProjectileStyle;
use crate::entity::{Direction, Entity, EntityKind, behavior};
use crate::gfx::{Rectangle, Screen, color};
use crate::level;
use crate::level::tile::dispatch as tiles;

/// Post-port: the projectile stops here — drop its payload item (a thrown spear/knife
/// waiting to be picked back up), if any, then despawn it.
fn land(g: &mut Game, e: &mut Entity, x: i32, y: i32) {
    let payload = match &mut e.kind {
        EntityKind::Arrow(a) => a.payload.take(),
        _ => None,
    };
    if let (Some(data), Some(lvl)) = (payload, e.c.level) {
        let item = crate::item::registry::get(g, &data);
        level::drop_item(g, lvl, x, y, item);
    }
    behavior::remove_entity(g, e);
}

/// Java `Arrow.tick()` (also drives the post-port thrown/launched projectiles —
/// spears, knives, slingshot pellets — which land instead of vanishing).
pub fn arrow_tick(g: &mut Game, e: &mut Entity) {
    let Some(lvl) = e.c.level else { return };
    let (level_w, level_h) = {
        let level = g.level(lvl);
        (level.w, level.h)
    };

    // JAVA: note `>` not `>=` on the tile bounds (finite levels; infinite have no edge)
    if !g.level(lvl).is_infinite()
        && (e.c.x < 0 || e.c.x >> 4 > level_w || e.c.y < 0 || e.c.y >> 4 > level_h)
    {
        behavior::remove_entity(g, e);
        return;
    }

    let (dir, damage, owner, speed, has_payload) = {
        let EntityKind::Arrow(a) = &mut e.kind else {
            return;
        };
        // ranged-limited projectiles (thrown weapons, pellets) fall to the ground once
        // their flight time runs out
        if a.range_ticks > 0 {
            a.range_ticks -= 1;
            if a.range_ticks == 0 {
                let (x, y) = (e.c.x, e.c.y);
                land(g, e, x, y);
                return;
            }
        }
        (a.dir, a.damage, a.owner, a.speed, a.payload.is_some())
    };

    let (prev_x, prev_y) = (e.c.x, e.c.y);
    e.c.x += dir.x() * speed;
    e.c.y += dir.y() * speed;

    let entitylist = level::get_entities_in_rect(
        g,
        lvl,
        &Rectangle::new(e.c.x, e.c.y, 0, 0, Rectangle::CENTER_DIMS),
    );
    let critical_hit = g.random.next_int_bound(11) < 9;
    for hit_id in entitylist {
        let is_mob = g.entities.get(hit_id).map(|h| h.is_mob()).unwrap_or(false);
        if is_mob && hit_id != owner {
            let extradamage = (if g
                .entities
                .get(hit_id)
                .map(|h| h.is_player())
                .unwrap_or(false)
            {
                0
            } else {
                3
            }) + (if critical_hit { 0 } else { 1 });
            g.with_entity(hit_id, |mob, g| {
                behavior::mob_hurt_by_eid(g, owner, mob, damage + extradamage, dir);
            });
            if has_payload {
                // a thrown weapon sticks in whatever it hits, then drops at their feet
                let (x, y) = (e.c.x, e.c.y);
                land(g, e, x, y);
                return;
            }
        }

        let xt = e.c.x / 16;
        let yt = e.c.y / 16;
        let tile = g.tile_at(lvl, xt, yt);
        if !tiles::may_pass(g, &tile, lvl, xt, yt, e) && !tile.connects_to_water && tile.id != 16 {
            // drop the payload on the near side of the blocking tile, not inside it
            land(g, e, prev_x, prev_y);
        }
    }
}

/// Java `Arrow.render(screen)` (+ post-port styles for the thrown projectiles).
pub fn arrow_render(_g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    let EntityKind::Arrow(a) = &e.kind else {
        return;
    };
    let (xt, yt) = match a.style {
        // TODO(art): dedicated spear-in-flight cells; placeholder reuses the arrow's
        // directional cells (13..16,5) with the spear's wood/iron tint (e.c.col).
        ProjectileStyle::Arrow | ProjectileStyle::Spear => (
            match a.dir {
                Direction::Left => 14,
                Direction::Up => 15,
                Direction::Down => 16,
                _ => 13,
            },
            5,
        ),
        // TODO(art): dedicated knife-in-flight cell; placeholder reuses the shard item
        // cell (23,4).
        ProjectileStyle::Knife => (23, 4),
        // TODO(art): dedicated pellet cell; placeholder reuses the stone item cell (2,4).
        ProjectileStyle::Pellet => (2, 4),
    };
    screen.render(e.c.x - 4, e.c.y - 4, xt + yt * 32, e.c.col, 0);
}

/// Java `Arrow.getData()`.
pub fn arrow_get_data(e: &Entity) -> String {
    let EntityKind::Arrow(a) = &e.kind else {
        return String::new();
    };
    format!("{}:{}:{}", a.owner, a.dir.ordinal(), a.damage)
}

/// Adapted Java `Spark.tick()` — the Night Wisp's zap bolt. Same tile-ignoring
/// double-precision flight; it fizzles on hitting any mob other than a Night Wisp.
pub fn zap_tick(g: &mut Game, e: &mut Entity) {
    let owner = {
        let EntityKind::Zap(s) = &mut e.kind else {
            return;
        };
        s.time += 1;
        if s.time >= s.life_time {
            behavior::remove_entity(g, e);
            return;
        }
        s.xx += s.xa;
        s.yy += s.ya;
        e.c.x = s.xx as i32;
        e.c.y = s.yy as i32;
        s.owner
    };

    let Some(lvl) = e.c.level else { return };
    let to_hit = level::get_entities_in_rect(
        g,
        lvl,
        &Rectangle::new(e.c.x, e.c.y, 0, 0, Rectangle::CENTER_DIMS),
    );
    for hit_id in to_hit {
        let hurt_it = g
            .entities
            .get(hit_id)
            .map(|h| h.is_mob() && !matches!(h.kind, EntityKind::NightWisp(_)))
            .unwrap_or(false);
        if hurt_it {
            // JAVA (Spark): `mob.hurt(owner, 1)` — attack dir from the owner's position
            let owner_pos = g.entities.get(owner).map(|o| (o.c.x, o.c.y));
            let hit = g
                .with_entity(hit_id, |mob, g| {
                    let attack_dir = match owner_pos {
                        Some((ox, oy)) => Direction::get_direction(mob.c.x - ox, mob.c.y - oy),
                        None => Direction::None,
                    };
                    behavior::mob_hurt_by_eid(g, owner, mob, 1, attack_dir);
                })
                .is_some();
            // unlike the Spark swarm, a zap is a single bolt: spent on impact
            if hit {
                behavior::remove_entity(g, e);
                return;
            }
        }
    }
}

/// Adapted Java `Spark.render(screen)`.
pub fn zap_render(g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    let EntityKind::Zap(s) = &e.kind else {
        return;
    };
    // blinking effect near end of life
    if s.time >= s.life_time - 6 * 20 && s.time / 6 % 2 == 0 {
        return;
    }

    let xt = 8;
    let yt = 13;

    let randmirror = g.random.next_int_bound(4);
    screen.render(
        e.c.x - 4,
        e.c.y - 4 - 2,
        xt + yt * 32,
        color::WHITE,
        randmirror,
    ); // the zap bolt
    screen.render(
        e.c.x - 4,
        e.c.y - 4 + 2,
        xt + yt * 32,
        color::BLACK,
        randmirror,
    ); // its shadow
}

/// Adapted Java `Spark.getData()`.
pub fn zap_get_data(e: &Entity) -> String {
    let EntityKind::Zap(s) = &e.kind else {
        return String::new();
    };
    s.owner.to_string()
}
