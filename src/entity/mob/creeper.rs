//! Port of `fdoom.entity.mob.Creeper`. Data + constructor; behavior in `creeper` fns.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{MobAnims, Sprite, compile_sprite_list};

use super::EnemyMobData;

// JAVA: list of 3 frames; walking = {1, 2}, standing = {0, 0}; sprites = [standing].
static LIST: LazyLock<Vec<Sprite>> = LazyLock::new(|| compile_sprite_list(4, 18, 2, 2, 0, 3));
pub static WALKING: LazyLock<Vec<Sprite>> =
    LazyLock::new(|| vec![LIST[1].clone(), LIST[2].clone()]);
pub static STANDING: LazyLock<Vec<Sprite>> =
    LazyLock::new(|| vec![LIST[0].clone(), LIST[0].clone()]);
static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| vec![STANDING.clone()]);

pub const LVLCOLS: [i32; 4] = [
    color::get4(-1, 20, 40, 30),
    color::get4(-1, 200, 262, 232),
    color::get4(-1, 200, 272, 222),
    color::get4(-1, 200, 292, 282),
];

pub const MAX_FUSE_TIME: i32 = 60;
pub const BLAST_RADIUS: i32 = 60;
pub const BLAST_DAMAGE: i32 = 10;

#[derive(Debug, Clone)]
pub struct CreeperData {
    pub enemy: EnemyMobData,
    pub fuse_time: i32,
    pub fuse_lit: bool,
}

/// Java `new Creeper(lvl)`.
pub fn new(g: &Game, lvl: i32) -> Entity {
    // FIX: clamp to the lvlcols range — Java indexed lvlcols[lvl-1] unchecked and an
    // out-of-range level (e.g. from a hand-edited save) crashed the game.
    let lvl = lvl.clamp(1, LVLCOLS.len() as i32);
    let diff_idx = g.settings.get_idx("diff");
    let (enemy, col) = EnemyMobData::simple(lvl, &SPRITES, &LVLCOLS, 10, 50, diff_idx);
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(
        c,
        EntityKind::Creeper(CreeperData {
            enemy,
            fuse_time: 0,
            fuse_lit: false,
        }),
    )
}

fn data_mut(e: &mut Entity) -> &mut CreeperData {
    match &mut e.kind {
        EntityKind::Creeper(d) => d,
        _ => panic!("entity is not a creeper"),
    }
}

fn data(e: &Entity) -> &CreeperData {
    match &e.kind {
        EntityKind::Creeper(d) => d,
        _ => panic!("entity is not a creeper"),
    }
}

/// Java `Creeper.tick()`.
///
/// Java Creeper overrides `move()` (dir forced DOWN; walkDist reset when standing
/// still), which the shared MobAi tick body invokes virtually; the MobAi/EnemyMob tick
/// bodies are inlined here (mirroring `behavior::mobai_tick_base`/
/// `enemy_mob_tick_base`) so the override applies.
pub fn tick(g: &mut Game, e: &mut Entity) {
    use crate::core::io::sound::Sound;
    use crate::entity::Direction;
    use crate::entity::behavior::{
        do_hurt, get_closest_player, is_within, mob_hurt_by_eid, mob_tick_base, mobai_move,
        randomize_walk_dir, remove_entity,
    };
    use crate::gfx::Point;

    /* ----- MobAi.tick() body ----- */
    if !mob_tick_base(g, e) {
        return;
    }

    {
        let Some(ai) = e.mob_ai_mut() else { return };
        if ai.lifetime > 0 {
            ai.age += 1;
            if ai.age > ai.lifetime {
                remove_entity(g, e);
                return;
            }
        }
    }

    if e.c.level.is_some() {
        let mut found_player = false;
        if let Some(lvl) = e.c.level {
            for pid in crate::level::get_players(g, lvl) {
                if let Some(p) = g.entities.get(pid) {
                    if is_within(p, 8, e)
                        && p.player()
                            .potioneffects
                            .contains_key(&crate::item::PotionType::Time)
                    {
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

    // Java `skipTick()` (private in MobAi).
    if let Some(ai) = e.mob_ai() {
        if ai.slowtick && (ai.mob.tick_time + 1) % 4 == 0 {
            return;
        }
    }

    let (xa, ya, speed) = {
        let Some(ai) = e.mob_ai() else { return };
        (ai.xa, ai.ya, ai.mob.speed)
    };
    let moved = mobai_move(g, e, xa * speed, ya * speed);
    // JAVA: Creeper.move() — dir forced DOWN; walkDist reset when not moving.
    if let Some(mob) = e.mob_mut() {
        mob.dir = Direction::Down;
        if xa * speed == 0 && ya * speed == 0 {
            mob.walk_dist = 0;
        }
    }
    if !moved {
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
    if e.c.removed {
        return;
    }

    /* ----- EnemyMob.tick() body ----- */
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
    if e.c.removed {
        return;
    }

    /* ----- Creeper.tick() leaf ----- */
    if data(e).fuse_time > 0 {
        let d = data_mut(e);
        d.fuse_time -= 1; // fuse getting shorter...
        if let Some(ai) = e.mob_ai_mut() {
            ai.xa = 0;
            ai.ya = 0;
        }
    } else if data(e).fuse_lit {
        // fuseLit is set to true when fuseTime is set to max, so this happens after
        // fuseTime hits zero, while fuse is lit: blow up
        if let Some(ai) = e.mob_ai_mut() {
            ai.xa = 0;
            ai.ya = 0;
        }

        let mut hurt_one = false; // tells if any players were hurt
        let diff_idx = g.settings.get_idx("diff");
        let easy = g.settings.get("diff").as_str() == "Easy";
        let Some(lvl) = e.c.level else { return };

        // JAVA: level.getEntitiesOfClass(Mob.class) includes this creeper itself, so it
        // self-damages too; the taken-out self is handled explicitly (pd = 0).
        {
            let dmg = BLAST_DAMAGE + diff_idx;
            do_hurt(g, e, dmg, Direction::None); // getAttackDir(this, this) == NONE
        }

        let mob_ids: Vec<i32> = g
            .entities
            .entities_on_level(lvl)
            .filter(|o| o.is_mob())
            .map(|o| o.c.eid)
            .collect();
        for oid in mob_ids {
            let (ex, ey, eid) = (e.c.x, e.c.y, e.c.eid);
            g.with_entity(oid, |other, g| {
                let pdx = (other.c.x - ex).abs();
                let pdy = (other.c.y - ey).abs();
                if pdx < BLAST_RADIUS && pdy < BLAST_RADIUS {
                    let pd = ((pdx * pdx + pdy * pdy) as f32).sqrt();
                    let dmg =
                        (BLAST_DAMAGE as f32 * (1.0 - pd / BLAST_RADIUS as f32)) as i32 + diff_idx;
                    let attack_dir = Direction::get_direction(other.c.x - ex, other.c.y - ey);
                    mob_hurt_by_eid(g, eid, other, dmg, attack_dir);
                    if other.is_player() {
                        super::player_behavior::pay_stamina(other, dmg * if easy { 1 } else { 2 });
                        hurt_one = true;
                    }
                }
            });
        }

        if hurt_one {
            g.play_sound(Sound::Explode);

            // figure out which tile the mob died on
            let xt = e.c.x >> 4;
            let yt = (e.c.y - 2) >> 4;

            // change tile to an appropriate crater: sets all tiles within a certain
            // radius to a hole, unless they have a Spawner on them or stairs (stairs
            // check happens in Level). All entities on the reset tiles which are not
            // allowed to occupy a hole tile are then removed (or killed, for mobs).
            let creeper_lvl = e.enemy_mob().map(|em| em.lvl).unwrap_or(1);
            let radius = creeper_lvl * 2 / 3;
            let entities_in_range = crate::level::get_entities_in_tiles(
                g,
                lvl,
                xt - radius,
                yt - radius,
                xt + radius,
                yt + radius,
            );
            let tile_positions =
                crate::level::get_area_tile_positions(g, lvl, xt, yt, radius, radius);

            let mut skip_entities: Vec<Point> = Vec::new();
            for oid in &entities_in_range {
                if let Some(other) = g.entities.get(*oid) {
                    if matches!(other.kind, EntityKind::Spawner(_)) {
                        skip_entities.push(Point::new(other.c.x >> 4, other.c.y >> 4));
                    }
                }
            }

            let hole = g.tiles.get("hole");
            if skip_entities.is_empty() {
                crate::level::set_area_tiles(g, lvl, xt, yt, radius, &hole, 0, false);
            } else {
                for pos in &tile_positions {
                    let mut matched = false;
                    for sp in &skip_entities {
                        if sp == pos {
                            matched = true;
                            break;
                        }
                    }
                    if !matched {
                        crate::level::set_area_tiles(g, lvl, pos.x, pos.y, 0, &hole, 0, false);
                    }
                }
            }

            for oid in &entities_in_range {
                if *oid == e.c.eid {
                    continue;
                }
                g.with_entity(*oid, |other, g| {
                    let epos = Point::new(other.c.x >> 4, other.c.y >> 4);

                    for p in &tile_positions {
                        if *p != epos {
                            continue;
                        }

                        let tile = g.tile_at(lvl, p.x, p.y);
                        if !crate::level::tile::dispatch::may_pass(g, &tile, lvl, p.x, p.y, other) {
                            crate::entity::behavior::die(g, other);
                        }
                    }
                });
            }

            // dying now kind of kills everything. the super class will take care of it.
            die(g, e);
        } else {
            let d = data_mut(e);
            d.fuse_time = 0;
            d.fuse_lit = false;
        }
    }
}

/// Java `Creeper.render(screen)` — fuse flash + standing/walking sprite swap.
pub fn render(_g: &mut Game, screen: &mut crate::gfx::Screen, e: &mut Entity) {
    // JAVA: Creeper.render mutates the instance lvlcols copy for the fuse flash.
    {
        let (fuse_lit, fuse_time) = {
            let d = data(e);
            (d.fuse_lit, d.fuse_time)
        };
        let Some(em) = e.enemy_mob_mut() else { return };
        let idx = (em.lvl - 1) as usize;
        if fuse_lit && fuse_time % 6 == 0 {
            em.lvlcols[idx] = color::get(-1, 252);
        } else {
            em.lvlcols[idx] = LVLCOLS[idx];
        }
    }

    // JAVA: sprites[0] = walkDist == 0 ? standing : walking; then EnemyMob.render →
    // MobAi.render, inlined here because the sprite row swap mutates a Java static.
    let Some(em) = e.enemy_mob() else { return };
    e.c.col = em.lvlcols[(em.lvl - 1) as usize];

    let Some(mob) = e.mob() else { return };
    let xo = e.c.x - 8;
    let yo = e.c.y - 11;
    let col = if mob.hurt_time > 0 {
        color::WHITE
    } else {
        e.c.col
    };

    let row: &[Sprite] = if mob.walk_dist == 0 {
        &STANDING
    } else {
        &WALKING
    };
    let cur_sprite = &row[((mob.walk_dist >> 3) as usize) % row.len()];
    cur_sprite.render_color(screen, xo, yo, col);
}

/// Java `Creeper.touchedBy(entity)` — lights the fuse and hurts the player.
pub fn touched_by(g: &mut Game, this_e: &mut Entity, by: &mut Entity) {
    use crate::core::io::sound::Sound;

    if by.is_player() {
        if data(this_e).fuse_time == 0 {
            g.play_sound(Sound::Fuse);
            let d = data_mut(this_e);
            d.fuse_time = MAX_FUSE_TIME;
            d.fuse_lit = true;
        }
        let attack_dir = crate::entity::behavior::get_attack_dir(this_e, by);
        super::player_behavior::hurt_by_mob(g, by, this_e, 1, attack_dir);
    }
}

/// Java `Creeper.die()`.
pub fn die(g: &mut Game, e: &mut Entity) {
    let max = 4 - g.settings.get_idx("diff");
    let gunpowder = crate::item::registry::get(g, "Gunpowder");
    crate::entity::behavior::mobai_drop_items(g, e, 1, max, &[gunpowder]);

    crate::entity::behavior::enemy_mob_die(g, e);
}
