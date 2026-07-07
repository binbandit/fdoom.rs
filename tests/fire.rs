//! Fire & campfire wave: campfire fuel lifecycle (drain / refuel / ember / relight),
//! the rest-by-the-fire stamina bonus, mushroom cooking, tile-fire spread through
//! flammable runs (and its hard stop at stone), rain extinguishing, burn-out
//! products, the cold-camp ember spawn, and a night gallery screenshot.

use fdoom::core::updater::Time;
use fdoom::core::weather;
use fdoom::entity::furniture::campfire::{
    FUEL_PER_WOOD, LIGHT_RADIUS, MAX_FUEL, START_FUEL, ember_sprite, lit_sprite,
};
use fdoom::entity::furniture::{campfire, campfire_behavior};
use fdoom::entity::{Direction, EntityKind, behavior};
use fdoom::item::{Item, registry};
use fdoom::level::structures_gen::{StructureKind, placements_in_rect, variant_of};
use fdoom::level::tile::fire;
use fdoom::testutil::TestWorld;

const SEED: i64 = 20260707;

/// Pave a `(2r+1)²` dirt apron around the player so nothing flammable sits near a
/// test fire (and no pond can smother one).
fn pave(tw: &mut TestWorld, r: i32) {
    for dy in -r..=r {
        for dx in -r..=r {
            tw.place("dirt", dx, dy);
        }
    }
}

/// Add a campfire entity `(dx, dy)` tiles from the player and return its eid.
fn add_campfire(tw: &mut TestWorld, dx: i32, dy: i32) -> i32 {
    let (px, py) = tw.player_tile();
    let lvl = tw.current_level;
    let e = campfire::new();
    tw.g.level_mut(lvl).add_at(e, px + dx, py + dy, true, lvl);
    tw.tick_n(1); // drain into the arena
    campfire_eids(tw)[0]
}

fn campfire_eids(tw: &TestWorld) -> Vec<i32> {
    let lvl = tw.current_level;
    tw.g.entities
        .entities_on_level(lvl)
        .filter(|e| matches!(e.kind, EntityKind::Campfire(_)))
        .map(|e| e.c.eid)
        .collect()
}

fn campfire_fuel(tw: &TestWorld, eid: i32) -> i32 {
    match &tw.g.entities.get(eid).expect("campfire").kind {
        EntityKind::Campfire(cf) => cf.fuel,
        _ => panic!("not a campfire"),
    }
}

fn set_campfire_fuel(tw: &mut TestWorld, eid: i32, fuel: i32) {
    if let Some(e) = tw.g.entities.get_mut(eid) {
        if let EntityKind::Campfire(cf) = &mut e.kind {
            cf.fuel = fuel;
        }
    }
}

/// Run `campfire_behavior::interact` the way the player attack path does (player
/// taken out, held item as an Option). Returns (handled, item afterwards).
fn interact_campfire(tw: &mut TestWorld, eid: i32, item: Option<Item>) -> (bool, Option<Item>) {
    let mut item = item;
    let mut player = tw.g.entities.take(tw.g.player_id).expect("player");
    let handled =
        tw.g.with_entity(eid, |cf, g| {
            campfire_behavior::interact(g, cf, &mut player, &mut item, Direction::Down)
        })
        .unwrap_or(false);
    tw.g.entities.put_back(player);
    (handled, item)
}

/* --------------------------- campfire fuel lifecycle --------------------------- */

#[test]
fn fuel_drains_to_ember_and_wood_relights() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    pave(&mut tw, 4);
    let eid = add_campfire(&mut tw, 2, 0);

    // fresh: lit with the 2 crafting Wood (minus the ticks since placement)
    let fuel0 = campfire_fuel(&tw, eid);
    assert!(fuel0 > START_FUEL - 60 && fuel0 <= START_FUEL, "{fuel0}");
    assert_eq!(
        behavior::get_light_radius(tw.g.entities.get(eid).unwrap()),
        LIGHT_RADIUS
    );

    // burn down: fuel ticks away 1:1, then the fire dies to a dark ember
    set_campfire_fuel(&mut tw, eid, 5);
    tw.tick_n(10);
    assert_eq!(campfire_fuel(&tw, eid), 0);
    let e = tw.g.entities.get(eid).unwrap();
    assert_eq!(behavior::get_light_radius(e), 0, "ember gives no light");
    assert_eq!(
        e.furniture().unwrap().sprite.get_pos(),
        ember_sprite().get_pos(),
        "ember art"
    );

    // relight: interact with Wood in hand — consumes 1, restores a wood of fuel
    let wood = registry::get(&tw.g, "Wood_3");
    let (handled, left) = interact_campfire(&mut tw, eid, Some(wood));
    assert!(handled);
    assert_eq!(left.unwrap().count(), 2, "one Wood consumed");
    assert_eq!(campfire_fuel(&tw, eid), FUEL_PER_WOOD);
    let e = tw.g.entities.get(eid).unwrap();
    assert_eq!(behavior::get_light_radius(e), LIGHT_RADIUS, "relit");
    assert_eq!(
        e.furniture().unwrap().sprite.get_pos(),
        lit_sprite().get_pos()
    );

    // cap: feeding past MAX_FUEL is refused (wood not consumed)
    set_campfire_fuel(&mut tw, eid, MAX_FUEL);
    let one_wood = registry::get(&tw.g, "Wood_1");
    let (handled, left) = interact_campfire(&mut tw, eid, Some(one_wood));
    assert!(handled);
    assert_eq!(left.unwrap().count(), 1, "full fire refuses more wood");
    assert_eq!(campfire_fuel(&tw, eid), MAX_FUEL);

    // empty-handed interact reads the fuel state
    tw.notifications.clear();
    let (handled, _) = interact_campfire(&mut tw, eid, None);
    assert!(handled);
    assert!(!tw.notifications.is_empty(), "fuel-state notification");
}

/* ------------------------------- rest by the fire ------------------------------- */

#[test]
fn lit_campfire_doubles_stamina_regen() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    pave(&mut tw, 4);

    let gain_over = |tw: &mut TestWorld, ticks: usize| {
        tw.g.with_entity(tw.g.player_id, |p, _| {
            let pd = p.player_mut();
            pd.stamina = 1;
            pd.stamina_recharge = 0;
            pd.stamina_recharge_delay = 0;
        });
        tw.tick_n(ticks);
        tw.player().player().stamina - 1
    };

    let cold = gain_over(&mut tw, 60);
    let eid = add_campfire(&mut tw, 1, 0); // within 2 tiles: resting range
    let warm = gain_over(&mut tw, 60);
    assert!(
        warm >= cold * 2 - 1 && warm > cold,
        "2x regen by the fire: cold {cold}, warm {warm}"
    );

    // an ember gives no bonus
    set_campfire_fuel(&mut tw, eid, 0);
    let ember = gain_over(&mut tw, 60);
    assert!(ember <= cold + 1, "ember: {ember} vs cold {cold}");
}

/* ----------------------------------- cooking ----------------------------------- */

#[test]
fn lit_campfire_cooks_mushrooms() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    pave(&mut tw, 4);
    let eid = add_campfire(&mut tw, 2, 0);

    let shrooms = registry::get(&tw.g, "Mushroom_2");
    let (handled, left) = interact_campfire(&mut tw, eid, Some(shrooms));
    assert!(handled);
    assert_eq!(left.unwrap().count(), 1, "one Mushroom consumed");
    let cooked = registry::get(&tw.g, "Cooked Mushroom");
    assert_eq!(tw.player().player().inventory.count(&cooked), 1);

    // a cold ember cooks nothing
    set_campfire_fuel(&mut tw, eid, 0);
    let shroom = registry::get(&tw.g, "Mushroom_1");
    let (handled, left) = interact_campfire(&mut tw, eid, Some(shroom));
    assert!(!handled, "ember can't cook");
    assert_eq!(left.unwrap().count(), 1);
}

/* --------------------------------- fire spread --------------------------------- */

#[test]
fn fire_spreads_along_planks_but_not_across_stone() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    pave(&mut tw, 9);
    let (px, py) = tw.player_tile();
    let lvl = tw.current_level;

    // a 2-wide plank corridor, a 1-column stone firebreak, more planks beyond
    let y0 = py + 3;
    let x0 = px - 5;
    for x in x0..x0 + 6 {
        tw.place_at("Wood Planks", x, y0);
        tw.place_at("Wood Planks", x, y0 + 1);
    }
    for y in [y0, y0 + 1] {
        tw.place_at("Stone Bricks", x0 + 6, y); // the firebreak
        tw.place_at("Wood Planks", x0 + 7, y);
        tw.place_at("Wood Planks", x0 + 8, y);
    }

    assert!(fire::ignite(&mut tw.g, lvl, x0, y0), "planks must ignite");
    assert!(fire::is_burning(&tw.g, lvl, x0, y0));
    assert!(
        !fire::ignite(&mut tw.g, lvl, x0 + 6, y0),
        "stone must not ignite"
    );

    // burn until the corridor fire is entirely out (or the budget runs dry)
    let region_burning = |tw: &TestWorld| {
        (x0..=x0 + 8)
            .any(|x| fire::is_burning(&tw.g, lvl, x, y0) || fire::is_burning(&tw.g, lvl, x, y0 + 1))
    };
    let mut reached_far_plank = false;
    for _ in 0..200 {
        tw.tick_n(100);
        let far = tw.g.tile_at(lvl, x0 + 5, y0).name.clone();
        if far == "DIRT" || fire::is_burning(&tw.g, lvl, x0 + 5, y0) {
            reached_far_plank = true;
        }
        if reached_far_plank && !region_burning(&tw) {
            break;
        }
    }
    assert!(reached_far_plank, "fire failed to run the plank corridor");
    assert!(!region_burning(&tw), "fire failed to burn out in budget");

    // the corridor burned through to dirt...
    assert_eq!(tw.g.tile_at(lvl, x0 + 5, y0).name, "DIRT");
    // ...but the firebreak held: nothing past the stone ever burned
    assert_eq!(tw.g.tile_at(lvl, x0 + 6, y0).name, "STONE BRICKS");
    for y in [y0, y0 + 1] {
        assert_eq!(tw.g.tile_at(lvl, x0 + 7, y).name, "WOOD PLANKS");
        assert_eq!(tw.g.tile_at(lvl, x0 + 8, y).name, "WOOD PLANKS");
    }
}

#[test]
fn grass_fire_stays_contained_even_in_dense_fuel() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    pave(&mut tw, 12);
    let (px, py) = tw.player_tile();
    let lvl = tw.current_level;

    // a solid 17x17 tall-grass field around the player — worst-case light fuel
    let r = 8;
    for dy in -r..=r {
        for dx in -r..=r {
            if (dx, dy) != (0, 0) {
                tw.place_at("tall grass", px + dx, py + dy);
            }
        }
    }
    assert!(fire::ignite(&mut tw.g, lvl, px + 1, py));

    // run until the field holds no fire at all
    let any_burning = |tw: &TestWorld| {
        (-r..=r).any(|dy| (-r..=r).any(|dx| fire::is_burning(&tw.g, lvl, px + dx, py + dy)))
    };
    let mut out = false;
    for _ in 0..80 {
        tw.tick_n(100);
        if !any_burning(&tw) {
            out = true;
            break;
        }
    }
    assert!(out, "grass fire must die out on its own");

    // ...having charred only a handful of tiles, not the field
    let burned = (-r..=r)
        .flat_map(|dy| (-r..=r).map(move |dx| (dx, dy)))
        .filter(|&(dx, dy)| tw.g.tile_at(lvl, px + dx, py + dy).name == "DIRT")
        .count();
    let field = (2 * r as usize + 1).pow(2) - 1;
    assert!(
        burned >= 1 && burned < field / 4,
        "contained: {burned} of {field} tiles burned"
    );
}

/* ------------------------------- burn-out products ------------------------------- */

#[test]
fn burn_products_tree_to_dirt_wall_to_planks() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    pave(&mut tw, 6);
    let lvl = tw.current_level;
    let (px, py) = tw.player_tile();

    let (tree_x, tree_y) = (px + 3, py - 3);
    let (wall_x, wall_y) = (px - 3, py + 3);
    tw.place_at("tree", tree_x, tree_y);
    tw.place_at("Wood Wall", wall_x, wall_y);
    assert!(fire::ignite(&mut tw.g, lvl, tree_x, tree_y));
    assert!(fire::ignite(&mut tw.g, lvl, wall_x, wall_y));

    for _ in 0..120 {
        tw.tick_n(100);
        if !fire::is_burning(&tw.g, lvl, tree_x, tree_y)
            && !fire::is_burning(&tw.g, lvl, wall_x, wall_y)
        {
            break;
        }
    }
    assert_eq!(tw.g.tile_at(lvl, tree_x, tree_y).name, "DIRT", "tree chars");
    assert_eq!(
        tw.g.tile_at(lvl, wall_x, wall_y).name,
        "WOOD PLANKS",
        "wall collapses into plank rubble"
    );
}

/* ------------------------------- rain extinguishes ------------------------------- */

#[test]
fn heavy_rain_puts_fires_out() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    pave(&mut tw, 6);
    let lvl = tw.current_level;
    let (px, py) = tw.player_tile();

    // find a heavy-rain moment in the pure schedule (mid-slice plateau > 0.6)
    let slice_len = fdoom::core::weather::SLICE_LEN;
    let (day, tick) = (1..120)
        .flat_map(|d| (0..fdoom::core::weather::SLICES_PER_DAY).map(move |s| (d, s)))
        .map(|(d, s)| (d, s * slice_len + slice_len / 2))
        .find(|&(d, t)| weather::schedule_intensity(SEED, d, t) > 0.6)
        .expect("no heavy rain in 120 days?");

    // pin the day clock there (weather is pure f(seed, day, tick))
    tw.set_time(tick - 1);
    tw.tick_n(1);
    tw.events.day_number = day;
    assert!(
        weather::extinguishes_fire(&tw.g),
        "picked moment must be a downpour at the player"
    );

    let (bx, by) = (px + 3, py + 3);
    tw.place_at("Wood Wall", bx, by);
    assert!(fire::ignite(&mut tw.g, lvl, bx, by));
    tw.tick_n(600); // a handful of burn ticks' worth
    assert!(
        !fire::is_burning(&tw.g, lvl, bx, by),
        "rain must douse the fire"
    );
    assert_eq!(
        tw.g.tile_at(lvl, bx, by).name,
        "WOOD WALL",
        "doused, not burned out — the wall survives"
    );
}

/* ------------------------------ cold-camp ember spawn ------------------------------ */

#[test]
fn cold_camps_spawn_an_ember_campfire() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    let seed = tw.world_seed;

    // nearest cold-camp (variant != 0) placement
    let camp = placements_in_rect(seed, -4096, -4096, 4096, 4096)
        .into_iter()
        .filter(|p| p.kind == StructureKind::Camp && variant_of(seed, *p) != 0)
        .min_by_key(|p| p.x.abs() + p.y.abs())
        .expect("no cold camp within range");

    tw.teleport(camp.x, camp.y + 2);
    tw.tick_n(10); // stream the chunks in (fresh generation spawns the entities)

    let lvl = tw.current_level;
    let ember =
        tw.g.entities
            .entities_on_level(lvl)
            .find(|e| matches!(&e.kind, EntityKind::Campfire(cf) if cf.fuel == 0))
            .expect("cold camp must hold a burnt-out campfire");
    assert_eq!((ember.c.x >> 4, ember.c.y >> 4), (camp.x, camp.y));

    // and the old still-burning torch is gone from cold camps
    assert_ne!(tw.g.tile_at(lvl, camp.x, camp.y).name, "TORCH DIRT");
}

/* --------------------------------- night gallery --------------------------------- */

#[test]
fn night_screenshot_with_smoke_and_fire_glow() {
    let mut tw = TestWorld::infinite().seed(SEED).build();
    pave(&mut tw, 7);
    add_campfire(&mut tw, 2, -1);
    let lvl = tw.current_level;
    let (px, py) = tw.player_tile();

    // a burning tree off to the side for the tile-fire overlay + flicker light
    tw.place_at("tree", px - 4, py - 2);
    tw.change_time_of_day(Time::Night);
    tw.tick_n(45); // a couple of smoke puffs into the air
    fire::ignite(&mut tw.g, lvl, px - 4, py - 2);
    tw.tick_n(2);

    let path = tw.screenshot("fire_night.png");
    assert!(path.exists());
}
