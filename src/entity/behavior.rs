//! The Java `Entity`/`Mob`/`MobAi`/`EnemyMob`/`PassiveMob`/`Furniture` base-class
//! behaviors, plus the per-kind dispatch hubs (tick/render/touchedBy/...).
//!
//! Entities are ticked with the take-out pattern: the entity is removed from the arena,
//! so `e` and `g` are independently mutable. Interactions with other entities take those
//! out too (`Game::with_entity`).

use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::core::updater::Time;
use crate::entity::mob::MobAiData;
use crate::entity::{Direction, Entity, EntityKind};
use crate::gfx::{color, Rectangle, Screen};
use crate::level;
use crate::level::tile::dispatch as tiles;

impl Game {
    /// The take-out helper (see PORTING.md). Returns None if the entity is absent
    /// (already taken out or deleted).
    pub fn with_entity<R>(&mut self, eid: i32, f: impl FnOnce(&mut Entity, &mut Game) -> R) -> Option<R> {
        let mut e = self.entities.take(eid)?;
        let r = f(&mut e, self);
        self.entities.put_back(e);
        Some(r)
    }

    /// Java `Game.player` (panicking, like Java's implicit non-null uses). Note: while the
    /// player is taken out (its own tick), use the `&mut Entity` you already have.
    pub fn player(&self) -> &Entity {
        self.entities.get(self.player_id).expect("player entity missing")
    }

    pub fn player_mut(&mut self) -> &mut Entity {
        self.entities.get_mut(self.player_id).expect("player entity missing")
    }

    /// Java `Game.player != null && !removed` style checks where the player may be absent.
    pub fn try_player(&self) -> Option<&Entity> {
        self.entities.get(self.player_id)
    }
}

/* --------------------------- Entity base (Entity.java) --------------------------- */

/// Java `Entity.remove()` — marks removed and queues level removal.
pub fn remove_entity(g: &mut Game, e: &mut Entity) {
    e.c.removed = true;
    if let Some(lvl) = e.c.level {
        if g.levels[lvl].is_some() {
            g.level_mut(lvl).remove(e.c.eid);
        }
    }
}

/// Java `Entity.die()` — per-kind override dispatch; default is remove().
pub fn die(g: &mut Game, e: &mut Entity) {
    match &e.kind {
        EntityKind::Player(_) => super::mob::player_behavior::die(g, e),
        EntityKind::Cow(_) => super::mob::cow::die(g, e),
        EntityKind::Pig(_) => super::mob::pig::die(g, e),
        EntityKind::Sheep(_) => super::mob::sheep::die(g, e),
        EntityKind::GlowWorm(_) => super::mob::glow_worm::die(g, e),
        EntityKind::Zombie(_) => super::mob::zombie::die(g, e),
        EntityKind::Slime(_) => super::mob::slime::die(g, e),
        EntityKind::Creeper(_) => super::mob::creeper::die(g, e),
        EntityKind::Skeleton(_) => super::mob::skeleton::die(g, e),
        EntityKind::Snake(_) => super::mob::snake::die(g, e),
        EntityKind::Knight(_) => super::mob::knight::die(g, e),
        EntityKind::AirWizard(_) => super::mob::air_wizard::die(g, e),
        _ => remove_entity(g, e),
    }
}

/// Java `Entity.isSolid()` (overridden to false by the free-floating kinds).
pub fn is_solid(e: &Entity) -> bool {
    !matches!(
        e.kind,
        EntityKind::ItemEntity(_)
            | EntityKind::Arrow(_)
            | EntityKind::Spark(_)
            | EntityKind::Particle(_)
            | EntityKind::TextParticle(_)
    )
}

/// Java `Entity.blocks(e)` — furniture blocks everything.
pub fn blocks(this: &Entity, e: &Entity) -> bool {
    if this.is_furniture() {
        return true;
    }
    is_solid(this) && is_solid(e)
}

/// Java `Entity.canSwim()`.
pub fn can_swim(e: &Entity) -> bool {
    match &e.kind {
        EntityKind::Player(_) => true,
        EntityKind::AirWizard(a) => a.secondform,
        _ => false,
    }
}

/// Java `Entity.canWool()`.
pub fn can_wool(e: &Entity) -> bool {
    match &e.kind {
        EntityKind::Player(_) => true,
        EntityKind::AirWizard(_) => false, // overrides MobAi's true
        _ => e.is_mob_ai() || e.is_furniture(),
    }
}

/// Java `Entity.getLightRadius()`.
pub fn get_light_radius(e: &Entity) -> i32 {
    match &e.kind {
        EntityKind::Player(_) => super::mob::player_behavior::get_light_radius(e),
        EntityKind::Lantern(l) => l.lantern_type.light(),
        EntityKind::GlowWorm(_) => 2, // TODO(port:entity-behavior): verify GlowWorm radius
        _ => 0,
    }
}

/// Java `Entity.isWithin(tileRadius, other)`.
pub fn is_within(e: &Entity, tile_radius: i32, other: &Entity) -> bool {
    if e.c.level.is_none() || other.c.level.is_none() || e.c.level != other.c.level {
        return false;
    }
    let distance = f64::hypot((e.c.x - other.c.x) as f64, (e.c.y - other.c.y) as f64).abs();
    (distance.round() as i64 >> 4) as i32 <= tile_radius
}

/// Java `Entity.move(xa, ya)` — returns whether the entity was unimpeded.
pub fn entity_move(g: &mut Game, e: &mut Entity, xa: i32, ya: i32) -> bool {
    if g.saving || (xa == 0 && ya == 0) {
        return true; // pretend that it kept moving
    }
    let mut stopped = true;
    if entity_move2(g, e, xa, 0) {
        stopped = false;
    }
    if entity_move2(g, e, 0, ya) {
        stopped = false;
    }
    if !stopped {
        if let Some(lvl) = e.c.level {
            let xt = e.c.x >> 4;
            let yt = e.c.y >> 4;
            let tile = g.tile_at(lvl, xt, yt);
            tiles::stepped_on(g, &tile, lvl, xt, yt, e);
        }
    }
    !stopped
}

/// Java `Entity.move2(xa, ya)` — one-axis movement with tile and entity collision.
pub fn entity_move2(g: &mut Game, e: &mut Entity, xa: i32, ya: i32) -> bool {
    if xa == 0 && ya == 0 {
        return true; // was not stopped
    }
    let Some(lvl) = e.c.level else { return false };

    let interact = true;

    // tile coordinates of the sprite's corners, before...
    let xto0 = (e.c.x - e.c.xr) >> 4;
    let yto0 = (e.c.y - e.c.yr) >> 4;
    let xto1 = (e.c.x + e.c.xr) >> 4;
    let yto1 = (e.c.y + e.c.yr) >> 4;

    // ...and after movement
    let xt0 = ((e.c.x + xa) - e.c.xr) >> 4;
    let yt0 = ((e.c.y + ya) - e.c.yr) >> 4;
    let xt1 = ((e.c.x + xa) + e.c.xr) >> 4;
    let yt1 = ((e.c.y + ya) + e.c.yr) >> 4;

    for yt in yt0..=yt1 {
        for xt in xt0..=xt1 {
            if xt >= xto0 && xt <= xto1 && yt >= yto0 && yt <= yto1 {
                continue; // skip tiles the entity is already touching
            }
            let tile = g.tile_at(lvl, xt, yt);
            if interact {
                tiles::bumped_into(g, &tile, lvl, xt, yt, e); // used in tiles like cactus
            }
            if !tiles::may_pass(g, &tile, lvl, xt, yt, e) {
                return false;
            }
        }
    }

    // entities currently intersected (before moving)
    let was_inside = level::get_entities_in_rect(g, lvl, &e.c.bounds());

    let (xr, yr) = (e.c.xr, e.c.yr);
    let new_rect = Rectangle::new(e.c.x + xa, e.c.y + ya, xr * 2, yr * 2, Rectangle::CENTER_DIMS);
    let is_inside = level::get_entities_in_rect(g, lvl, &new_rect);

    // touch each entity about to be touched
    if interact {
        for other_id in &is_inside {
            if *other_id == e.c.eid {
                continue; // touching yourself doesn't count
            }
            // JAVA: if the other is a Player (and we are not), *we* get touchedBy(player);
            // otherwise the other gets touchedBy(us).
            let other_is_player = g.entities.get(*other_id).map(|o| o.is_player()).unwrap_or(false);
            if other_is_player && !e.is_player() {
                g.with_entity(*other_id, |player, g| {
                    touched_by(g, e, player);
                });
            } else {
                g.with_entity(*other_id, |other, g| {
                    touched_by(g, other, e);
                });
            }
        }
    }

    for other_id in &is_inside {
        if was_inside.contains(other_id) || *other_id == e.c.eid {
            continue;
        }
        let Some(other) = g.entities.get(*other_id) else { continue };
        if blocks(other, e) {
            return false; // the other entity prevents movement
        }
    }

    // finally, the entity moves!
    e.c.x += xa;
    e.c.y += ya;
    true
}

/// Java `Entity.touchedBy(entity)` — `this_e` is the entity being touched; `by` is the
/// moving entity. Per-kind override dispatch.
pub fn touched_by(g: &mut Game, this_e: &mut Entity, by: &mut Entity) {
    match &this_e.kind {
        // Furniture.touchedBy: player pushes furniture
        _ if this_e.is_furniture() => {
            if by.is_player() {
                super::furniture::behavior::try_push(g, this_e, by);
            }
        }
        EntityKind::ItemEntity(_) => super::item_entity_behavior::touched_by(g, this_e, by),
        EntityKind::Zombie(_)
        | EntityKind::Slime(_)
        | EntityKind::Skeleton(_)
        | EntityKind::Snake(_)
        | EntityKind::Knight(_)
        | EntityKind::AirWizard(_) =>

            // EnemyMob.touchedBy: hurt the player, damage based on lvl
            enemy_touched_by(g, this_e, by),
        EntityKind::Creeper(_) => super::mob::creeper::touched_by(g, this_e, by),
        _ => {}
    }
}

/// Java `EnemyMob.touchedBy(entity)`.
fn enemy_touched_by(g: &mut Game, this_e: &mut Entity, by: &mut Entity) {
    if by.is_player() {
        let lvl_dmg = this_e.enemy_mob().map(|em| em.lvl).unwrap_or(1);
        let hard = g.settings.get("diff").as_str() == "Hard";
        let dmg = lvl_dmg * if hard { 2 } else { 1 };
        let attack_dir = get_attack_dir(this_e, by);
        super::mob::player_behavior::hurt_by_mob(g, by, this_e, dmg, attack_dir);
    }
}

/// Java `Mob.getAttackDir(attacker, hurt)`.
pub fn get_attack_dir(attacker: &Entity, hurt: &Entity) -> Direction {
    Direction::get_direction(hurt.c.x - attacker.c.x, hurt.c.y - attacker.c.y)
}

/// Java `Entity.interact(player, item, attackDir)` — per-kind dispatch. Returns true if
/// the interaction was handled. `item` is the player's active item (may be "null").
pub fn entity_interact(
    g: &mut Game,
    this_e: &mut Entity,
    player: &mut Entity,
    item: &mut Option<crate::item::Item>,
    attack_dir: Direction,
) -> bool {
    // JAVA Entity.interact: if item != null, return item.interact(player, this, attackDir)
    if let Some(it) = item {
        return crate::item::interact::item_interact_entity(g, it, player, this_e, attack_dir);
    }
    false
}

/// Java `Entity.getClosestPlayer()` for a (taken-out) entity.
pub fn get_closest_player(g: &Game, e: &Entity) -> Option<i32> {
    if e.is_player() {
        return Some(e.c.eid);
    }
    let lvl = e.c.level?;
    level::get_closest_player(g, lvl, e.c.x, e.c.y)
}

/* ------------------------------ tick/render dispatch ------------------------------ */

/// Java `entity.tick()` — the per-kind virtual dispatch.
pub fn entity_tick(g: &mut Game, e: &mut Entity) {
    match &e.kind {
        EntityKind::Player(_) => super::mob::player_behavior::tick(g, e),
        EntityKind::Cow(_) => super::mob::cow::tick(g, e),
        EntityKind::Pig(_) => super::mob::pig::tick(g, e),
        EntityKind::Sheep(_) => super::mob::sheep::tick(g, e),
        EntityKind::GlowWorm(_) => super::mob::glow_worm::tick(g, e),
        EntityKind::Zombie(_) => super::mob::zombie::tick(g, e),
        EntityKind::Slime(_) => super::mob::slime::tick(g, e),
        EntityKind::Creeper(_) => super::mob::creeper::tick(g, e),
        EntityKind::Skeleton(_) => super::mob::skeleton::tick(g, e),
        EntityKind::Snake(_) => super::mob::snake::tick(g, e),
        EntityKind::Knight(_) => super::mob::knight::tick(g, e),
        EntityKind::AirWizard(_) => super::mob::air_wizard::tick(g, e),
        EntityKind::ItemEntity(_) => super::item_entity_behavior::tick(g, e),
        EntityKind::Arrow(_) => super::projectile_behavior::arrow_tick(g, e),
        EntityKind::Spark(_) => super::projectile_behavior::spark_tick(g, e),
        EntityKind::Particle(_) => super::particle_behavior::tick(g, e),
        EntityKind::TextParticle(_) => super::particle_behavior::text_tick(g, e),
        EntityKind::Furniture(_)
        | EntityKind::Chest(_)
        | EntityKind::Bed(_)
        | EntityKind::Crafter(_)
        | EntityKind::Lantern(_) => super::furniture::behavior::tick(g, e),
        EntityKind::DeathChest(_) => super::furniture::death_chest_behavior::tick(g, e),
        EntityKind::DungeonChest(_) => super::furniture::behavior::tick(g, e),
        EntityKind::Spawner(_) => super::furniture::spawner_behavior::tick(g, e),
        EntityKind::Tnt(_) => super::furniture::tnt_behavior::tick(g, e),
    }
}

/// Java `entity.render(screen)` — the per-kind virtual dispatch.
pub fn entity_render(g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    match &e.kind {
        EntityKind::Player(_) => super::mob::player_behavior::render(g, screen, e),
        EntityKind::Cow(_) | EntityKind::Pig(_) | EntityKind::Sheep(_) => {
            passive_mob_render(g, screen, e)
        }
        EntityKind::GlowWorm(_) => super::mob::glow_worm::render(g, screen, e),
        EntityKind::Zombie(_)
        | EntityKind::Skeleton(_)
        | EntityKind::Snake(_)
        | EntityKind::Knight(_) => enemy_mob_render(g, screen, e),
        EntityKind::Slime(_) => super::mob::slime::render(g, screen, e),
        EntityKind::Creeper(_) => super::mob::creeper::render(g, screen, e),
        EntityKind::AirWizard(_) => super::mob::air_wizard::render(g, screen, e),
        EntityKind::ItemEntity(_) => super::item_entity_behavior::render(g, screen, e),
        EntityKind::Arrow(_) => super::projectile_behavior::arrow_render(g, screen, e),
        EntityKind::Spark(_) => super::projectile_behavior::spark_render(g, screen, e),
        EntityKind::Particle(_) => super::particle_behavior::render(g, screen, e),
        EntityKind::TextParticle(_) => super::particle_behavior::text_render(g, screen, e),
        EntityKind::DeathChest(_) => super::furniture::death_chest_behavior::render(g, screen, e),
        _ if e.is_furniture() => super::furniture::behavior::render(g, screen, e),
        _ => {}
    }
}

/* ------------------------------- Mob base (Mob.java) ------------------------------- */

/// Java `Mob.tick()` — the shared part every mob tick calls first. Returns false if the
/// mob was removed by it.
pub fn mob_tick_base(g: &mut Game, e: &mut Entity) -> bool {
    let Some(mob) = e.mob_mut() else { return false };
    mob.tick_time += 1;

    if e.c.removed {
        return false;
    }

    if let Some(lvl) = e.c.level {
        let standing = g.tile_at(lvl, e.c.x >> 4, e.c.y >> 4);
        if standing.name == "LAVA" {
            // hurt ourselves, sourced from the lava tile
            mob_hurt_tile(g, e, &standing, e.c.x, e.c.y, 4);
        }
    }

    if e.mob().map(|m| m.health).unwrap_or(1) <= 0 {
        die(g, e);
        return false;
    }
    if let Some(mob) = e.mob_mut() {
        if mob.hurt_time > 0 {
            mob.hurt_time -= 1;
        }
    }

    // knockback processing
    let (mut xd, mut yd) = (0, 0);
    if let Some(mob) = e.mob_mut() {
        if mob.x_knockback != 0 {
            xd = (mob.x_knockback as f64 / 2.0).ceil() as i32;
            mob.x_knockback -= mob.x_knockback / mob.x_knockback.abs();
        }
        if mob.y_knockback != 0 {
            yd = (mob.y_knockback as f64 / 2.0).ceil() as i32;
            mob.y_knockback -= mob.y_knockback / mob.y_knockback.abs();
        }
    }
    mob_move(g, e, xd, yd, false);
    !e.c.removed
}

/// Java `Mob.move(xa, ya)` / the private `move(xa, ya, changeDir)`.
pub fn mob_move(g: &mut Game, e: &mut Entity, xa: i32, ya: i32, change_dir: bool) -> bool {
    if e.c.level.is_none() {
        return false; // stopped b/c there's no level to move in!
    }

    {
        let is_player = e.is_player();
        let swimming = is_swimming(g, e);
        let wooling = is_wooling(g, e);
        let Some(mob) = e.mob_mut() else { return false };
        // these return true b/c the mob is still technically moving (just slower)
        if mob.tick_time % 2 == 0 && (swimming || (!is_player && wooling)) {
            return true;
        }
        if mob.walk_time > 1 && mob.tick_time % mob.walk_time == 0 {
            return true;
        }
    }

    let mut moved = true;

    let hurt_time = e.mob().map(|m| m.hurt_time).unwrap_or(0);
    if hurt_time == 0 || e.is_player() {
        let (mut xa, mut ya) = (xa, ya);
        if xa != 0 || ya != 0 {
            if let Some(mob) = e.mob_mut() {
                if change_dir {
                    mob.dir = Direction::get_direction(xa, ya);
                }
                mob.walk_dist += 1;
            }
        }

        // can't move in the direction being knocked back from
        if let Some(mob) = e.mob() {
            if mob.x_knockback != 0 {
                let same_sign = (xa as f64).signum() == (mob.x_knockback as f64).signum();
                xa = if xa != 0 && !same_sign { xa } else { 0 };
            }
            if mob.y_knockback != 0 {
                let same_sign = (ya as f64).signum() == (mob.y_knockback as f64).signum();
                ya = if ya != 0 && !same_sign { ya } else { 0 };
            }
        }

        moved = entity_move(g, e, xa, ya);
    }

    moved
}

/// Java `Mob.isWooling()`.
pub fn is_wooling(g: &Game, e: &Entity) -> bool {
    let Some(lvl) = e.c.level else { return false };
    g.tile_at(lvl, e.c.x >> 4, e.c.y >> 4).name == "WOOL"
}

/// Java `Mob.isLight()`.
pub fn mob_is_light(g: &Game, e: &Entity) -> bool {
    let Some(lvl) = e.c.level else { return false };
    level::is_light(g, lvl, e.c.x >> 4, e.c.y >> 4)
}

/// Java `Mob.isSwimming()`.
pub fn is_swimming(g: &Game, e: &Entity) -> bool {
    let Some(lvl) = e.c.level else { return false };
    let tile = g.tile_at(lvl, e.c.x >> 4, e.c.y >> 4);
    tile.name == "WATER" || tile.name == "LAVA"
}

/// Java `Mob.hurt(Tile tile, x, y, damage)`.
pub fn mob_hurt_tile(g: &mut Game, e: &mut Entity, tile: &level::tile::TileDef, x: i32, y: i32, damage: i32) {
    let Some(mob) = e.mob() else { return };
    let attack_dir = Direction::from_dir(mob.dir.get_dir() ^ 1); // opposite of our direction
    let lava_immune = tile.name == "LAVA"
        && e.is_player()
        && e.player().potioneffects.contains_key(&crate::item::PotionType::Lava);
    if !lava_immune {
        let lvl = e.c.level.unwrap_or(g.current_level);
        let dir = if tiles::may_pass(g, tile, lvl, x, y, e) { Direction::None } else { attack_dir };
        do_hurt(g, e, damage, dir);
    }
}

/// Java `Mob.hurt(Mob mob, damage, attackDir)` — `attacker` is the damage source.
pub fn mob_hurt_by_mob(g: &mut Game, attacker: &mut Entity, e: &mut Entity, damage: i32, attack_dir: Direction) {
    if attacker.is_player() && g.is_mode("creative") && attacker.c.eid != e.c.eid {
        let health = e.mob().map(|m| m.health).unwrap_or(0);
        do_hurt(g, e, health, attack_dir); // kill the mob instantly
    } else {
        do_hurt(g, e, damage, attack_dir);
    }
}

/// Java `Mob.hurt(Mob mob, damage, attackDir)` where the attacker is referenced by eid
/// (used by projectiles, whose owner may not be takeable).
pub fn mob_hurt_by_eid(g: &mut Game, attacker_eid: i32, e: &mut Entity, damage: i32, attack_dir: Direction) {
    let attacker_is_player = g.entities.get(attacker_eid).map(|a| a.is_player()).unwrap_or(false);
    if attacker_is_player && g.is_mode("creative") && attacker_eid != e.c.eid {
        let health = e.mob().map(|m| m.health).unwrap_or(0);
        do_hurt(g, e, health, attack_dir); // kill the mob instantly
    } else {
        do_hurt(g, e, damage, attack_dir);
    }
}

/// Java `Mob.doHurt(damage, attackDir)` — with the Player and MobAi overrides dispatched.
pub fn do_hurt(g: &mut Game, e: &mut Entity, damage: i32, attack_dir: Direction) {
    if e.is_player() {
        super::mob::player_behavior::do_hurt(g, e, damage, attack_dir);
        return;
    }
    if e.is_mob_ai() {
        mobai_do_hurt(g, e, damage, attack_dir);
        return;
    }
    mob_do_hurt_base(g, e, damage, attack_dir);
}

/// The Mob.java `doHurt` body.
pub fn mob_do_hurt_base(g: &mut Game, e: &mut Entity, damage: i32, attack_dir: Direction) {
    let _ = g;
    if e.c.removed {
        return;
    }
    let Some(mob) = e.mob_mut() else { return };
    if mob.hurt_time > 0 {
        return;
    }
    mob.health -= damage;
    mob.x_knockback = attack_dir.x() * 6;
    mob.y_knockback = attack_dir.y() * 6;
    mob.hurt_time = 10;
}

/// Java `MobAi.doHurt` — plays monsterHurt when a player is near, adds a text particle.
pub fn mobai_do_hurt(g: &mut Game, e: &mut Entity, damage: i32, attack_dir: Direction) {
    let hurt_time = e.mob().map(|m| m.hurt_time).unwrap_or(0);
    if e.c.removed || hurt_time > 0 {
        return;
    }

    if let Some(player_id) = get_closest_player(g, e) {
        if let Some(player) = g.entities.get(player_id) {
            let xd = player.c.x - e.c.x;
            let yd = player.c.y - e.c.y;
            if xd * xd + yd * yd < 80 * 80 {
                g.play_sound(Sound::MonsterHurt);
            }
        }
    }
    if let Some(lvl) = e.c.level {
        let p = super::particle::new_text_particle(&damage.to_string(), e.c.x, e.c.y, color::RED, &mut g.random);
        g.level_mut(lvl).add(p, lvl);
    }

    mob_do_hurt_base(g, e, damage, attack_dir);
}

/// Java `Mob.heal(heal)`.
pub fn heal(g: &mut Game, e: &mut Entity, heal: i32) {
    let Some(mob) = e.mob() else { return };
    if mob.hurt_time > 0 {
        return;
    }
    if let Some(lvl) = e.c.level {
        let p = super::particle::new_text_particle(&heal.to_string(), e.c.x, e.c.y, color::GREEN, &mut g.random);
        g.level_mut(lvl).add(p, lvl);
    }
    if let Some(mob) = e.mob_mut() {
        mob.health += heal;
        if mob.health > mob.max_health {
            mob.health = mob.max_health;
        }
    }
}

/* ------------------------------ MobAi base (MobAi.java) ------------------------------ */

/// Java `MobAi.skipTick()`.
fn skip_tick(ai: &MobAiData) -> bool {
    ai.slowtick && (ai.mob.tick_time + 1) % 4 == 0
}

/// Java `MobAi.tick()` — shared AI movement. Returns false if the mob was removed.
pub fn mobai_tick_base(g: &mut Game, e: &mut Entity) -> bool {
    if !mob_tick_base(g, e) {
        return false;
    }

    {
        let Some(ai) = e.mob_ai_mut() else { return false };
        if ai.lifetime > 0 {
            ai.age += 1;
            if ai.age > ai.lifetime {
                remove_entity(g, e);
                return false;
            }
        }
    }

    if e.c.level.is_some() {
        // slowtick if a player with the Time potion effect is within 8 tiles
        let mut found_player = false;
        if let Some(lvl) = e.c.level {
            for pid in level::get_players(g, lvl) {
                if let Some(p) = g.entities.get(pid) {
                    if is_within(p, 8, e) && p.player().potioneffects.contains_key(&crate::item::PotionType::Time) {
                        found_player = true;
                        break;
                    }
                }
            }
        }
        if let Some(ai) = e.mob_ai_mut() {
            ai.slowtick = found_player;
        }
    }

    if let Some(ai) = e.mob_ai() {
        if skip_tick(ai) {
            return true;
        }
    }

    let (xa, ya, speed) = {
        let Some(ai) = e.mob_ai() else { return false };
        (ai.xa, ai.ya, ai.mob.speed)
    };
    if !mobai_move(g, e, xa * speed, ya * speed) {
        if let Some(ai) = e.mob_ai_mut() {
            ai.xa = 0;
            ai.ya = 0;
        }
    }

    let chance = e.mob_ai().map(|ai| ai.random_walk_chance).unwrap_or(1);
    if g.random.next_int_bound(chance) == 0 {
        randomize_walk_dir(g, e, true);
    }

    if let Some(ai) = e.mob_ai_mut() {
        if ai.random_walk_time > 0 {
            ai.random_walk_time -= 1;
        }
    }
    !e.c.removed
}

/// Java `MobAi.move` (client check is always false here).
pub fn mobai_move(g: &mut Game, e: &mut Entity, xa: i32, ya: i32) -> bool {
    mob_move(g, e, xa, ya, true)
}

/// Java `MobAi.render(screen)` — the shared mob render.
pub fn mobai_render(screen: &mut Screen, e: &mut Entity) {
    let Some(mob) = e.mob() else { return };
    let xo = e.c.x - 8;
    let yo = e.c.y - 11;

    let color = if mob.hurt_time > 0 { color::WHITE } else { e.c.col };

    let dir_idx = mob.dir.get_dir() as usize;
    let row = &mob.sprites[dir_idx.min(mob.sprites.len() - 1)];
    let cur_sprite = &row[((mob.walk_dist >> 3) as usize) % row.len()];
    cur_sprite.render_color(screen, xo, yo, color);
}

/// Java `MobAi.randomizeWalkDir(byChance)` — with the PassiveMob override.
pub fn randomize_walk_dir(g: &mut Game, e: &mut Entity, by_chance: bool) {
    let is_passive = e.passive_mob().is_some();
    let Some(ai) = e.mob_ai_mut() else { return };

    if is_passive {
        // PassiveMob override
        if ai.xa == 0 && ai.ya == 0 && g.random.next_int_bound(5) == 0
            || by_chance
            || g.random.next_int_bound(ai.random_walk_chance) == 0
        {
            ai.random_walk_time = ai.random_walk_duration;
            // multiplier at the end ups the chance of not moving by 50%
            ai.xa = (g.random.next_int_bound(3) - 1) * g.random.next_int_bound(2);
            ai.ya = (g.random.next_int_bound(3) - 1) * g.random.next_int_bound(2);
        }
        return;
    }

    if !by_chance && g.random.next_int_bound(ai.random_walk_chance) != 0 {
        return;
    }
    ai.random_walk_time = ai.random_walk_duration;
    ai.xa = g.random.next_int_bound(3) - 1;
    ai.ya = g.random.next_int_bound(3) - 1;
}

/// Java `MobAi.dropItem(mincount, maxcount, items...)`.
pub fn mobai_drop_items(g: &mut Game, e: &Entity, mincount: i32, maxcount: i32, items: &[crate::item::Item]) {
    let Some(lvl) = e.c.level else { return };
    let count = g.random.next_int_bound(maxcount - mincount + 1) + mincount;
    for _ in 0..count {
        for item in items {
            level::drop_item(g, lvl, e.c.x, e.c.y, item.clone());
        }
    }
}

/// Java `MobAi.checkStartPos(level, x, y, playerDist, soloRadius)`.
pub fn mobai_check_start_pos(g: &Game, lvl: usize, x: i32, y: i32, player_dist: i32, solo_radius: i32) -> bool {
    if let Some(pid) = level::get_closest_player(g, lvl, x, y) {
        if let Some(player) = g.entities.get(pid) {
            let xd = player.c.x - x;
            let yd = player.c.y - y;
            if xd * xd + yd * yd < player_dist * player_dist {
                return false;
            }
        }
    }

    let r = g.level(lvl).monster_density * solo_radius; // get no-mob radius
    if !level::get_entities_in_rect(g, lvl, &Rectangle::new(x, y, r * 2, r * 2, Rectangle::CENTER_DIMS)).is_empty() {
        return false;
    }

    let tile = g.tile_at(lvl, x >> 4, y >> 4);
    tiles::may_spawn(&tile)
}

/// Java `MobAi.die(points, multAdd)`.
pub fn mobai_die(g: &mut Game, e: &mut Entity, points: i32, mult_add: i32) {
    if let Some(lvl) = e.c.level {
        let score_mode = g.is_mode("score");
        for pid in level::get_players(g, lvl) {
            if pid == e.c.eid {
                continue;
            }
            g.with_entity(pid, |p, _g| {
                let pd = p.player_mut();
                pd.add_score(points, score_mode);
                if mult_add != 0 {
                    pd.add_multiplier(mult_add, score_mode);
                }
            });
        }
    }
    remove_entity(g, e);
}

/* --------------------------- EnemyMob base (EnemyMob.java) --------------------------- */

/// Java `EnemyMob.tick()` — chase the player. Returns false if removed.
pub fn enemy_mob_tick_base(g: &mut Game, e: &mut Entity) -> bool {
    if !mobai_tick_base(g, e) {
        return false;
    }

    if let Some(pid) = get_closest_player(g, e) {
        let sleeping = g.bed_state.players_awake == 0;
        let random_walk_time = e.mob_ai().map(|ai| ai.random_walk_time).unwrap_or(0);
        if !sleeping && random_walk_time <= 0 {
            if let Some(player) = g.entities.get(pid) {
                let xd = player.c.x - e.c.x;
                let yd = player.c.y - e.c.y;
                let detect_dist = e.enemy_mob().map(|em| em.detect_dist).unwrap_or(0);
                if xd * xd + yd * yd < detect_dist * detect_dist {
                    let sig0 = 1; // prevents mobs from bobbing up and down
                    if let Some(ai) = e.mob_ai_mut() {
                        ai.xa = 0;
                        ai.ya = 0;
                        if xd < sig0 {
                            ai.xa = -1;
                        }
                        if xd > sig0 {
                            ai.xa = 1;
                        }
                        if yd < sig0 {
                            ai.ya = -1;
                        }
                        if yd > sig0 {
                            ai.ya = 1;
                        }
                    }
                } else {
                    randomize_walk_dir(g, e, false);
                }
            }
        }
    }
    !e.c.removed
}

/// Java `EnemyMob.render(screen)`.
pub fn enemy_mob_render(_g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    if let Some(em) = e.enemy_mob() {
        e.c.col = em.lvlcols[(em.lvl - 1) as usize];
    }
    mobai_render(screen, e);
}

/// Java `EnemyMob.die()` — 50 points per level, +1 multiplier.
pub fn enemy_mob_die(g: &mut Game, e: &mut Entity) {
    let lvl = e.enemy_mob().map(|em| em.lvl).unwrap_or(1);
    mobai_die(g, e, 50 * lvl, 1);
}

/// Java `EnemyMob.checkStartPos(level, x, y)`.
pub fn enemy_check_start_pos(g: &Game, lvl: usize, x: i32, y: i32) -> bool {
    let depth = g.level(lvl).depth;
    let r = if depth == -4 {
        if g.is_mode("score") { 22 } else { 15 }
    } else {
        13
    };

    if !mobai_check_start_pos(g, lvl, x, y, 60, r) {
        return false;
    }

    let xt = x >> 4;
    let yt = y >> 4;

    let t = g.tile_at(lvl, xt, yt);
    if depth == -4 {
        if t.name != "OBSIDIAN" {
            return false;
        }
        true
    } else if t.name != "STONE DOOR" && t.name != "WOOD DOOR" && t.name != "OBSIDIAN DOOR" && t.name != "WHEAT" && t.name != "FARMLAND" {
        // prevents mobs from spawning on lit tiles, farms, or doors (unless in dungeon)
        !level::is_light(g, lvl, xt, yt)
    } else {
        false
    }
}

/* -------------------------- PassiveMob base (PassiveMob.java) -------------------------- */

/// Java `PassiveMob.render(screen)`.
pub fn passive_mob_render(_g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    if let Some(pm) = e.passive_mob() {
        e.c.col = pm.color;
    }
    mobai_render(screen, e);
}

/// Java `PassiveMob.die()` — 15 points.
pub fn passive_mob_die(g: &mut Game, e: &mut Entity) {
    mobai_die(g, e, 15, 0);
}

/// Java `PassiveMob.checkStartPos(level, x, y)`.
pub fn passive_check_start_pos(g: &Game, lvl: usize, x: i32, y: i32) -> bool {
    let r = (if g.is_mode("score") { 22 } else { 15 })
        + (if g.get_time() == Time::Night { 0 } else { 5 });

    if !mobai_check_start_pos(g, lvl, x, y, 80, r) {
        return false;
    }

    let tile = g.tile_at(lvl, x >> 4, y >> 4);
    tile.name == "GRASS" || tile.name == "FLOWER"
}
