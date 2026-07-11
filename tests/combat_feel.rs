//! Combat-feel juice (playtest #1) + night threat legibility (playtest #3):
//! - attacking at 0 stamina is inert (no tile damage, stamina never below 0) but
//!   cued (gray breath puff, soft sound);
//! - a successful tile hit spawns material puffs and arms the swing-sweep flash;
//! - a whiff spawns nothing;
//! - player hurt sets the white-flash state (and keeps the damage numbers);
//! - hostile mobs render the warm eye-glint pixels at night / in the dark.

use std::sync::Arc;

use fdoom::core::updater::Time;
use fdoom::entity::behavior::entity_render;
use fdoom::entity::mob::player::PLAYER_HURT_TIME;
use fdoom::entity::mob::{player_behavior, zombie};
use fdoom::entity::{Direction, EntityKind};
use fdoom::gfx::{Screen, screen};
use fdoom::testutil::TestWorld;

/// Count entities of a kind on the current level, live or still queued.
fn count_kind(tw: &TestWorld, pred: impl Fn(&EntityKind) -> bool) -> usize {
    let lvl = tw.current_level;
    let queued = tw
        .level(lvl)
        .entities_to_add
        .iter()
        .filter(|e| pred(&e.kind))
        .count();
    let live = tw
        .entities
        .ids_on_level(lvl)
        .into_iter()
        .filter(|id| tw.entities.get(*id).map(|e| pred(&e.kind)).unwrap_or(false))
        .count();
    queued + live
}

fn count_particles(tw: &TestWorld) -> usize {
    count_kind(tw, |k| matches!(k, EntityKind::Particle(_)))
}

/// One attack-key press, platform-style (press + tick, release + tick).
fn press_attack(tw: &mut TestWorld) {
    tw.input.key_toggled("SPACE", true);
    tw.tick();
    tw.input.key_toggled("SPACE", false);
    tw.tick();
}

#[test]
fn winded_attack_is_inert_but_cued() {
    let mut tw = TestWorld::infinite().seed(11).build();
    let (tx, ty) = tw.place("tree", 0, 1); // the player spawns facing Down
    let lvl = tw.current_level;
    let tree_id = tw.tile_at(lvl, tx, ty).id;
    let dmg_before = tw.level(lvl).get_data(tx, ty);
    tw.player_mut().player_mut().stamina = 0;

    for _ in 0..3 {
        press_attack(&mut tw);
    }

    assert_eq!(
        tw.player().player().stamina,
        0,
        "winded attacks must never push stamina below 0"
    );
    assert_eq!(tw.tile_at(lvl, tx, ty).id, tree_id, "tile unchanged");
    assert_eq!(
        tw.level(lvl).get_data(tx, ty),
        dmg_before,
        "no tile damage lands at 0 stamina"
    );
    assert_eq!(
        count_kind(&tw, |k| matches!(k, EntityKind::TextParticle(_))),
        0,
        "no damage numbers at 0 stamina"
    );
    assert!(
        count_particles(&tw) >= 1,
        "the winded cue leaves a gray breath puff"
    );
}

#[test]
fn tile_hit_spawns_material_puffs_and_arms_swing_flash() {
    let mut tw = TestWorld::infinite().seed(11).build();
    let (tx, ty) = tw.place("tree", 0, 1);
    let lvl = tw.current_level;
    tw.player_mut().player_mut().stamina = 10;
    let before = count_particles(&tw);

    tw.input.key_toggled("SPACE", true);
    tw.tick();
    tw.input.key_toggled("SPACE", false);

    let pd = tw.player().player();
    assert!(pd.attack_time > 0, "swing in progress");
    assert!(pd.swing_flash > 0, "melee swing arms the sweep flash");
    let after = count_particles(&tw);
    assert!(
        after >= before + 2,
        "tile hit spawns material puffs ({before} -> {after})"
    );
    assert!(tw.level(lvl).get_data(tx, ty) > 0, "the tree took damage");
}

#[test]
fn whiff_spawns_no_puff() {
    let mut tw = TestWorld::infinite().seed(11).build();
    tw.place("grass", 0, 1); // nothing hurtable in the facing tile
    tw.player_mut().player_mut().stamina = 10;
    let before = count_particles(&tw);

    tw.input.key_toggled("SPACE", true);
    tw.tick();
    tw.input.key_toggled("SPACE", false);

    assert_eq!(
        count_particles(&tw),
        before,
        "an air punch stays visually silent (no impact puff)"
    );
}

#[test]
fn player_hurt_sets_flash_state_and_keeps_damage_numbers() {
    let mut tw = TestWorld::infinite().seed(11).build();
    let pid = tw.player_id;

    tw.g.with_entity(pid, |e, g| {
        player_behavior::do_hurt(g, e, 2, Direction::Left)
    });

    let pd = tw.player().player();
    assert_eq!(pd.mob.hurt_time, PLAYER_HURT_TIME, "hurt window opened");
    assert!(
        pd.mob.hurt_time > PLAYER_HURT_TIME - 10,
        "inside the white-flash window (render paints the player white here)"
    );
    assert!(
        count_kind(&tw, |k| matches!(k, EntityKind::TextParticle(_))) >= 1,
        "the damage number particle still spawns"
    );
}

/// The two warm glint pixels of fx/eye_glints.png — a true-color cell, so a direct
/// (ungraded) entity render must emit them literally. The cell renders at
/// `(x - 4, y - 8)` with the lit pixels at cell offsets (+2, +3) and (+5, +3).
const GLINT_RGB: i32 = 0xFFD830;

#[test]
fn night_hostiles_emit_eye_glint_pixels() {
    let mut tw = TestWorld::infinite().seed(11).build();
    let lvl = tw.current_level;
    for dy in -4..=4 {
        for dx in -4..=4 {
            tw.place("grass", dx, dy); // clean open stage: no tall grass, no torches
        }
    }
    tw.change_time_of_day(Time::Night);

    let (px, py) = tw.player_pos();
    let mut z = zombie::new(&tw.g, 1);
    z.c.x = px;
    z.c.y = py - 48;
    tw.g.level_mut(lvl).add(z, lvl);
    tw.tick_n(1); // flush the add queue

    let zid = tw
        .entities
        .ids_on_level(lvl)
        .into_iter()
        .find(|id| {
            matches!(
                tw.entities.get(*id).map(|e| &e.kind),
                Some(EntityKind::Zombie(_))
            )
        })
        .expect("zombie is live on the level");
    let (zx, zy) = {
        let z = tw.entities.get(zid).expect("zombie");
        (z.c.x, z.c.y)
    };

    // direct entity render to a fresh screen: no night grade, exact pixel values
    let mut scr = Screen::new(Arc::new(fdoom::assets::sprite_sheet()));
    scr.clear(0);
    scr.set_offset(zx - 32, zy - 32);
    tw.g.with_entity(zid, |z, g| entity_render(g, &mut scr, z))
        .expect("render zombie");

    fn at(scr: &Screen, x: i32, y: i32) -> i32 {
        scr.pixels[x as usize + y as usize * screen::W as usize] & 0xFFFFFF
    }
    // world (zx-2, zy-5) and (zx+1, zy-5) minus the (zx-32, zy-32) offset:
    assert_eq!(
        at(&scr, 30, 27),
        GLINT_RGB,
        "left eye glint at EYES_POS offset"
    );
    assert_eq!(
        at(&scr, 33, 27),
        GLINT_RGB,
        "right eye glint at EYES_POS offset"
    );

    // control: by day (surface) the glint is off
    tw.change_time_of_day(Time::Day);
    scr.clear(0);
    scr.set_offset(zx - 32, zy - 32);
    tw.g.with_entity(zid, |z, g| entity_render(g, &mut scr, z))
        .expect("render zombie");
    assert_ne!(at(&scr, 30, 27), GLINT_RGB, "no glint in daylight");
    assert_ne!(at(&scr, 33, 27), GLINT_RGB, "no glint in daylight");
}

/// Not a test assertion — a staged-frame dumper for visual review. Ignored unless
/// run explicitly; writes PNGs to `$FDOOM_SHOT_DIR` (skips silently when unset so
/// `--ignored` sweeps stay green).
#[test]
#[ignore = "visual fixture: set FDOOM_SHOT_DIR and run explicitly"]
fn showcase_shots() {
    let Ok(dir) = std::env::var("FDOOM_SHOT_DIR") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let (w, h) = (screen::W as usize, screen::H as usize);

    // -- swing + impact puffs, daylight, tree below the player --
    let mut tw = TestWorld::infinite().seed(11).build();
    tw.change_time_of_day(Time::Day);
    tw.place("tree", 0, 1);
    tw.player_mut().player_mut().stamina = 10;
    tw.input.key_toggled("SPACE", true);
    tw.tick();
    tw.input.key_toggled("SPACE", false);
    let px = tw.render();
    fdoom::testutil::save_png(dir.join("swing_t1.png"), &px, w, h, 1);
    tw.tick();
    let px = tw.render();
    fdoom::testutil::save_png(dir.join("swing_t2.png"), &px, w, h, 1);
    tw.tick_n(3);
    let px = tw.render();
    fdoom::testutil::save_png(dir.join("hit_puff.png"), &px, w, h, 1);

    // -- winded cue: gray breath puff at 0 stamina --
    let mut tw = TestWorld::infinite().seed(11).build();
    tw.change_time_of_day(Time::Day);
    tw.place("tree", 0, 1);
    tw.player_mut().player_mut().stamina = 0;
    tw.input.key_toggled("SPACE", true);
    tw.tick();
    tw.input.key_toggled("SPACE", false);
    tw.tick_n(2);
    let px = tw.render();
    fdoom::testutil::save_png(dir.join("winded_puff.png"), &px, w, h, 1);

    // -- night scene: hostiles around the player with glowing eyes --
    let mut tw = TestWorld::infinite().seed(11).build();
    let lvl = tw.current_level;
    for dy in -7..=7 {
        for dx in -10..=10 {
            tw.place("grass", dx, dy);
        }
    }
    tw.change_time_of_day(Time::Night);
    let (ppx, ppy) = tw.player_pos();
    for (dx, dy) in [(-56, -40), (48, -24), (-24, 40), (72, 32)] {
        let mut z = zombie::new(&tw.g, 1);
        z.c.x = ppx + dx;
        z.c.y = ppy + dy;
        tw.g.level_mut(lvl).add(z, lvl);
    }
    let mut gh = fdoom::entity::mob::ghost::new(&tw.g, 1);
    gh.c.x = ppx - 80;
    gh.c.y = ppy + 8;
    tw.g.level_mut(lvl).add(gh, lvl);
    tw.tick_n(1);
    let px = tw.render();
    fdoom::testutil::save_png(dir.join("night_eyes.png"), &px, w, h, 1);

    // -- player hurt flash, daylight --
    let mut tw = TestWorld::infinite().seed(11).build();
    tw.change_time_of_day(Time::Day);
    let pid = tw.player_id;
    tw.g.with_entity(pid, |e, g| {
        player_behavior::do_hurt(g, e, 2, Direction::Left)
    });
    let px = tw.render();
    fdoom::testutil::save_png(dir.join("hurt_flash.png"), &px, w, h, 1);
}
