//! Behaviors of `fdoom.entity.Arrow` and `fdoom.entity.Spark`.

use crate::core::game::Game;
use crate::entity::{Direction, Entity, EntityKind, behavior};
use crate::gfx::{Rectangle, Screen, color};
use crate::level;
use crate::level::tile::dispatch as tiles;

/// Java `Arrow.tick()`.
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

    let (dir, damage, owner, speed) = {
        let EntityKind::Arrow(a) = &e.kind else {
            return;
        };
        (a.dir, a.damage, a.owner, a.speed)
    };

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
        }

        let xt = e.c.x / 16;
        let yt = e.c.y / 16;
        let tile = g.tile_at(lvl, xt, yt);
        if !tiles::may_pass(g, &tile, lvl, xt, yt, e) && !tile.connects_to_water && tile.id != 16 {
            behavior::remove_entity(g, e);
        }
    }
}

/// Java `Arrow.render(screen)`.
pub fn arrow_render(_g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    let EntityKind::Arrow(a) = &e.kind else {
        return;
    };
    let xt = match a.dir {
        Direction::Left => 14,
        Direction::Up => 15,
        Direction::Down => 16,
        _ => 13,
    };
    let yt = 5;
    screen.render(e.c.x - 4, e.c.y - 4, xt + yt * 32, e.c.col, 0);
}

/// Java `Arrow.getData()`.
pub fn arrow_get_data(e: &Entity) -> String {
    let EntityKind::Arrow(a) = &e.kind else {
        return String::new();
    };
    format!("{}:{}:{}", a.owner, a.dir.ordinal(), a.damage)
}

/// Java `Spark.tick()`.
pub fn spark_tick(g: &mut Game, e: &mut Entity) {
    let owner = {
        let EntityKind::Spark(s) = &mut e.kind else {
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
            .map(|h| h.is_mob() && !matches!(h.kind, EntityKind::AirWizard(_)))
            .unwrap_or(false);
        if hurt_it {
            // Java `mob.hurt(owner, 1)` — attack dir from the owner's position
            let owner_pos = g.entities.get(owner).map(|o| (o.c.x, o.c.y));
            g.with_entity(hit_id, |mob, g| {
                let attack_dir = match owner_pos {
                    Some((ox, oy)) => Direction::get_direction(mob.c.x - ox, mob.c.y - oy),
                    None => Direction::None,
                };
                behavior::mob_hurt_by_eid(g, owner, mob, 1, attack_dir);
            });
        }
    }
}

/// Java `Spark.render(screen)`.
pub fn spark_render(g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    let EntityKind::Spark(s) = &e.kind else {
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
    ); // the spark
    screen.render(
        e.c.x - 4,
        e.c.y - 4 + 2,
        xt + yt * 32,
        color::BLACK,
        randmirror,
    ); // its shadow
}

/// Java `Spark.getData()`.
pub fn spark_get_data(e: &Entity) -> String {
    let EntityKind::Spark(s) = &e.kind else {
        return String::new();
    };
    s.owner.to_string()
}
