//! Adversarial cross-product sweeps: every item used on every entity kind, every
//! entity kind killed and then touched again after removal, furniture picked up out
//! from under an open menu, and the container screen's lifetime against a container
//! that is destroyed while it is open.
//!
//! A sweep runs every combination and reports ALL failures at once (one panic does
//! not hide the next), so a single run is a full map of the damage.

use std::cell::RefCell;
use std::panic::{self, AssertUnwindSafe};
use std::rc::Rc;
use std::sync::{Arc, Mutex, OnceLock};

use fdoom::core::game::Game;
use fdoom::entity::furniture::crafter::CrafterType;
use fdoom::entity::furniture::lantern::LanternType;
use fdoom::entity::furniture::scav_container::ScavKind;
use fdoom::entity::{Direction, Entity, behavior, furniture, mob};
use fdoom::item::Item;
use fdoom::level;
use fdoom::testutil::TestWorld;

/* ---------------------------------- the harness ---------------------------------- */

/// The panic hook is process-global; sweeps take this so tests running in parallel in
/// the same binary don't clobber each other's hooks.
fn hook_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

type Case = (String, Box<dyn FnOnce()>);

/// Run every case, collecting panics instead of stopping at the first.
fn sweep(what: &str, cases: Vec<Case>) {
    let guard = hook_lock().lock().unwrap_or_else(|e| e.into_inner());
    let last: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let sink = Arc::clone(&last);
    let prev = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        *sink.lock().unwrap_or_else(|e| e.into_inner()) = info.to_string();
    }));

    let total = cases.len();
    let mut failures: Vec<String> = Vec::new();
    for (name, case) in cases {
        if panic::catch_unwind(AssertUnwindSafe(case)).is_err() {
            let msg = last.lock().unwrap_or_else(|e| e.into_inner()).clone();
            failures.push(format!("  {name}\n      {msg}"));
        }
    }
    panic::set_hook(prev);
    drop(guard);

    assert!(
        failures.is_empty(),
        "{} of {total} {what} cases panicked:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/* --------------------------------- world helpers --------------------------------- */

/// Place `e` near the player with a pinned eid (the arena only assigns one when the
/// eid is < 0, so the caller can never pick up an unrelated entity).
fn place(tw: &mut TestWorld, mut e: Entity, dx: i32, dy: i32) -> i32 {
    static NEXT_EID: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(700_001);
    let lvl = tw.current_level;
    let (px, py) = tw.player_pos();
    let eid = NEXT_EID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    e.c.eid = eid;
    e.c.x = px + dx;
    e.c.y = py + dy;
    tw.g.level_mut(lvl).add(e, lvl);
    level::tick_level(&mut tw.g, lvl, false);
    assert!(tw.g.entities.contains(eid), "entity placed");
    eid
}

/// Every entity kind that can exist in a level, freshly built.
fn every_entity_kind(g: &mut Game) -> Vec<(String, Entity)> {
    let mut out: Vec<(String, Entity)> = vec![
        ("Cow".into(), mob::cow::new(g)),
        ("Deer".into(), mob::deer::new(g)),
        ("Pig".into(), mob::pig::new(g)),
        ("Sheep".into(), mob::sheep::new(g)),
        ("GlowWorm".into(), mob::glow_worm::new(g)),
        ("Zombie".into(), mob::zombie::new(g, 1)),
        ("Snake".into(), mob::snake::new(g, 1)),
        ("Knight".into(), mob::knight::new(g, 1)),
        ("MarshLurker".into(), mob::marsh_lurker::new(g, 1)),
        ("FeralHound".into(), mob::feral_hound::new(g, 1)),
        ("StoneGolem".into(), mob::stone_golem::new(g, 1)),
        ("NightWisp".into(), mob::night_wisp::new(g, 1)),
        ("Ghost".into(), mob::ghost::new(g, 1)),
        ("Chest".into(), furniture::chest::new()),
        ("DeathChest".into(), furniture::death_chest::new(g)),
        ("DungeonChest".into(), furniture::dungeon_chest::new(g)),
        ("Bed".into(), furniture::bed::new()),
        ("Tnt".into(), furniture::tnt::new()),
        ("Campfire".into(), furniture::campfire::new()),
        ("CampfireEmber".into(), furniture::campfire::new_ember()),
    ];
    for k in ScavKind::VALUES {
        out.push((
            format!("ScavContainer({k:?})"),
            furniture::scav_container::new(k),
        ));
    }
    for c in CrafterType::VALUES {
        out.push((format!("Crafter({c:?})"), furniture::crafter::new(c)));
    }
    for l in [LanternType::Norm, LanternType::Iron, LanternType::Gold] {
        out.push((format!("Lantern({l:?})"), furniture::lantern::new(l)));
    }
    let spawner_mob = mob::zombie::new(g, 1);
    let mut rng = g.random.clone();
    out.push((
        "Spawner".into(),
        furniture::spawner::new(spawner_mob, &mut rng),
    ));
    let wood = fdoom::item::registry::get(g, "Wood");
    out.push((
        "ItemEntity".into(),
        fdoom::entity::item_entity::new(wood, 0, 0, &mut rng),
    ));
    out.push(("Fireflies".into(), fdoom::entity::fireflies::new(&mut rng)));
    out
}

fn every_item(g: &Game) -> Vec<Item> {
    let mut items: Vec<Item> = g.items.to_vec();
    items.push(fdoom::item::registry::new_power_glove());
    items.push(fdoom::item::registry::new_unknown_item("Nonsense"));
    items
}

/* ------------------------------ item x entity interact ------------------------------ */

/// Every registry item (plus the power glove and an unknown item) interacted on every
/// entity kind, the way the player's swing does it: both entities taken out of the
/// arena, so any code assuming either is still present shows up here. Each case then
/// interacts a SECOND time, after the first interaction may have removed the target.
#[test]
fn every_item_interacted_on_every_entity_kind() {
    let mut world = TestWorld::infinite().name("sweep_item_entity").build();
    let kinds = every_entity_kind(&mut world.g);
    let items = every_item(&world.g);
    let pid = world.player_id;
    let tw = Rc::new(RefCell::new(world));

    let mut cases: Vec<Case> = Vec::new();
    for (kind_name, proto) in kinds {
        for item in &items {
            let name = format!("{kind_name} <- {}", item.get_name());
            let tw = Rc::clone(&tw);
            let proto = proto.clone();
            let item = item.clone();
            cases.push((
                name,
                Box::new(move || {
                    let mut tw = tw.borrow_mut();
                    let eid = place(&mut tw, proto, 20, 0);
                    let mut held = Some(item);
                    for _ in 0..2 {
                        tw.g.with_entity(pid, |player, g| {
                            g.with_entity(eid, |target, g| {
                                behavior::entity_interact(
                                    g,
                                    target,
                                    player,
                                    &mut held,
                                    Direction::Down,
                                );
                            });
                        });
                    }
                    tw.g.entities.delete(eid);
                    // the glove leaves furniture in hand; clear it so cases stay independent
                    tw.g.with_entity(pid, |player, _g| {
                        player.player_mut().active_item = None;
                        player.player_mut().prev_item = None;
                    });
                }),
            ));
        }
    }
    sweep("item-on-entity", cases);
}

/* ------------------------------- touch / kill / re-hurt ------------------------------- */

/// Every entity kind touched by the player and touching the player, then killed, then
/// hurt and touched again after it is gone — the "died in the same tick" shape.
#[test]
fn every_entity_kind_touched_killed_and_hurt_again() {
    let mut world = TestWorld::infinite().name("sweep_touch_kill").build();
    let kinds = every_entity_kind(&mut world.g);
    let pid = world.player_id;
    let tw = Rc::new(RefCell::new(world));

    let mut cases: Vec<Case> = Vec::new();
    for (kind_name, proto) in kinds {
        let tw = Rc::clone(&tw);
        cases.push((
            kind_name,
            Box::new(move || {
                let mut tw = tw.borrow_mut();
                let eid = place(&mut tw, proto, 8, 0);
                // the player walks into it, and it walks into the player
                tw.g.with_entity(pid, |player, g| {
                    g.with_entity(eid, |target, g| {
                        behavior::touched_by(g, target, player);
                    });
                });
                tw.g.with_entity(eid, |target, g| {
                    g.with_entity(pid, |player, g| {
                        behavior::touched_by(g, player, target);
                    });
                });
                // kill it (its own die path: drops, score, removal)
                tw.g.with_entity(eid, |target, g| {
                    if let Some(m) = target.mob_mut() {
                        m.health = 0;
                    }
                    behavior::die(g, target);
                });
                // ...then keep hitting the corpse, and tick it
                tw.g.with_entity(eid, |target, g| {
                    behavior::do_hurt(g, target, 5, Direction::Up);
                    behavior::entity_tick(g, target);
                    behavior::die(g, target);
                });
                let lvl = tw.current_level;
                level::tick_level(&mut tw.g, lvl, true);
                tw.g.entities.delete(eid);
            }),
        ));
    }
    sweep("touch/kill/re-hurt", cases);
}

/* ------------------------------- lethal damage math ------------------------------- */

/// Damage and healing at the extremes: a killing blow of `i32::MAX`, negative damage,
/// healing past the cap, and hurting an entity that is already dead.
#[test]
fn extreme_damage_values_on_every_mob() {
    let mut world = TestWorld::infinite().name("sweep_damage").build();
    let kinds = every_entity_kind(&mut world.g);
    let tw = Rc::new(RefCell::new(world));

    let mut cases: Vec<Case> = Vec::new();
    for (kind_name, proto) in kinds {
        for dmg in [i32::MAX, i32::MIN, -1, 0, 1_000_000] {
            let tw = Rc::clone(&tw);
            let proto = proto.clone();
            cases.push((
                format!("{kind_name} hurt {dmg}"),
                Box::new(move || {
                    let mut tw = tw.borrow_mut();
                    let eid = place(&mut tw, proto, 12, 0);
                    tw.g.with_entity(eid, |target, g| {
                        behavior::do_hurt(g, target, dmg, Direction::Left);
                        if let Some(m) = target.mob_mut() {
                            m.hurt_time = 0;
                        }
                        behavior::do_hurt(g, target, dmg, Direction::Right);
                        behavior::heal(g, target, dmg);
                        behavior::entity_tick(g, target);
                    });
                    tw.g.entities.delete(eid);
                }),
            ));
        }
    }
    sweep("extreme-damage", cases);
}

/* --------------------------- furniture yanked out of a menu --------------------------- */

/// Open every furniture's screen, then destroy the furniture under it and keep
/// driving the menu: ticks, renders, and every key. A screen that remembers an eid
/// has to survive its entity vanishing.
#[test]
fn furniture_destroyed_while_its_menu_is_open() {
    let mut world = TestWorld::infinite().name("sweep_menu_lifetime").build();
    let kinds: Vec<(String, Entity)> = every_entity_kind(&mut world.g)
        .into_iter()
        .filter(|(_, e)| e.is_furniture())
        .collect();
    let pid = world.player_id;
    let tw = Rc::new(RefCell::new(world));

    let mut cases: Vec<Case> = Vec::new();
    for (kind_name, proto) in kinds {
        for destroy_first in [false, true] {
            let tw = Rc::clone(&tw);
            let proto = proto.clone();
            cases.push((
                format!("{kind_name} (destroy_first={destroy_first})"),
                Box::new(move || {
                    let mut tw = tw.borrow_mut();
                    let eid = place(&mut tw, proto, 16, 0);
                    // stock any container so its list has rows to index
                    tw.g.with_entity(eid, |target, g| {
                        if let Some(chest) = target.chest_mut() {
                            for name in ["Wood", "Stone", "Coal"] {
                                let item = fdoom::item::registry::get(g, &format!("{name}_5"));
                                chest.inventory.add(item);
                            }
                        }
                    });
                    tw.g.with_entity(pid, |player, g| {
                        g.with_entity(eid, |target, g| {
                            furniture::behavior::use_furniture(g, target, player);
                        });
                    });
                    if destroy_first {
                        tw.g.entities.delete(eid);
                    } else {
                        tw.g.with_entity(eid, |target, g| behavior::remove_entity(g, target));
                        let lvl = tw.current_level;
                        level::tick_level(&mut tw.g, lvl, true);
                    }
                    // now drive the menu that outlived its furniture
                    for key in [
                        "down",
                        "up",
                        "left",
                        "right",
                        "select",
                        "drop-one",
                        "drop-stack",
                        "attack",
                        "menu",
                    ] {
                        tw.press(key);
                        tw.render();
                    }
                    tw.g.clear_menu();
                    tw.g.entities.delete(eid);
                }),
            ));
        }
    }
    sweep("furniture-menu-lifetime", cases);
}

/* ------------------------------ power glove on everything ------------------------------ */

/// The power glove picks furniture up; do it while the furniture's own screen is open,
/// then place it again, then pick it up again. Nothing may assume the entity is still
/// in the world.
#[test]
fn power_glove_take_and_replace_every_furniture() {
    let mut world = TestWorld::infinite().name("sweep_glove").build();
    let kinds: Vec<(String, Entity)> = every_entity_kind(&mut world.g)
        .into_iter()
        .filter(|(_, e)| e.is_furniture())
        .collect();
    let pid = world.player_id;
    let tw = Rc::new(RefCell::new(world));

    let mut cases: Vec<Case> = Vec::new();
    for (kind_name, proto) in kinds {
        let tw = Rc::clone(&tw);
        cases.push((
            kind_name,
            Box::new(move || {
                let mut tw = tw.borrow_mut();
                let eid = place(&mut tw, proto, 16, 0);
                // open its screen first, so the take happens under a live menu
                tw.g.with_entity(pid, |player, g| {
                    g.with_entity(eid, |target, g| {
                        furniture::behavior::use_furniture(g, target, player);
                    });
                });
                let mut glove = Some(fdoom::item::registry::new_power_glove());
                for _ in 0..2 {
                    tw.g.with_entity(pid, |player, g| {
                        g.with_entity(eid, |target, g| {
                            behavior::entity_interact(g, target, player, &mut glove, Direction::Up);
                        });
                    });
                }
                tw.tick_recover();
                tw.render();
                // place whatever ended up in hand back into the world, twice
                let (tx, ty) = tw.player_tile();
                for _ in 0..2 {
                    let held =
                        tw.g.with_entity(pid, |player, _g| player.player_mut().active_item.take());
                    if let Some(Some(mut item)) = held {
                        tw.interact_item(&mut item, 1, 0);
                        tw.g.with_entity(pid, |player, _g| {
                            player.player_mut().active_item = Some(item);
                        });
                    }
                    let _ = (tx, ty);
                }
                tw.g.clear_menu();
                tw.g.with_entity(pid, |player, _g| {
                    player.player_mut().active_item = None;
                    player.player_mut().prev_item = None;
                });
                tw.g.entities.delete(eid);
            }),
        ));
    }
    sweep("power-glove", cases);
}
