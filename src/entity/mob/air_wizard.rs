//! Port of `fdoom.entity.mob.AirWizard`. Data + constructor; behavior in `air_wizard` fns.
//!
//! Java's static `AirWizard.beaten` lives on `Game` (`g.air_wizard_beaten`).

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{MobAnims, compile_mob_sprite_animations};

use super::EnemyMobData;

static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| compile_mob_sprite_animations(8, 14));

#[derive(Debug, Clone)]
pub struct AirWizardData {
    pub enemy: EnemyMobData,
    pub secondform: bool,
    pub attack_delay: i32,
    pub attack_time: i32,
    pub attack_type: i32,
}

/// Java `new AirWizard(secondform)`.
pub fn new(g: &Game, secondform: bool) -> Entity {
    let diff_idx = g.settings.get_idx("diff");
    // JAVA: lvlcols is `new int[2]` (all zeros), lifetime is -1 (unlimited).
    let (mut enemy, _col) = EnemyMobData::new(
        if secondform { 2 } else { 1 },
        &SPRITES,
        &[0, 0],
        if secondform { 5000 } else { 2000 },
        false,
        16 * 8,
        -1,
        10,
        50,
        diff_idx,
    );
    if secondform {
        enemy.ai.mob.speed = 3;
    } else {
        enemy.ai.mob.speed = 2;
    }
    enemy.ai.mob.walk_time = 2;
    // top half color / bottom half color
    enemy.lvlcols[0] = if secondform {
        color::get4(-1, 0, 2, 46)
    } else {
        color::get4(-1, 100, 500, 555)
    };
    enemy.lvlcols[1] = if secondform {
        color::get4(-1, 0, 2, 46)
    } else {
        color::get4(-1, 100, 500, 532)
    };
    let mut c = EntityCommon::new(4, 3);
    c.col = enemy.lvlcols[(enemy.lvl - 1) as usize];
    Entity::new(
        c,
        EntityKind::AirWizard(AirWizardData {
            enemy,
            secondform,
            attack_delay: 0,
            attack_time: 0,
            attack_type: 0,
        }),
    )
}

fn data_mut(e: &mut Entity) -> &mut AirWizardData {
    match &mut e.kind {
        EntityKind::AirWizard(d) => d,
        _ => panic!("entity is not the air wizard"),
    }
}

fn data(e: &Entity) -> &AirWizardData {
    match &e.kind {
        EntityKind::AirWizard(d) => d,
        _ => panic!("entity is not the air wizard"),
    }
}

/// Java `AirWizard.tick()` — the boss attack AI.
pub fn tick(g: &mut Game, e: &mut Entity) {
    use crate::entity::Direction;
    use crate::entity::behavior::get_closest_player;

    if !crate::entity::behavior::enemy_mob_tick_base(g, e) {
        return;
    }

    if data(e).attack_delay > 0 {
        if let Some(ai) = e.mob_ai_mut() {
            ai.xa = 0;
            ai.ya = 0;
        }
        let attack_delay = data(e).attack_delay;
        let mut dir = (attack_delay - 45) / 4 % 4; // the direction of attack.
        dir = (dir * 2 % 4) + (dir / 2); // direction attack changes
        if attack_delay < 45 {
            dir = 0; // direction is reset, if attackDelay is less than 45; prepping for attack.
        }

        if let Some(mob) = e.mob_mut() {
            mob.dir = Direction::from_dir(dir);
        }

        let d = data_mut(e);
        d.attack_delay -= 1;
        if d.attack_delay == 0 {
            let secondform = d.secondform;
            let (health, max_health) = e.mob().map(|m| (m.health, m.max_health)).unwrap_or((0, 1));
            let d = data_mut(e);
            if health < max_health / 2 {
                d.attack_type = 1; // if at 1000 health (50%) or lower, attackType = 1
            }
            if health < max_health / 10 {
                d.attack_type = 2; // if at 200 health (10%) or lower, attackType = 2
            }
            // attackTime set to 120 or 180 (2 or 3 seconds, at default 60 ticks/sec)
            d.attack_time = 60 * (if secondform { 3 } else { 2 });
        }
        return; // skips the rest of the code (attackDelay must have been > 0)
    }

    if data(e).attack_time > 0 {
        if let Some(ai) = e.mob_ai_mut() {
            ai.xa = 0;
            ai.ya = 0;
        }
        let d = data_mut(e);
        d.attack_time -= 1; // attackTime will decrease by 1.
        let attack_time = d.attack_time;
        // assigns a local direction variable from the attack time.
        let dir = attack_time as f64 * 0.25 * (attack_time % 2 * 2 - 1) as f64;
        // speed is dependent on the attackType. (higher attackType, faster speeds)
        let speed = (if d.secondform { 1.2 } else { 0.7 }) + d.attack_type as f64 * 0.2;
        if let Some(lvl) = e.c.level {
            // adds a spark entity with the cosine and sine of dir times speed.
            let spark = crate::entity::projectile::new_spark(
                e.c.eid,
                e.c.x,
                e.c.y,
                dir.cos() * speed,
                dir.sin() * speed,
                &mut g.random,
            );
            g.level_mut(lvl).add(spark, lvl);
        }
        return; // skips the rest of the code (attackTime was > 0; ie we're attacking.)
    }

    let player_id = get_closest_player(g, e);

    if let Some(pid) = player_id {
        let random_walk_time = e.mob_ai().map(|ai| ai.random_walk_time).unwrap_or(0);
        // if there is a player around, and the walking is not random
        if random_walk_time == 0 {
            if let Some(player) = g.entities.get(pid) {
                let xd = player.c.x - e.c.x; // the horizontal distance between the player and the air wizard.
                let yd = player.c.y - e.c.y; // the vertical distance between the player and the air wizard.
                if xd * xd + yd * yd < 16 * 16 * 2 * 2 {
                    // Move away from the player if less than 2 blocks away
                    if let Some(ai) = e.mob_ai_mut() {
                        ai.xa = 0; // accelerations
                        ai.ya = 0;
                        // these four statements basically just find which direction is away from the player:
                        if xd < 0 {
                            ai.xa = 1;
                        }
                        if xd > 0 {
                            ai.xa = -1;
                        }
                        if yd < 0 {
                            ai.ya = 1;
                        }
                        if yd > 0 {
                            ai.ya = -1;
                        }
                    }
                } else if xd * xd + yd * yd > 16 * 16 * 15 * 15 {
                    // 15 squares away: drags the airwizard to the player, maintaining relative position.
                    let hypot = ((xd * xd + yd * yd) as f64).sqrt();
                    let newxd = (xd as f64 * ((16 * 16 * 15 * 15) as f64).sqrt() / hypot) as i32;
                    let newyd = (yd as f64 * ((16 * 16 * 15 * 15) as f64).sqrt() / hypot) as i32;
                    let (px, py) = (player.c.x, player.c.y);
                    e.c.x = px - newxd;
                    e.c.y = py - newyd;
                }
            }
        }
    }

    if let Some(pid) = player_id {
        let random_walk_time = e.mob_ai().map(|ai| ai.random_walk_time).unwrap_or(0);
        if random_walk_time == 0 {
            if let Some(player) = g.entities.get(pid) {
                let xd = player.c.x - e.c.x; // x dist to player
                let yd = player.c.y - e.c.y; // y dist to player
                // if a random number, 0-3, equals 0, and the player is less than 50
                // blocks away, and attackDelay and attackTime equal 0...
                if g.random.next_int_bound(4) == 0
                    && xd * xd + yd * yd < 50 * 50
                    && data(e).attack_delay == 0
                    && data(e).attack_time == 0
                {
                    // ...then set attackDelay to 120 (2 seconds at default 60 ticks/sec)
                    data_mut(e).attack_delay = 60 * 2;
                }
            }
        }
    }
}

/// Java `AirWizard.doHurt(damage, attackDir)` — starts an attack when hurt while idle.
///
/// NOTE: `behavior::do_hurt` currently routes the AirWizard through the generic
/// `mobai_do_hurt`; it needs an `AirWizard` dispatch arm calling this function so the
/// attack-on-hurt trigger applies.
pub fn do_hurt(g: &mut Game, e: &mut Entity, damage: i32, attack_dir: crate::entity::Direction) {
    crate::entity::behavior::mobai_do_hurt(g, e, damage, attack_dir);
    let d = data_mut(e);
    if d.attack_delay == 0 && d.attack_time == 0 {
        d.attack_delay = 60 * 2;
    }
}

/// Java `AirWizard.touchedBy(entity)` — deals 1 (or 2, second form) damage, unlike
/// EnemyMob's `lvl * (hard ? 2 : 1)`, and does NOT call super.
///
/// NOTE: `behavior::touched_by` currently routes the AirWizard to the shared EnemyMob
/// touchedBy; it needs an `AirWizard` dispatch arm calling this function instead.
pub fn touched_by(g: &mut Game, this_e: &mut Entity, by: &mut Entity) {
    if by.is_player() {
        // if the entity is the Player, then deal them 1 or 2 damage points.
        let dmg = if data(this_e).secondform { 2 } else { 1 };
        let attack_dir = crate::entity::behavior::get_attack_dir(this_e, by);
        super::player_behavior::hurt_by_mob(g, by, this_e, dmg, attack_dir);
    }
}

/// Java `AirWizard.render(screen)` — split-color sprite plus the health-percent text.
pub fn render(_g: &mut Game, screen: &mut crate::gfx::Screen, e: &mut Entity) {
    use crate::gfx::font;

    let d = data(e);
    let secondform = d.secondform;
    let attack_type = d.attack_type;
    let Some(mob) = e.mob() else { return };

    let xo = e.c.x - 8; // the horizontal location to start drawing the sprite
    let yo = e.c.y - 11; // the vertical location to start drawing the sprite

    // top half color / bottom half color
    let mut col1 = if secondform {
        color::get4(-1, 0, 2, 46)
    } else {
        color::get4(-1, 100, 500, 555)
    };
    let mut col2 = if secondform {
        color::get4(-1, 0, 2, 46)
    } else {
        color::get4(-1, 100, 500, 532)
    };

    if attack_type == 1 && mob.tick_time / 5 % 4 == 0
        || attack_type == 2 && mob.tick_time / 3 % 2 == 0
    {
        // change colors.
        col1 = if secondform {
            color::get4(-1, 2, 0, 46)
        } else {
            color::get4(-1, 500, 100, 555)
        };
        col2 = if secondform {
            color::get4(-1, 2, 0, 46)
        } else {
            color::get4(-1, 500, 100, 532)
        };
    }

    if mob.hurt_time > 0 {
        // turn the sprite white, momentarily.
        col1 = color::WHITE;
        col2 = color::WHITE;
    }

    let cur_sprite = &mob.sprites[mob.dir.get_dir() as usize][((mob.walk_dist >> 3) & 1) as usize];
    cur_sprite.render_row_color(0, screen, xo, yo, col1);
    cur_sprite.render_row_color(1, screen, xo, yo + 8, col2);

    let mut textcol = color::get(-1, 40);
    let mut textcol2 = color::get(-1, 10);
    let percent = mob.health / (mob.max_health / 100);
    let mut h = format!("{percent}%");

    if percent < 1 {
        h = "1%".to_string();
    }

    if percent < 16 {
        textcol = color::get(-1, 400);
        textcol2 = color::get(-1, 100);
    } else if percent < 51 {
        textcol = color::get(-1, 440);
        textcol2 = color::get(-1, 110);
    }
    let textwidth = font::text_width(&h);
    font::draw(
        &h,
        screen,
        (e.c.x - textwidth / 2) + 1,
        e.c.y - 17,
        textcol2,
    );
    font::draw(&h, screen, e.c.x - textwidth / 2, e.c.y - 18, textcol);
}

/// Java `AirWizard.die()` — score, notifications, beaten flag / costume unlock.
pub fn die(g: &mut Game, e: &mut Entity) {
    use crate::core::io::sound::Sound;

    let secondform = data(e).secondform;

    if let Some(lvl) = e.c.level {
        let score_mode = g.is_mode("score");
        let players = crate::level::get_players(g, lvl);
        // if the player is still here, give the player 100K or 500K points.
        for pid in players {
            g.with_entity(pid, |p, _g| {
                p.player_mut()
                    .add_score(if secondform { 500000 } else { 100000 }, score_mode);
            });
        }
    }

    g.play_sound(Sound::BossDeath); // play boss-death sound.

    if !secondform {
        g.notify_all("Air Wizard: Defeated!");
        if !g.air_wizard_beaten {
            g.notify_all_tick("The Dungeon is now open!", -400);
        }
        g.air_wizard_beaten = true;
    } else {
        g.notify_all("Air Wizard II: Defeated!");
        if !g.settings.get("unlockedskin").as_bool() {
            g.notify_all_tick("A costume lies on the ground...", -200);
        }
        g.settings.set("unlockedskin", true);
        crate::saveload::save::save_prefs(g); // JAVA: new Save()
    }

    crate::entity::behavior::enemy_mob_die(g, e); // calls the die() method in EnemyMob.java
}
