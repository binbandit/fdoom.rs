//! Towns & scavenge wave: the town age axis (Overgrown / Weathered / Settled) with
//! its generation markers, one-time searchable scavenge containers with seeded loot
//! and an emptied render state, and the scavenge item chain (water bottles, old food
//! cans, can -> tin smelting).
//!
//! Set `TOWNS_SHOT_DIR=/some/dir` to also dump day frames of one town per age, a
//! house interior with containers, and the emptied-state readability pair.

use fdoom::core::game::Game;
use fdoom::entity::furniture::scav_container::ScavKind;
use fdoom::entity::{Direction, EntityKind};
use fdoom::gfx::screen;
use fdoom::item::{Inventory, registry};
use fdoom::level::structures_gen::{
    Placement, StructureKind, TownAge, container_positions, lantern_positions, placements_in_rect,
    structure_writes, town_age,
};
use fdoom::level::tile::Tiles;
use fdoom::rng::Rng;
use fdoom::saveload::{load, save};
use fdoom::testutil::{TestWorld, bare_game, save_png};

const SEEDS: [i64; 5] = [20260707, 7, 9, 42, 1234];

fn towns(seed: i64, radius: i32) -> Vec<Placement> {
    placements_in_rect(seed, -radius, -radius, radius, radius)
        .into_iter()
        .filter(|p| matches!(p.kind, StructureKind::Village | StructureKind::Hamlet))
        .collect()
}

/// The first town of `kind` and `age` across the seed sweep (nearest to the origin
/// within each seed's scan, so world boots don't stream chunks forever).
fn find_town(kind: StructureKind, age: TownAge) -> (i64, Placement) {
    SEEDS
        .iter()
        .find_map(|&s| {
            towns(s, 8192)
                .into_iter()
                .filter(|p| p.kind == kind && town_age(s, *p) == age)
                .min_by_key(|p| i64::from(p.x).pow(2) + i64::from(p.y).pow(2))
                .map(|p| (s, p))
        })
        .unwrap_or_else(|| panic!("no {kind:?} of age {age:?} across the seed sweep"))
}

#[test]
fn all_three_ages_generate_for_both_town_kinds() {
    let (mut counts_v, mut counts_h) = ([0i32; 3], [0i32; 3]);
    for &s in &SEEDS {
        for p in towns(s, 16384) {
            let slot = match town_age(s, p) {
                TownAge::Overgrown => 0,
                TownAge::Weathered => 1,
                TownAge::Settled => 2,
            };
            match p.kind {
                StructureKind::Village => counts_v[slot] += 1,
                _ => counts_h[slot] += 1,
            }
        }
    }
    for (i, name) in ["Overgrown", "Weathered", "Settled"].iter().enumerate() {
        assert!(counts_v[i] > 0, "no {name} village across the sweep");
        assert!(counts_h[i] > 0, "no {name} hamlet across the sweep");
    }
}

#[test]
fn age_markers_overgrowth_gardens_and_lantern_counts() {
    let tiles = Tiles::new();
    let id = |n: &str| tiles.get(n).id;
    let tufts = [id("small grass"), id("medium grass"), id("tall grass")];
    let count_of = |s: i64, p: Placement, want: &dyn Fn(u8) -> bool| {
        structure_writes(s, p, &tiles)
            .iter()
            .filter(|w| want(w.2))
            .count()
    };

    // OVERGROWN: flora reclaiming the town, lanterns burnt out (at most one)
    let (s, p) = find_town(StructureKind::Village, TownAge::Overgrown);
    let flora = count_of(s, p, &|t| tufts.contains(&t) || t == id("grass"));
    assert!(
        flora >= 5,
        "overgrown village has only {flora} reclaimed tiles"
    );
    assert!(
        lantern_positions(s, p).len() <= 1,
        "overgrown village still fully lit"
    );

    // SETTLED: tended garden (farmland + picket fence), every lamp burning plus
    // the town-center one — strictly brighter than any overgrown town
    let (s2, p2) = find_town(StructureKind::Village, TownAge::Settled);
    assert!(
        count_of(s2, p2, &|t| t == id("Farmland")) >= 3,
        "settled village keeps no tended plot"
    );
    assert!(
        count_of(s2, p2, &|t| t == id("Fence")) >= 4,
        "settled village garden lost its pickets"
    );
    let settled_lanterns = lantern_positions(s2, p2).len();
    assert!(
        settled_lanterns >= 4,
        "settled village only lights {settled_lanterns} lanterns"
    );

    // WEATHERED: the classic look — one lantern per house, no *tended* garden.
    // (Bare Farmland is allowed: the farming wave gives aged villages a field
    // gone to seed. The kitchen garden's marker is its berry bush at the picket
    // gap, which only `stamp_garden` writes.)
    let (s3, p3) = find_town(StructureKind::Village, TownAge::Weathered);
    let weathered = lantern_positions(s3, p3).len();
    assert!(
        (3..=5).contains(&weathered),
        "weathered village lights {weathered} lanterns"
    );
    assert_eq!(
        count_of(s3, p3, &|t| t == id("Berry Bush")),
        0,
        "weathered village should not keep a tended kitchen garden"
    );

    // and hamlets ride the same axis
    let (s4, p4) = find_town(StructureKind::Hamlet, TownAge::Overgrown);
    assert!(lantern_positions(s4, p4).len() <= 1);
    let (s5, p5) = find_town(StructureKind::Hamlet, TownAge::Settled);
    assert!(lantern_positions(s5, p5).len() >= 3);
}

#[test]
fn container_density_leans_on_age() {
    // aggregate across the sweep: settled towns keep more intact stock per town
    // than overgrown ones (whose fewer holds lean time-capsule instead)
    let (mut settled, mut settled_n) = (0usize, 0usize);
    let (mut overgrown, mut overgrown_n) = (0usize, 0usize);
    for &s in &SEEDS {
        for p in towns(s, 16384) {
            match town_age(s, p) {
                TownAge::Settled => {
                    settled += container_positions(s, p).len();
                    settled_n += 1;
                }
                TownAge::Overgrown => {
                    overgrown += container_positions(s, p).len();
                    overgrown_n += 1;
                }
                TownAge::Weathered => {}
            }
        }
    }
    assert!(settled_n > 10 && overgrown_n > 10, "sweep too small");
    assert!(
        settled * overgrown_n > overgrown * settled_n,
        "settled towns must average more containers ({settled}/{settled_n} vs {overgrown}/{overgrown_n})"
    );
}

/// Boot the world at a town container, returning (world, container eid, kind).
fn boot_at_container() -> (TestWorld, i32, ScavKind) {
    let (s, p, (cx, cy, kind)) = SEEDS
        .iter()
        .find_map(|&s| {
            towns(s, 2048)
                .into_iter()
                .filter_map(|p| {
                    container_positions(s, p)
                        .into_iter()
                        .next()
                        .map(|c| (s, p, c))
                })
                .min_by_key(|(_, p, _)| i64::from(p.x).pow(2) + i64::from(p.y).pow(2))
        })
        .expect("no town container within 2048 tiles across the sweep");
    let _ = p;
    let mut tw = TestWorld::infinite().seed(s).build();
    tw.teleport(cx, cy + 2);
    tw.tick_n(8);
    let eid = tw
        .g
        .entities
        .iter()
        .find(|e| {
            matches!(e.kind, EntityKind::ScavContainer(_)) && (e.c.x >> 4, e.c.y >> 4) == (cx, cy)
        })
        .map(|e| e.c.eid)
        .unwrap_or_else(|| panic!("no ScavContainer entity spawned at ({cx}, {cy})"));
    (tw, eid, kind)
}

#[test]
fn container_yields_seeded_loot_exactly_once_and_reads_emptied() {
    let (mut tw, eid, kind) = boot_at_container();

    // the seeded loot is there before the first rummage...
    let before: Vec<String> = {
        let e = tw.g.entities.get(eid).expect("container");
        let inv = &e.chest().expect("chest layer").inventory;
        (0..inv.inv_size())
            .map(|i| inv.get(i).get_name().to_string())
            .collect()
    };
    assert!(!before.is_empty(), "container generated with no loot");

    // ...and an identical world rolls the identical loot (pure from seed + pos)
    let (tw2, eid2, _) = boot_at_container();
    let again: Vec<String> = {
        let e = tw2.g.entities.get(eid2).expect("container");
        let inv = &e.chest().expect("chest layer").inventory;
        (0..inv.inv_size())
            .map(|i| inv.get(i).get_name().to_string())
            .collect()
    };
    assert_eq!(before, again, "container loot must be seeded, not random");

    // first rummage: spills everything, flips the emptied state
    let mut e = tw.g.entities.take(eid).expect("container");
    let mut player = tw.g.entities.take(tw.g.player_id).expect("player");
    assert!(fdoom::entity::furniture::behavior::use_furniture(
        &mut tw.g,
        &mut e,
        &mut player
    ));
    let dropped_after_first = {
        // count the spilled drops while the container is still taken out
        tw.g.level(tw.g.current_level).entities_to_add.len()
    };
    assert!(dropped_after_first > 0, "rummage spilled nothing");
    match &e.kind {
        EntityKind::ScavContainer(sc) => {
            assert!(sc.searched, "container not marked searched");
            assert_eq!(sc.chest.inventory.inv_size(), 0, "loot not drained");
            assert_eq!(
                sc.chest.furniture.sprite.color,
                kind.col(true),
                "sprite not flipped to the emptied palette"
            );
        }
        _ => panic!("wrong kind"),
    }
    assert_eq!(e.c.col, kind.col(true), "entity color not emptied");

    // second rummage: handled (dust), but strictly nothing more comes out
    assert!(fdoom::entity::furniture::behavior::use_furniture(
        &mut tw.g,
        &mut e,
        &mut player
    ));
    assert_eq!(
        tw.g.level(tw.g.current_level).entities_to_add.len(),
        dropped_after_first,
        "second rummage must yield nothing"
    );
    tw.g.entities.put_back(player);
    tw.g.entities.put_back(e);
}

#[test]
fn water_bottle_drinks_and_refills() {
    use fdoom::entity::mob::player::MAX_STAMINA;
    let mut tw = TestWorld::infinite().seed(11).build();
    let lvl = tw.g.current_level;
    let (px, py) = tw.player_tile();

    // drink: modest stamina refresh, bottle empties
    tw.g.player_mut().player_mut().stamina = 2;
    let mut bottle = registry::get(&tw.g, "Water Bottle");
    let mut player = tw.g.entities.take(tw.g.player_id).expect("player");
    assert!(fdoom::item::interact::item_interact_on_tile(
        &mut tw.g,
        &mut bottle,
        lvl,
        px,
        py,
        &mut player,
        Direction::Down,
    ));
    assert_eq!(player.player().stamina, 6, "drink must restore 4 stamina");
    assert!(
        bottle.get_name().eq_ignore_ascii_case("Empty Bottle"),
        "drinking must leave an Empty Bottle"
    );

    // a full player refuses the drink (never wasted)
    player.player_mut().stamina = MAX_STAMINA;
    let mut second = registry::get(&tw.g, "Water Bottle");
    assert!(!fdoom::item::interact::item_interact_on_tile(
        &mut tw.g,
        &mut second,
        lvl,
        px,
        py,
        &mut player,
        Direction::Down,
    ));
    tw.g.entities.put_back(player);

    // refill: empty bottle on open water becomes a Water Bottle again
    let (wx, wy) = tw.place("water", 1, 0);
    let mut player = tw.g.entities.take(tw.g.player_id).expect("player");
    assert!(fdoom::item::interact::item_interact_on_tile(
        &mut tw.g,
        &mut bottle,
        lvl,
        wx,
        wy,
        &mut player,
        Direction::Down,
    ));
    assert!(
        bottle.get_name().eq_ignore_ascii_case("Water Bottle"),
        "refill must give the Water Bottle back"
    );
    tw.g.entities.put_back(player);
}

#[test]
fn old_food_can_feeds_leaves_the_can_and_sometimes_churns() {
    let mut tw = TestWorld::infinite().seed(11).build();
    let lvl = tw.g.current_level;
    let (px, py) = tw.player_tile();
    tw.g.random = Rng::new(99); // pin the churn rolls

    let mut can = registry::get(&tw.g, "Old Food Can_8");
    let mut churned = false;
    let mut eaten = 0;
    for _ in 0..8 {
        {
            let pd = tw.g.player_mut().player_mut();
            pd.hunger = 2;
            pd.stamina = 10;
        }
        let mut player = tw.g.entities.take(tw.g.player_id).expect("player");
        assert!(fdoom::item::interact::item_interact_on_tile(
            &mut tw.g,
            &mut can,
            lvl,
            px,
            py,
            &mut player,
            Direction::Down,
        ));
        eaten += 1;
        assert!(player.player().hunger > 2, "the can must still feed");
        if player.player().stamina < 10 - 5 {
            // 5 paid to eat; anything past that is the churn's knock
            churned = true;
        }
        tw.g.entities.put_back(player);
    }
    let empties = {
        let inv = &tw.g.player().player().inventory;
        inv.count(&registry::get(&tw.g, "Empty Can"))
    };
    assert_eq!(empties, eaten, "every can eaten must leave an Empty Can");
    assert!(
        churned,
        "the pinned RNG must land at least one churn in 8 cans"
    );
    assert!(
        tw.g.notifications
            .iter()
            .any(|n| n.contains("stomach churns")),
        "the churn must be telegraphed"
    );
}

#[test]
fn empty_cans_melt_into_tin_at_the_furnace() {
    let g = bare_game("tin_melt");
    let recipe = g
        .recipes
        .furnace
        .iter()
        .find(|r| r.product_name().eq_ignore_ascii_case("Tin"))
        .expect("no Tin recipe at the furnace");

    let mut inv = Inventory::new();
    inv.add(registry::get(&g, "Empty Can_3"));
    inv.add(registry::get(&g, "Coal_1"));
    assert!(recipe.craft(&g, &mut inv), "can -> tin melt failed");
    assert_eq!(inv.count(&registry::get(&g, "Tin")), 1);
    assert_eq!(inv.count(&registry::get(&g, "Empty Can")), 0);

    // and the tin is actually good for something at the workbench
    assert!(
        g.recipes
            .workbench
            .iter()
            .any(|r| r.get_costs().iter().any(|(c, _)| c == "TIN")),
        "no workbench recipe consumes Tin"
    );
}

#[test]
fn scav_container_save_roundtrip_preserves_kind_and_searched() {
    let mut tw = TestWorld::infinite().seed(5).name("townsscav").build();
    let lvl = tw.g.current_level;
    let (px, py) = tw.player_tile();

    let mut full = fdoom::entity::furniture::scav_container::new(ScavKind::Cupboard);
    full.chest_mut()
        .unwrap()
        .inventory
        .add(registry::get(&tw.g, "Old Coin_2"));
    let mut emptied = fdoom::entity::furniture::scav_container::new(ScavKind::Barrel);
    if let EntityKind::ScavContainer(sc) = &mut emptied.kind {
        sc.searched = true;
    }
    tw.g.level_mut(lvl).add_at(full, px + 1, py, true, lvl);
    tw.g.level_mut(lvl).add_at(emptied, px + 2, py, true, lvl);
    tw.g.tick();

    let name = tw.g.world_name.clone();
    save::save_world_named(&mut tw.g, &name);

    let mut g2 = Game::new(false, false, tw.g.game_dir.clone());
    let mut player = fdoom::entity::mob::player::new(&g2, None);
    player.c.eid = 0;
    g2.entities.put_back(player);
    load::load_world_named(&mut g2, &name);

    // loaded entities sit in the level's add-queue until the first tick drains it
    let find = |tx: i32, ty: i32| -> (ScavKind, bool, i32) {
        g2.level(lvl)
            .entities_to_add
            .iter()
            .find_map(|e| match &e.kind {
                EntityKind::ScavContainer(sc) if (e.c.x >> 4, e.c.y >> 4) == (tx, ty) => Some((
                    sc.kind,
                    sc.searched,
                    sc.chest.inventory.count(&registry::get(&g2, "Old Coin")),
                )),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no ScavContainer reloaded at ({tx}, {ty})"))
    };
    let (k1, s1, coins) = find(px + 1, py);
    assert_eq!(k1, ScavKind::Cupboard);
    assert!(!s1, "unsearched container reloaded as searched");
    assert_eq!(coins, 2, "container inventory lost in the roundtrip");
    let (k2, s2, _) = find(px + 2, py);
    assert_eq!(k2, ScavKind::Barrel);
    assert!(s2, "searched flag lost in the roundtrip");
}

/* --------------------------------- screenshots --------------------------------- */

fn shot(tw: &mut TestWorld, name: &str) {
    if let Ok(dir) = std::env::var("TOWNS_SHOT_DIR") {
        let pixels = tw.render();
        let path = std::path::Path::new(&dir).join(name);
        save_png(&path, &pixels, screen::W as usize, screen::H as usize, 2);
    }
}

#[test]
fn town_age_screenshots() {
    if std::env::var("TOWNS_SHOT_DIR").is_err() {
        return;
    }
    for (age, label) in [
        (TownAge::Overgrown, "overgrown"),
        (TownAge::Weathered, "weathered"),
        (TownAge::Settled, "settled"),
    ] {
        let (s, p) = find_town(StructureKind::Village, age);
        let mut tw = TestWorld::infinite().seed(s).build();
        tw.g.change_time_of_day(fdoom::core::updater::Time::Day); // full daylight
        tw.teleport(p.x, p.y + 2);
        tw.tick_n(12);
        shot(&mut tw, &format!("town_{label}.png"));
        // a house-level shot: how the walls/floors read at this age (lantern spots
        // mark houses; overgrown towns may only have a container to aim at)
        let house_spot = lantern_positions(s, p)
            .first()
            .copied()
            .or_else(|| container_positions(s, p).first().map(|c| (c.0, c.1)));
        if let Some((hx, hy)) = house_spot {
            tw.teleport(hx, hy + 1);
            tw.tick_n(8);
            shot(&mut tw, &format!("town_{label}_house.png"));
        }
        // a house interior for the settled town (containers + lantern)
        if age == TownAge::Settled {
            if let Some((cx, cy, _)) = container_positions(s, p).first().copied() {
                tw.teleport(cx, cy + 1);
                tw.tick_n(8);
                shot(&mut tw, "town_house_interior.png");
            }
        }
    }
    // emptied-state readability: the same container before and after the rummage
    let (mut tw, eid, _) = boot_at_container();
    tw.g.change_time_of_day(fdoom::core::updater::Time::Day);
    tw.tick_n(4);
    shot(&mut tw, "container_full.png");
    let mut e = tw.g.entities.take(eid).expect("container");
    let mut player = tw.g.entities.take(tw.g.player_id).expect("player");
    fdoom::entity::furniture::behavior::use_furniture(&mut tw.g, &mut e, &mut player);
    tw.g.entities.put_back(player);
    tw.g.entities.put_back(e);
    tw.tick_n(30); // let the spilled drops settle around it
    shot(&mut tw, "container_emptied.png");
}
