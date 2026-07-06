//! Behavior of `fdoom.entity.furniture.Tnt`.

use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::mob::player_behavior;
use crate::entity::{Direction, Entity, EntityKind, behavior};
use crate::gfx::{Rectangle, Screen, color};
use crate::item::Item;
use crate::level;

use super::tnt::{BLAST_DAMAGE, BLAST_RADIUS, FUSE_TIME};

/// Java 300ms `explodeTimer`, in game ticks (18 ticks at 60/s).
const EXPLODE_RESTORE_TICKS: i32 = 18;

/// Java `Tnt.tick()`.
pub fn tick(g: &mut Game, e: &mut Entity) {
    // JAVA: the post-explosion tile restore ran on a 300ms swing Timer after the entity
    // was removed; here the exploded entity itself hosts the countdown (see TntData) and
    // is removed once the tiles are restored.
    if let EntityKind::Tnt(t) = &mut e.kind {
        if let Some(ticks) = t.explode_ticks_left {
            if ticks > 1 {
                t.explode_ticks_left = Some(ticks - 1);
            } else {
                // Java `actionPerformed(e)` — does the (tile part of the) explosion.
                if let Some(lvl) = e.c.level {
                    let xt = e.c.x >> 4;
                    let yt = (e.c.y - 2) >> 4;
                    let hole = g.tiles.get("hole");
                    level::set_area_tiles(g, lvl, xt, yt, 1, &hole, 0, false);
                }
                behavior::remove_entity(g, e);
            }
            return;
        }
    }

    super::behavior::tick(g, e);

    let fuse_ready = {
        let EntityKind::Tnt(t) = &mut e.kind else {
            return;
        };
        if t.fuse_lit {
            t.ftik += 1;
            t.ftik >= FUSE_TIME
        } else {
            false
        }
    };

    if fuse_ready {
        // blow up
        let Some(lvl) = e.c.level else { return };
        let entities_in_range = level::get_entities_in_rect(
            g,
            lvl,
            &Rectangle::new(
                e.c.x,
                e.c.y,
                BLAST_RADIUS * 2,
                BLAST_RADIUS * 2,
                Rectangle::CENTER_DIMS,
            ),
        );

        for eid in entities_in_range {
            g.with_entity(eid, |other, g| {
                let dist =
                    f64::hypot((other.c.x - e.c.x) as f64, (other.c.y - e.c.y) as f64) as f32;
                let dmg = (BLAST_DAMAGE as f32 * (1.0 - dist / BLAST_RADIUS as f32)) as i32 + 1;
                if other.is_mob() {
                    // JAVA: mob.hurt(this, dmg) → doHurt(dmg, getAttackDir(tnt, mob))
                    let attack_dir = behavior::get_attack_dir(e, other);
                    behavior::do_hurt(g, other, dmg, attack_dir);
                    if other.is_player() {
                        // JAVA: Player.hurt(Tnt tnt, int dmg) also calls payStamina(dmg * 2)
                        player_behavior::pay_stamina(other, dmg * 2);
                    }
                }
                if let EntityKind::Tnt(tnt) = &mut other.kind {
                    if !tnt.fuse_lit {
                        tnt.fuse_lit = true;
                        g.play_sound(Sound::Fuse);
                        tnt.ftik = FUSE_TIME * 2 / 3;
                    }
                }
            });
        }

        g.play_sound(Sound::Explode);

        let xt = e.c.x >> 4;
        let yt = (e.c.y - 2) >> 4;

        let explode = g.tiles.get("explode");
        level::set_area_tiles(g, lvl, xt, yt, 1, &explode, 0, false);

        // JAVA: levelSave = level; explodeTimer.start(); super.remove(); — see the note at
        // the top of this fn.
        if let EntityKind::Tnt(t) = &mut e.kind {
            t.explode_ticks_left = Some(EXPLODE_RESTORE_TICKS);
        }
    }
}

/// Java `Tnt.render(screen)`.
pub fn render(g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    let EntityKind::Tnt(t) = &e.kind else { return };
    if t.explode_ticks_left.is_some() {
        return; // JAVA: the entity was already removed at this point — render nothing.
    }
    if t.fuse_lit {
        let col_fctr = 100 * ((t.ftik % 15) / 5) + 200;
        // JAVA: only `col` is set; Furniture.render draws sprite.color, so the flash has
        // no visible effect — preserved.
        e.c.col = color::get4(-1, col_fctr, col_fctr + 100, 555);
    }
    super::behavior::render(g, screen, e);
}

/// Java `Tnt.interact(player, heldItem, attackDir)` — lights the fuse.
pub fn interact(
    g: &mut Game,
    e: &mut Entity,
    player: &mut Entity,
    item: &mut Option<Item>,
    attack_dir: Direction,
) -> bool {
    let _ = (player, item, attack_dir);
    if let EntityKind::Tnt(t) = &mut e.kind {
        if !t.fuse_lit {
            t.fuse_lit = true;
            g.play_sound(Sound::Fuse);
            return true;
        }
    }

    false
}
