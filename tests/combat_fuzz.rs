//! Adversarial crash hunt for combat, death/respawn, potions and the bench: swinging
//! every tool at every tier into every entity kind, dying and respawning with a full
//! pack, chained explosions, and fitting bench modules while the bench is destroyed.

use fdoom::entity::furniture::crafter::{CrafterType, Module};
use fdoom::entity::{Direction, Entity, EntityKind, behavior, furniture, mob};
use fdoom::item::{PotionType, ToolType, registry};
use fdoom::level;
use fdoom::testutil::TestWorld;

fn place(tw: &mut TestWorld, mut e: Entity, dx: i32, dy: i32) -> i32 {
    static NEXT_EID: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(600_001);
    let lvl = tw.current_level;
    let (px, py) = tw.player_pos();
    let eid = NEXT_EID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    e.c.eid = eid;
    e.c.x = px + dx;
    e.c.y = py + dy;
    tw.g.level_mut(lvl).add(e, lvl);
    level::tick_level(&mut tw.g, lvl, false);
    eid
}

fn mob_roster(g: &fdoom::core::game::Game) -> Vec<Entity> {
    vec![
        mob::cow::new(g),
        mob::deer::new(g),
        mob::pig::new(g),
        mob::sheep::new(g),
        mob::glow_worm::new(g),
        mob::zombie::new(g, 1),
        mob::snake::new(g, 1),
        mob::knight::new(g, 1),
        mob::marsh_lurker::new(g, 1),
        mob::feral_hound::new(g, 1),
        mob::stone_golem::new(g, 1),
        mob::night_wisp::new(g, 1),
        mob::ghost::new(g, 1),
    ]
}

/* --------------------------------- swinging things --------------------------------- */

/// Every tool at every tier, swung into every mob kind and into empty air, with and
/// without ammunition, at zero stamina and at full — the whole attack dispatch.
#[test]
fn every_tool_at_every_tier_swung_at_every_mob() {
    let mut tw = TestWorld::infinite().name("combat_tools").build();
    let pid = tw.player_id;
    let roster = mob_roster(&tw.g);

    // the tier only indexes the name/color tables, so the ends and a middle cover it
    for ttype in ToolType::VALUES {
        for level in [0, 3, 5] {
            for stamina in [0, 10] {
                let tool = registry::new_tool_item(ttype, level);
                // targets: one of each mob right in front, plus a swing at nothing
                for target in roster.iter() {
                    let eid = place(&mut tw, target.clone(), 12, 0);
                    tw.g.with_entity(pid, |player, _g| {
                        let pd = player.player_mut();
                        pd.active_item = Some(tool.clone());
                        pd.stamina = stamina;
                        pd.mob.dir = Direction::Right;
                    });
                    tw.g.with_entity(pid, |player, g| {
                        mob::player_behavior::attack(g, player);
                    });
                    // and again with the target already dead
                    tw.g.with_entity(eid, |t, g| {
                        if let Some(m) = t.mob_mut() {
                            m.health = 0;
                        }
                        behavior::die(g, t);
                    });
                    tw.g.with_entity(pid, |player, g| {
                        mob::player_behavior::attack(g, player);
                    });
                    tw.g.entities.delete(eid);
                }
            }
        }
    }
    // the arrows/stones the ranged tools want, so the projectile paths run too
    tw.give("arrow", 20);
    tw.give("Stone", 20);
    for ttype in [ToolType::Bow, ToolType::Crossbow, ToolType::Slingshot] {
        let tool = registry::new_tool_item(ttype, 1);
        for _ in 0..6 {
            tw.g.with_entity(pid, |player, _g| {
                let pd = player.player_mut();
                pd.active_item = Some(tool.clone());
                pd.stamina = 10;
                pd.attack_time = 0;
            });
            tw.g.with_entity(pid, |player, g| {
                mob::player_behavior::attack(g, player);
            });
            tw.tick();
        }
    }
    tw.tick_n(40); // let the projectiles fly out and land
}

/* ------------------------------- death and respawn ------------------------------- */

/// Dying with a stuffed pack builds a death chest out of everything carried; do it
/// repeatedly, with armor and a held item, and respawn each time.
#[test]
fn dying_and_respawning_with_a_full_pack() {
    let mut tw = TestWorld::infinite().name("combat_death").build();
    let pid = tw.player_id;
    for round in 0..4 {
        for name in ["Wood", "Stone", "Coal", "Cord", "Leather Armor"] {
            tw.give(name, 5 + round);
        }
        tw.g.with_entity(pid, |player, g| {
            let armor = registry::get(g, "Leather Armor");
            let hat = registry::get(g, "Straw Hat");
            let pd = player.player_mut();
            pd.equip(armor);
            pd.equip(hat);
            pd.active_item = Some(registry::new_tool_item(ToolType::Sword, 1));
        });
        // lethal damage, then the mob-tick death path, then the direct die()
        tw.g.with_entity(pid, |player, g| {
            behavior::do_hurt(g, player, 1000, Direction::Down);
            player.player_mut().mob.health = 0;
            mob::player_behavior::die(g, player);
        });
        let lvl = tw.current_level;
        level::tick_level(&mut tw.g, lvl, true);
        tw.tick_recover();
        tw.render();
    }
}

/// A player killed by a mob it is standing on, while that mob dies in the same tick.
#[test]
fn player_and_mob_kill_each_other_in_one_tick() {
    let mut tw = TestWorld::infinite().name("combat_mutual").build();
    let pid = tw.player_id;
    for proto in mob_roster(&tw.g) {
        let eid = place(&mut tw, proto, 4, 0);
        tw.g.with_entity(pid, |player, _g| {
            player.player_mut().mob.health = 1;
            player.player_mut().mob.hurt_time = 0;
        });
        // the mob touches the player (may kill it) while the mob is itself at 0 hp
        tw.g.with_entity(eid, |m, g| {
            if let Some(mob) = m.mob_mut() {
                mob.health = 0;
            }
            g.with_entity(pid, |player, g| {
                behavior::touched_by(g, player, m);
            });
            behavior::entity_tick(g, m);
        });
        let lvl = tw.current_level;
        level::tick_level(&mut tw.g, lvl, true);
        tw.tick_recover();
        tw.g.entities.delete(eid);
    }
}

/* ---------------------------------- chain blasts ---------------------------------- */

/// A pile of TNT: lighting one lights the rest, each of which removes itself mid-blast
/// while the others are still resolving.
#[test]
fn chained_tnt_blasts_remove_each_other() {
    let mut tw = TestWorld::infinite().name("combat_tnt_chain").build();
    let mut eids = Vec::new();
    for i in 0..6 {
        let tnt = furniture::tnt::new();
        eids.push(place(&mut tw, tnt, 8 + i * 6, 0));
    }
    // a cow and a chest in the blast, so the damage and drop paths run too
    let cow = mob::cow::new(&tw.g);
    place(&mut tw, cow, 12, 4);
    let mut chest = furniture::chest::new();
    let wood = registry::get(&tw.g, "Wood_9");
    chest.chest_mut().expect("chest").inventory.add(wood);
    place(&mut tw, chest, 16, 8);

    // light the first one and run the whole chain out
    tw.g.with_entity(eids[0], |e, _g| {
        if let EntityKind::Tnt(t) = &mut e.kind {
            t.fuse_lit = true;
            t.ftik = furniture::tnt::FUSE_TIME - 1;
        }
    });
    for _ in 0..240 {
        tw.tick_recover();
    }
    tw.render();
}

/* ------------------------------------ potions ------------------------------------ */

/// Every potion applied, re-applied, expired and force-removed, in both orders.
#[test]
fn every_potion_applied_expired_and_removed() {
    let mut tw = TestWorld::infinite().name("combat_potions").build();
    let pid = tw.player_id;
    for ptype in PotionType::VALUES {
        for _ in 0..3 {
            tw.g.with_entity(pid, |player, g| {
                fdoom::item::interact::apply_potion(g, player, ptype, true);
                fdoom::item::interact::apply_potion_time(g, player, ptype, 1);
            });
            // let it tick down to nothing
            tw.tick_n(4);
            tw.g.with_entity(pid, |player, g| {
                fdoom::item::interact::apply_potion(g, player, ptype, false);
                fdoom::item::interact::apply_potion(g, player, ptype, false);
            });
            tw.render();
        }
    }
    // and drink every potion item for real
    for ptype in PotionType::VALUES {
        let mut item = registry::new_potion_item(ptype);
        tw.interact_item(&mut item, 0, 1);
    }
    tw.tick_n(10);
}

/* ------------------------------------ the bench ------------------------------------ */

/// Fitting modules at THE BENCH: every module, duplicates, an empty hand, and the
/// bench destroyed between the fit and the screen it opens.
#[test]
fn bench_module_fitting_survives_the_bench_being_destroyed() {
    let mut tw = TestWorld::infinite().name("combat_bench").build();
    let pid = tw.player_id;
    for destroy in [false, true] {
        for m in Module::VALUES {
            let bench = furniture::crafter::new(CrafterType::Bench);
            let eid = place(&mut tw, bench, 16, 0);
            for _ in 0..3 {
                tw.g.with_entity(pid, |player, g| {
                    player.player_mut().active_item = Some(registry::get(g, m.item_name()));
                });
                tw.g.with_entity(pid, |player, g| {
                    g.with_entity(eid, |b, g| {
                        furniture::crafter_behavior::use_furniture(g, b, player);
                    });
                });
                if destroy {
                    tw.g.entities.delete(eid);
                }
                tw.tick();
                for key in ["DOWN", "ENTER", "LEFT", "RIGHT", "Q", "SHIFT-Q"] {
                    tw.press(key);
                    tw.render();
                }
                tw.g.clear_menu();
            }
            tw.g.entities.delete(eid);
        }
    }
}

/* ------------------------------------ equipping ------------------------------------ */

/// Every registry item pushed through both equip paths — the WEAR pane's instant
/// `equip`/`unequip` and the legacy use-to-wear ritual — including everything that
/// fits no slot at all.
#[test]
fn every_item_equipped_and_unequipped_by_both_paths() {
    use fdoom::entity::mob::player::WearSlot;
    let mut tw = TestWorld::infinite().name("combat_equip").build();
    let pid = tw.player_id;
    let items = tw.g.items.to_vec();
    for item in items {
        // the WEAR pane path: equip, displace, unequip, unequip again
        tw.g.with_entity(pid, |player, _g| {
            let pd = player.player_mut();
            let displaced = pd.equip(item.clone());
            if let Some(d) = displaced {
                pd.inventory.add_at(0, d);
            }
            pd.equip(item.clone());
            for slot in [WearSlot::Head, WearSlot::Body] {
                if let Some(prev) = pd.unequip(slot) {
                    pd.inventory.add_at(0, prev);
                }
                pd.unequip(slot);
            }
        });
        // the legacy ritual: hold it and swing at the ground, at zero stamina and full
        for stamina in [0, 10] {
            tw.g.with_entity(pid, |player, _g| {
                player.player_mut().stamina = stamina;
            });
            let mut held = item.clone();
            tw.interact_item(&mut held, 0, 1);
        }
    }
    tw.press("E");
    for _ in 0..12 {
        tw.press("RIGHT");
        tw.press("ENTER");
        tw.press("Q");
        tw.render();
    }
}

/* ------------------------------ entities out of place ------------------------------ */

/// A mob on another level from the player, and a mob on no level at all (the state an
/// entity sits in between being removed and being dropped from the arena). Both get
/// ticked, hurt, killed and interacted with.
#[test]
fn mobs_on_another_level_or_no_level_still_tick() {
    let mut tw = TestWorld::infinite().name("combat_offlevel").build();
    let pid = tw.player_id;
    let other_level =
        tw.g.levels
            .iter()
            .enumerate()
            .find(|(i, l)| *i != tw.g.current_level && l.is_some())
            .map(|(i, _)| i)
            .expect("a second level exists");

    for proto in mob_roster(&tw.g) {
        for target_level in [Some(other_level), None] {
            let mut m = proto.clone();
            m.c.eid = 500_001;
            m.c.level = target_level;
            m.c.removed = false;
            m.c.x = 128;
            m.c.y = 128;
            tw.g.entities.put_back(m);
            tw.g.with_entity(500_001, |e, g| {
                behavior::entity_tick(g, e);
                behavior::do_hurt(g, e, 3, Direction::Up);
                behavior::entity_tick(g, e);
            });
            let mut glove = Some(registry::new_power_glove());
            tw.g.with_entity(pid, |player, g| {
                g.with_entity(500_001, |e, g| {
                    behavior::entity_interact(g, e, player, &mut glove, Direction::Down);
                    behavior::touched_by(g, e, player);
                });
            });
            tw.g.with_entity(500_001, |e, g| {
                if let Some(mob) = e.mob_mut() {
                    mob.health = 0;
                }
                behavior::die(g, e);
                behavior::entity_tick(g, e);
            });
            tw.g.entities.delete(500_001);
        }
    }
    tw.tick_recover();
}

/// Every station opened with the survival screen, driven through every tab with the
/// station destroyed underneath.
#[test]
fn every_station_screen_driven_after_the_station_is_gone() {
    let mut tw = TestWorld::infinite().name("combat_stations").build();
    let pid = tw.player_id;
    for ctype in CrafterType::VALUES {
        let station = furniture::crafter::new(ctype);
        let eid = place(&mut tw, station, 16, 0);
        tw.give("Wood", 30);
        tw.g.with_entity(pid, |player, g| {
            g.with_entity(eid, |s, g| {
                furniture::crafter_behavior::use_furniture(g, s, player);
            });
        });
        tw.tick();
        tw.g.entities.delete(eid);
        // every tab, every action key
        for tab in 0..5 {
            tw.press("RIGHT");
            for key in ["DOWN", "UP", "ENTER", "Q", "SHIFT-Q", "X"] {
                tw.press(key);
                tw.render();
            }
            let _ = tab;
        }
        tw.g.clear_menu();
    }
}
