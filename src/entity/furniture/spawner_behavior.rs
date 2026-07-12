//! Behavior of `fdoom.entity.furniture.Spawner`.

use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::{Direction, Entity, EntityKind, behavior, mob, particle};
use crate::gfx::color;
use crate::item::{Item, ItemKind, PotionType, ToolType};
use crate::level;
use crate::level::tile::dispatch as tiles;

use super::spawner::{
    ACTIVE_RADIUS, MAX_SPAWN_INTERVAL, MIN_MOB_SPAWN_CHANCE, MIN_SPAWN_INTERVAL, max_mob_level,
};

/// Java `Spawner.initMob(m)`.
fn init_mob(e: &mut Entity, m: Entity) {
    let col = m.c.col;
    let (lvl, max_lvl) = match m.enemy_mob() {
        Some(em) => (em.lvl, max_mob_level(&m)),
        None => (1, 1),
    };
    if let EntityKind::Spawner(s) = &mut e.kind {
        *s.mob = m;
        s.furniture.sprite.color = col; // the spawner is tinted like its mob
        s.lvl = lvl;
        s.max_mob_level = max_lvl;
    }
    e.c.col = col;
}

/// Java `mob.getClass().getConstructor(int.class).newInstance(lvl)` (enemy mobs) /
/// `mob.getClass().newInstance()` (others) — builds a fresh mob of the template's kind.
fn new_mob_instance(g: &Game, template: &Entity, lvl: i32) -> Option<Entity> {
    Some(match &template.kind {
        EntityKind::Cow(_) => mob::cow::new(g),
        EntityKind::Deer(_) => mob::deer::new(g),
        EntityKind::Pig(_) => mob::pig::new(g),
        EntityKind::Sheep(_) => mob::sheep::new(g),
        EntityKind::GlowWorm(_) => mob::glow_worm::new(g),
        EntityKind::Zombie(_) => mob::zombie::new(g, lvl),
        EntityKind::Snake(sd) => mob::snake::new_variant(g, sd.variant, lvl),
        EntityKind::Knight(_) => mob::knight::new(g, lvl),
        EntityKind::MarshLurker(_) => mob::marsh_lurker::new(g, lvl),
        EntityKind::FeralHound(_) => mob::feral_hound::new(g, lvl),
        EntityKind::StoneGolem(_) => mob::stone_golem::new(g, lvl),
        EntityKind::NightWisp(_) => mob::night_wisp::new(g, lvl),
        EntityKind::Ghost(_) => mob::ghost::new(g, lvl),
        _ => return None,
    })
}

/// Java `Spawner.tick()`.
pub fn tick(g: &mut Game, e: &mut Entity) {
    super::behavior::tick(g, e);

    let spawn_tick = {
        let EntityKind::Spawner(s) = &mut e.kind else {
            return;
        };
        s.spawn_tick -= 1;
        s.spawn_tick
    };
    if spawn_tick <= 0 {
        if let Some(lvl) = e.c.level {
            let (mob_count, max_mob_count) = {
                let l = g.level(lvl);
                (l.mob_count, l.max_mob_count)
            };
            // this forms a quadratic function that determines the mob spawn chance.
            let chance = (MIN_MOB_SPAWN_CHANCE as f64 * (mob_count as f64).powi(2)
                / (max_mob_count as f64).powi(2)) as i32;
            if chance <= 0 || g.random.next_int_bound(chance) == 0 {
                try_spawn(g, e);
            }
        }
        reset_spawn_interval(g, e);
    }
}

/// Java `Spawner.resetSpawnInterval()`.
fn reset_spawn_interval(g: &mut Game, e: &mut Entity) {
    if let EntityKind::Spawner(s) = &mut e.kind {
        s.spawn_tick = g
            .random
            .next_int_bound(MAX_SPAWN_INTERVAL - MIN_SPAWN_INTERVAL + 1)
            + MIN_SPAWN_INTERVAL;
    }
}

/// Java `Spawner.trySpawn()`.
fn try_spawn(g: &mut Game, e: &mut Entity) {
    let Some(lvl) = e.c.level else { return };
    if g.level(lvl).mob_count >= g.level(lvl).max_mob_count {
        return; // can't spawn more entities
    }

    let Some(pid) = behavior::get_closest_player(g, e) else {
        return;
    };
    let (px, py) = match g.entities.get(pid) {
        Some(p) => (p.c.x, p.c.y),
        None => return,
    };
    let xd = px - e.c.x;
    let yd = py - e.c.y;

    if xd * xd + yd * yd > ACTIVE_RADIUS * ACTIVE_RADIUS {
        return;
    }

    let template = match &e.kind {
        EntityKind::Spawner(s) => (*s.mob).clone(),
        _ => return,
    };
    let is_enemy = template.is_enemy_mob();
    let template_lvl = template.enemy_mob().map(|em| em.lvl).unwrap_or(1);
    // non-mob templates can't be instantiated; bail with a log
    let Some(mut newmob) = new_mob_instance(g, &template, template_lvl) else {
        println!("Spawner ERROR: could not spawn mob; error initializing mob instance:");
        return;
    };

    let pos = (e.c.x >> 4, e.c.y >> 4);
    let area_positions = level::get_area_tile_positions(g, lvl, pos.0, pos.1, 1, 1);
    let mut valid_positions = Vec::new();
    for p in area_positions {
        let tile = g.tile_at(lvl, p.x, p.y);
        let may_pass = tiles::may_pass(g, &tile, lvl, p.x, p.y, &newmob);
        let light = tiles::get_light_radius(g, &tile, lvl, p.x, p.y);
        if !(!may_pass || is_enemy && light > 0) {
            valid_positions.push(p);
        }
    }

    if valid_positions.is_empty() {
        return; // cannot spawn mob.
    }

    let spawn_pos = valid_positions[g.random.next_int_bound(valid_positions.len() as i32) as usize];

    newmob.c.x = spawn_pos.x << 4;
    newmob.c.y = spawn_pos.y << 4;

    g.level_mut(lvl).add(newmob, lvl);
    g.play_sound(Sound::MonsterHurt);
    for _ in 0..6 {
        let rand_x = g.random.next_int_bound(16);
        let rand_y = g.random.next_int_bound(12);
        let fire = particle::new_fire_particle(e.c.x - 8 + rand_x, e.c.y - 6 + rand_y);
        g.level_mut(lvl).add(fire, lvl);
    }
}

/// Java `Spawner.interact(player, item, attackDir)`.
pub fn interact(
    g: &mut Game,
    e: &mut Entity,
    player: &mut Entity,
    item: &mut Option<Item>,
    attack_dir: Direction,
) -> bool {
    let _ = attack_dir;
    if let Some(it) = item {
        if let ItemKind::Tool {
            ttype,
            level: tool_level,
            ..
        } = it.kind
        {
            // any tool damages a spawner; a pickaxe just does it much faster
            g.play_sound(Sound::MonsterHurt);

            let health = match &e.kind {
                EntityKind::Spawner(s) => s.health,
                _ => return false,
            };
            let dmg = if g.is_mode("creative") {
                health
            } else {
                let mut dmg = tool_level + g.random.next_int_bound(2);

                if ttype == ToolType::Pickaxe {
                    dmg += g.random.next_int_bound(5) + 2;
                }

                if player
                    .player()
                    .potioneffects
                    .contains_key(&PotionType::Haste)
                {
                    dmg *= 2;
                }
                dmg
            };

            let health = {
                let EntityKind::Spawner(s) = &mut e.kind else {
                    return false;
                };
                s.health -= dmg;
                s.health
            };
            if let Some(lvl) = e.c.level {
                let text = particle::new_text_particle(
                    &dmg.to_string(),
                    e.c.x,
                    e.c.y,
                    color::get4(-1, 200, 300, 400),
                    &mut g.random,
                );
                g.level_mut(lvl).add(text, lvl);
            }
            if health <= 0 {
                if let Some(lvl) = e.c.level {
                    g.level_mut(lvl).remove(e.c.eid);
                }
                g.play_sound(Sound::PlayerDeath);
                let score_mode = g.is_mode("score");
                player.player_mut().add_score(500, score_mode);
            }

            return true;
        }

        if matches!(it.kind, ItemKind::PowerGlove) && g.is_mode("creative") {
            if let Some(lvl) = e.c.level {
                g.level_mut(lvl).remove(e.c.eid);
            }
            let pd = player.player_mut();
            let active_is_glove = pd
                .active_item
                .as_ref()
                .is_some_and(|a| matches!(a.kind, ItemKind::PowerGlove));
            if !active_is_glove {
                // stash the current active item (if any) before the glove takes over
                if let Some(active) = pd.active_item.take() {
                    pd.inventory.add_at(0, active);
                }
            }
            pd.active_item = Some(crate::item::registry::new_furniture_item(e.clone()));
            return true;
        }

        false
    } else {
        // empty-handed interact falls through to `use`
        use_furniture(g, e, player)
    }
}

/// Java `Spawner.use(player)` — creative-mode mob level cycling.
pub fn use_furniture(g: &mut Game, e: &mut Entity, player: &mut Entity) -> bool {
    let _ = player;
    let is_enemy = match &e.kind {
        EntityKind::Spawner(s) => s.mob.is_enemy_mob(),
        _ => return false,
    };
    if g.is_mode("creative") && is_enemy {
        let (new_lvl, template) = {
            let EntityKind::Spawner(s) = &mut e.kind else {
                return false;
            };
            s.lvl += 1;
            if s.lvl > s.max_mob_level {
                s.lvl = 1;
            }
            (s.lvl, (*s.mob).clone())
        };
        if let Some(newmob) = new_mob_instance(g, &template, new_lvl) {
            init_mob(e, newmob);
        }
        return true;
    }

    false
}
