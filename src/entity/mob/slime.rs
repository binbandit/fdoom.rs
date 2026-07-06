//! Port of `fdoom.entity.mob.Slime`. Data + constructor; behavior in `slime` fns.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{MobAnims, compile_sprite_list};

use super::EnemyMobData;

// JAVA: slime sprites are a single row of two frames.
static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| vec![compile_sprite_list(0, 18, 2, 2, 0, 2)]);

pub const LVLCOLS: [i32; 4] = [
    color::get4(-1, 20, 40, 222),
    color::get4(-1, 100, 522, 555),
    color::get4(-1, 111, 444, 555),
    color::get4(-1, 0, 111, 224),
];

#[derive(Debug, Clone)]
pub struct SlimeData {
    pub enemy: EnemyMobData,
    /// jumpTimer, also acts as a rest timer before the next jump.
    pub jump_time: i32,
}

/// Java `new Slime(lvl)`.
pub fn new(g: &Game, lvl: i32) -> Entity {
    let diff_idx = g.settings.get_idx("diff");
    let (enemy, col) =
        EnemyMobData::with_default_lifetime(lvl, &SPRITES, &LVLCOLS, 1, true, 50, 60, 40, diff_idx);
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(
        c,
        EntityKind::Slime(SlimeData {
            enemy,
            jump_time: 0,
        }),
    )
}

fn data_mut(e: &mut Entity) -> &mut SlimeData {
    match &mut e.kind {
        EntityKind::Slime(d) => d,
        _ => panic!("entity is not a slime"),
    }
}

fn jump_time(e: &Entity) -> i32 {
    match &e.kind {
        EntityKind::Slime(d) => d.jump_time,
        _ => panic!("entity is not a slime"),
    }
}

/// Java `Slime.randomizeWalkDir(byChance)` — direction cannot be changed if the slime
/// is already jumping.
fn randomize_walk_dir(g: &mut Game, e: &mut Entity, by_chance: bool) {
    if jump_time(e) > 0 {
        return;
    }
    crate::entity::behavior::randomize_walk_dir(g, e, by_chance);
}

/// Java `Slime.tick()`.
///
/// Java Slime overrides `move()` (dir forced DOWN after every move) and
/// `randomizeWalkDir()` (no-op mid-jump), which the shared MobAi/EnemyMob tick bodies
/// invoke virtually; those bodies are inlined here (mirroring
/// `behavior::mobai_tick_base`/`enemy_mob_tick_base`) so the overrides apply.
pub fn tick(g: &mut Game, e: &mut Entity) {
    use crate::entity::Direction;
    use crate::entity::behavior::{
        get_closest_player, is_within, mob_tick_base, mobai_move, remove_entity,
    };

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
    // JAVA: Slime.move() — dir is forced DOWN after every move.
    if let Some(mob) = e.mob_mut() {
        mob.dir = Direction::Down;
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

    /* ----- Slime.tick() leaf ----- */
    // JAVA: jumpTime from 0 to -10 (or less) is the slime deciding where to jump;
    // 10 to 0 is it jumping.
    let (xa, ya) = e.mob_ai().map(|ai| (ai.xa, ai.ya)).unwrap_or((0, 0));
    let d = data_mut(e);
    if d.jump_time <= -10 && (xa != 0 || ya != 0) {
        d.jump_time = 10;
    }

    d.jump_time -= 1;
    if d.jump_time == 0 {
        if let Some(ai) = e.mob_ai_mut() {
            ai.xa = 0;
            ai.ya = 0;
        }
    }
}

/// Java `Slime.die()`.
pub fn die(g: &mut Game, e: &mut Entity) {
    let max = if g.is_mode("score") {
        2
    } else {
        4 - g.settings.get_idx("diff")
    };
    let slime = crate::item::registry::get(g, "slime");
    crate::entity::behavior::mobai_drop_items(g, e, 1, max, &[slime]);

    crate::entity::behavior::enemy_mob_die(g, e);
}

/// Java `Slime.render(screen)` — jump/ground sprite via walkDist, raised while jumping.
pub fn render(g: &mut Game, screen: &mut crate::gfx::Screen, e: &mut Entity) {
    let oldy = e.c.y;
    if jump_time(e) > 0 {
        if let Some(mob) = e.mob_mut() {
            mob.walk_dist = 8; // set to jumping sprite.
        }
        e.c.y -= 4; // raise up a bit.
    } else if let Some(mob) = e.mob_mut() {
        mob.walk_dist = 0; // set to ground sprite.
    }

    if let Some(mob) = e.mob_mut() {
        mob.dir = crate::entity::Direction::Down;
    }

    crate::entity::behavior::enemy_mob_render(g, screen, e);

    e.c.y = oldy;
}
