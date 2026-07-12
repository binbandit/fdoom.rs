//! The fishing wave (invisible fish): no fish mobs — the `weather::fish_presence`
//! field sets the per-cast odds (bubbling hotspots ~3x base, dead water much worse,
//! rain a bonus), and the kind of water sets the catch table (open water's classic
//! trio, Deep Water's Big Fish + rare treasure, underground pools' Cave Eels).

use fdoom::core::updater::DAY_LENGTH;
use fdoom::core::weather::{self, FISH_PRESENCE_THRESHOLD};
use fdoom::entity::Direction;
use fdoom::entity::mob::player_behavior::{fishing_catch_chance, go_fishing};
use fdoom::item::{Inventory, ItemKind, interact, registry};
use fdoom::level::infinite_gen::Biome;
use fdoom::level::tile::tidal;
use fdoom::testutil::{TestWorld, bare_game, find_recipe};

/// Scan outward from `(cx, cy)` for a tile whose fish presence satisfies `pred`.
fn find_presence(seed: i64, cx: i32, cy: i32, pred: impl Fn(f64) -> bool) -> (i32, i32) {
    for r in 0..200 {
        for dx in -r..=r {
            for dy in [-r, r] {
                for (x, y) in [(cx + dx, cy + dy), (cx + dy, cy + dx)] {
                    if pred(weather::fish_presence(seed, x, y)) {
                        return (x, y);
                    }
                }
            }
        }
    }
    panic!("no tile with the wanted fish presence within 200 tiles of ({cx}, {cy})");
}

/// Cast `n` times at tile `(xt, yt)` through `go_fishing` directly (the rod's
/// interact gate is exercised separately) and return only the *new* drops — the
/// level isn't ticked between the before/after snapshots, so nothing else can drop.
fn cast_batch(tw: &mut TestWorld, xt: i32, yt: i32, n: usize) -> Vec<String> {
    let before = tw.dropped_items();
    let mut player = tw.g.entities.take(tw.g.player_id).expect("player");
    let (px, py) = (player.c.x - 5, player.c.y - 5);
    for _ in 0..n {
        go_fishing(&mut tw.g, &mut player, px, py, xt, yt);
    }
    tw.g.entities.put_back(player);
    let mut new = tw.dropped_items();
    for b in &before {
        if let Some(i) = new.iter().position(|a| a == b) {
            new.remove(i);
        }
    }
    new
}

fn count(drops: &[String], name: &str) -> usize {
    drops
        .iter()
        .filter(|d| d.eq_ignore_ascii_case(name))
        .count()
}

/* ------------------------------- odds ------------------------------- */

#[test]
fn catch_chance_multipliers() {
    // The bubble edge is exactly the 3x hotspot edge: ~3x jump crossing it.
    let below = fishing_catch_chance(FISH_PRESENCE_THRESHOLD - 0.001, false);
    let at = fishing_catch_chance(FISH_PRESENCE_THRESHOLD, false);
    assert!(
        (2.8..=3.2).contains(&(at / below)),
        "hotspot edge should be ~3x: {below} -> {at}"
    );

    // Dead water is much worse than the hotspot (and than mid water).
    let dead = fishing_catch_chance(0.0, false);
    let mid = fishing_catch_chance(0.45, false);
    assert!(dead < mid && mid < at, "odds must grow with presence");
    assert!(at / dead > 8.0, "hotspot vs dead water: {at} vs {dead}");

    // Rain is a flat 1.3x bonus, and the total is capped below certainty.
    let dry = fishing_catch_chance(0.3, false);
    let wet = fishing_catch_chance(0.3, true);
    assert!((wet / dry - 1.3).abs() < 1e-9, "rain bonus: {dry} -> {wet}");
    assert!(fishing_catch_chance(1.0, true) <= 0.95);
}

#[test]
fn bubbling_water_out_fishes_dead_water() {
    let mut tw = TestWorld::infinite().build();
    let seed = tw.world_seed;
    let (px, py) = tw.player_tile();
    let (bx, by) = find_presence(seed, px, py, |p| p >= 0.75);
    let (dx, dy) = find_presence(seed, px, py, |p| p <= 0.12);

    const CASTS: usize = 400;
    let bubbling = cast_batch(&mut tw, bx, by, CASTS).len();
    let dead = cast_batch(&mut tw, dx, dy, CASTS).len();
    assert!(
        bubbling > 3 * dead.max(1),
        "hotspot should far out-fish dead water: {bubbling} vs {dead} over {CASTS} casts"
    );
    assert!(bubbling > 100, "hotspot catch rate collapsed: {bubbling}");
    assert!(dead < 80, "dead water catches too much: {dead}");

    // Casting on the bubbling spot cues the flavor note (once, deduped).
    assert!(
        tw.notifications.iter().any(|n| n.contains("stirs")),
        "no hotspot cue in {:?}",
        tw.notifications
    );
}

#[test]
fn rain_improves_the_bite() {
    let mut tw = TestWorld::infinite().build();
    tw.goto_biome(Biome::Forest); // rain presents as rain here (no desert/tundra gate)
    let seed = tw.world_seed;
    let (px, py) = tw.player_tile();
    // A mid-presence tile, well clear of the hotspot edge in both weathers.
    let (fx, fy) = find_presence(seed, px, py, |p| (0.3..=0.5).contains(&p));

    // Find a raining slice and pin the clock to its plateau (mid-slice).
    let (day, slice) = (1..=200)
        .flat_map(|d| (0..weather::SLICES_PER_DAY).map(move |s| (d, s)))
        .find(|&(d, s)| weather::slice_raining(seed, d, s))
        .expect("no rain in 200 days?");
    let mid = slice * weather::SLICE_LEN + weather::SLICE_LEN / 2;
    tw.set_time(mid - 1);
    tw.tick_n(1);

    const CASTS: usize = 1000;
    // Dry control first: day 0 never rains.
    tw.events.day_number = 0;
    assert!(!weather::is_raining(&tw.g), "day 0 must be dry");
    let dry = cast_batch(&mut tw, fx, fy, CASTS).len();

    tw.events.day_number = day;
    assert!(weather::is_raining(&tw.g), "pinned tick should be raining");
    let wet = cast_batch(&mut tw, fx, fy, CASTS).len();

    assert!(
        wet > dry,
        "rain should improve the bite: {wet} wet vs {dry} dry over {CASTS} casts"
    );
    // The rain flavor cue fired on a non-bubbling spot.
    assert!(
        tw.notifications.iter().any(|n| n.contains("biting")),
        "no rain-bite cue in {:?}",
        tw.notifications
    );
}

/* ------------------------------- catch tables ------------------------------- */

#[test]
fn deep_water_yields_the_big_table() {
    let mut tw = TestWorld::infinite().build();
    let seed = tw.world_seed;
    let (px, py) = tw.player_tile();
    let (bx, by) = find_presence(seed, px, py, |p| p >= 0.75);
    tw.teleport(bx, by);
    tw.tick_n(8); // stream the chunks in
    tw.place_at("Deep Water", bx, by);
    assert_eq!(tw.tile_at(tw.current_level, bx, by).name, "DEEP WATER");

    let drops = cast_batch(&mut tw, bx, by, 900);
    assert!(count(&drops, "Raw Fish") > 0, "deep water still holds fish");
    assert!(
        count(&drops, "Big Fish") > 0,
        "no Big Fish out deep: {drops:?}"
    );
    assert!(
        count(&drops, "gem") + count(&drops, "Iron") > 0,
        "no treasure out deep: {drops:?}"
    );
    // Big Fish is the rare one, treasure rarer still.
    assert!(count(&drops, "Big Fish") < count(&drops, "Raw Fish"));
    assert!(count(&drops, "gem") + count(&drops, "Iron") < count(&drops, "Big Fish"));
    assert_eq!(count(&drops, "Cave Eel"), 0);
    assert_eq!(count(&drops, "Leather Armor"), 0);
}

#[test]
fn regular_water_keeps_the_classic_trio_and_caves_hold_eels() {
    let mut tw = TestWorld::infinite().build();
    let seed = tw.world_seed;
    let (px, py) = tw.player_tile();
    let (bx, by) = find_presence(seed, px, py, |p| p >= 0.75);

    // Surface (regular water): mostly Raw Fish, no deep-water prizes.
    let drops = cast_batch(&mut tw, bx, by, 400);
    assert!(count(&drops, "Raw Fish") > count(&drops, "Slime"));
    assert_eq!(count(&drops, "Big Fish"), 0);
    assert_eq!(count(&drops, "Cave Eel"), 0);

    // Underground pools (depth < 0): the eel table.
    tw.g.player_mut().c.level = Some(2);
    tw.g.current_level = 2;
    let drops = cast_batch(&mut tw, bx, by, 400);
    assert!(
        count(&drops, "Cave Eel") > drops.len() / 2,
        "cave pools should mostly yield eels: {drops:?}"
    );
    assert_eq!(count(&drops, "Raw Fish"), 0);
}

/* --------------------------- the rod's interact gate --------------------------- */

/// Cast the rod at tile `(xt, yt)` through the real interact path; returns
/// (used, durability_delta).
fn rod_cast(tw: &mut TestWorld, xt: i32, yt: i32) -> (bool, i32) {
    let mut rod = registry::get(&tw.g, "Fishing Rod");
    let ItemKind::Tool { dur: before, .. } = rod.kind else {
        panic!("rod is not a tool")
    };
    let lvl = tw.current_level;
    let mut player = tw.g.entities.take(tw.g.player_id).expect("player");
    let used = interact::item_interact_on_tile(
        &mut tw.g,
        &mut rod,
        lvl,
        xt,
        yt,
        &mut player,
        Direction::Down,
    );
    tw.g.entities.put_back(player);
    let ItemKind::Tool { dur: after, .. } = rod.kind else {
        panic!("rod is not a tool")
    };
    (used, before - after)
}

#[test]
fn rod_fishes_all_water_kinds_and_pays_durability() {
    let mut tw = TestWorld::infinite().build();
    let (px, py) = tw.player_tile();

    for water in ["water", "Deep Water"] {
        tw.place_at(water, px + 1, py);
        let (used, paid) = rod_cast(&mut tw, px + 1, py);
        assert!(used, "{water}: the rod should fish here");
        assert_eq!(paid, 1, "{water}: one durability per cast");
    }

    // Dry land: no cast, no durability.
    tw.place_at("grass", px + 1, py);
    let (used, paid) = rod_cast(&mut tw, px + 1, py);
    assert!(!used && paid == 0, "grass is not fishable");
}

#[test]
fn tidal_flats_fish_only_while_submerged() {
    let mut tw = TestWorld::infinite().build();
    let seed = tw.world_seed;

    // A natural flat: its own elevation (land field) decides submersion, so a
    // placed tile won't do — walk to a generated one mid-band.
    let (fx, fy) = {
        let mut found = None;
        'scan: for r in 0..2000 {
            for (x, y) in [(r, 0), (-r, 0), (0, r), (0, -r)] {
                let land = fdoom::level::infinite_gen::land_at(seed, x, y);
                if (tidal::BAND_LOW + 0.005..tidal::BAND_HIGH - 0.005).contains(&land) {
                    found = Some((x, y));
                    break 'scan;
                }
            }
        }
        found.expect("no tidal-band tile on an axis within 2000 tiles")
    };
    tw.teleport(fx + 2, fy);
    tw.tick_n(8);
    assert_eq!(tw.tile_at(tw.current_level, fx, fy).name, "TIDAL FLAT");

    // High tide (tick 0 / DAY_LENGTH/2): the flat is under water — fishable.
    tw.set_time(DAY_LENGTH / 2 - 1);
    tw.tick_n(1);
    assert!(tidal::is_submerged(&tw.g, fx, fy));
    let (used, _) = rod_cast(&mut tw, fx, fy);
    assert!(used, "submerged flat should fish like regular water");

    // Low tide (DAY_LENGTH/4): exposed wet sand — no cast.
    tw.set_time(DAY_LENGTH / 4 - 1);
    tw.tick_n(1);
    assert!(!tidal::is_submerged(&tw.g, fx, fy));
    let (used, _) = rod_cast(&mut tw, fx, fy);
    assert!(!used, "exposed flat must not fish");
}

/* ------------------------------- cast feedback ------------------------------- */

/// Bobbers waiting in the current level's add-queue (casts spawn them there; they
/// enter the arena on the next tick). A bobber is the only particle with `bob` set.
fn queued_bobbers(tw: &TestWorld) -> usize {
    tw.g.level(tw.g.current_level)
        .entities_to_add
        .iter()
        .filter(|e| matches!(&e.kind, fdoom::entity::EntityKind::Particle(p) if p.bob > 0.0))
        .count()
}

#[test]
fn water_cast_spawns_a_bobber_and_dirt_cast_says_so() {
    let mut tw = TestWorld::infinite().build();
    let (px, py) = tw.player_tile();

    // A cast that lands on water: a bobber (and no "dirt" line).
    tw.place_at("water", px + 1, py);
    assert_eq!(queued_bobbers(&tw), 0);
    let (used, _) = rod_cast(&mut tw, px + 1, py);
    assert!(used, "water should take the cast");
    assert_eq!(queued_bobbers(&tw), 1, "a water cast should float a bobber");
    assert!(
        !tw.notifications.iter().any(|n| n.contains("dirt")),
        "water cast must not cue the dirt line: {:?}",
        tw.notifications
    );
    tw.tick_n(1); // move the bobber (and splash ring) into the arena

    // A cast that lands on dry ground: no bobber, but the line says where it went.
    tw.place_at("grass", px + 1, py);
    let (used, _) = rod_cast(&mut tw, px + 1, py);
    assert!(!used, "grass is not fishable");
    assert_eq!(queued_bobbers(&tw), 0, "no bobber on dry ground");
    assert!(
        tw.notifications
            .iter()
            .any(|n| n == "The line lands in the dirt."),
        "dirt cast should cue the landing line: {:?}",
        tw.notifications
    );
}

#[test]
fn bobber_persists_a_moment_then_sinks() {
    let mut tw = TestWorld::infinite().build();
    let (px, py) = tw.player_tile();
    tw.place_at("water", px + 1, py);
    rod_cast(&mut tw, px + 1, py);
    tw.tick_n(1); // into the arena

    let lvl = tw.g.current_level;
    let bobber_live = |tw: &TestWorld| {
        tw.g.entities.ids_on_level(lvl).into_iter().any(|id| {
            matches!(
                tw.g.entities.get(id).map(|e| &e.kind),
                Some(fdoom::entity::EntityKind::Particle(p)) if p.bob > 0.0
            )
        })
    };
    assert!(bobber_live(&tw), "bobber should be live after the cast");
    tw.tick_n(15);
    assert!(bobber_live(&tw), "bobber should persist ~20+ ticks");
    tw.tick_n(30);
    assert!(!bobber_live(&tw), "bobber should be gone within ~30 ticks");
}

/* ------------------------------- the new items ------------------------------- */

#[test]
fn fish_items_register_cook_and_heal() {
    let g = bare_game("fishing_items");

    // Heal values: raw on the raw scale, Cooked Big Fish the payoff at 5.
    for (name, want) in [
        ("Big Fish", 2),
        ("Cooked Big Fish", 5),
        ("Cave Eel", 1),
        ("Cooked Cave Eel", 3),
    ] {
        let item = registry::get(&g, name);
        let ItemKind::Food { heal, .. } = item.kind else {
            panic!("{name} is not a Food item");
        };
        assert_eq!(heal, want, "{name} heal value");
    }

    // Both cook at the oven from exactly their listed costs.
    for product in ["Cooked Big Fish", "Cooked Cave Eel"] {
        let recipe = find_recipe(&g.recipes.oven, product).clone();
        let mut inv = Inventory::new();
        for (cost, amt) in recipe.get_costs() {
            inv.add(registry::get(&g, &format!("{cost}_{amt}")));
        }
        assert!(recipe.craft(&g, &mut inv), "{product}: cooking failed");
        assert!(
            inv.count(&registry::get(&g, product)) >= 1,
            "{product}: missing after cooking"
        );
    }
}
