//! Headless tests for the fossicking mining overhaul (see `src/level/tile/fossick.rs`
//! and docs/TERRAIN.md "Fossicking"): prospector's pan, rock character, vein-chasing
//! pings, cave-ins + timber props, and highland (tier-2) mountain rock.

use fdoom::entity::{Direction, EntityKind};
use fdoom::item::registry;
use fdoom::level::infinite_gen::{highland_at, richness_at};
use fdoom::level::tile::fossick::{self, PanFind, RockCharacter, pan_outcome, rock_character};
use fdoom::level::tile::{TileKind, dispatch};
use fdoom::testutil::TestWorld;

/* ---------------------------------- panning ---------------------------------- */

/// The pan table is pure and richness-scaled: identical inputs give identical finds,
/// rich ground pays strictly more often, and the richness field itself is a pure
/// function of the seed.
#[test]
fn pan_finds_are_deterministic_and_richness_scaled() {
    for i in 0..200 {
        let roll = i as f64 / 200.0;
        assert_eq!(pan_outcome(0.37, roll), pan_outcome(0.37, roll));
    }

    let finds = |rich: f64| {
        (0..2000)
            .filter(|i| pan_outcome(rich, *i as f64 / 2000.0) != PanFind::Nothing)
            .count()
    };
    assert!(
        finds(0.9) > finds(0.1),
        "rich ground must pay more often than poor ground"
    );

    // the good colors actually appear on rich ground
    let outcomes: Vec<PanFind> = (0..2000)
        .map(|i| pan_outcome(0.95, i as f64 / 2000.0))
        .collect();
    for want in [PanFind::Gold, PanFind::Gem, PanFind::Iron, PanFind::Coal] {
        assert!(outcomes.contains(&want), "{want:?} never pans on rich dirt");
    }

    // the richness field: deterministic per seed, in [0, 1)
    for (x, y) in [(0, 0), (123, -456), (-9000, 42)] {
        let r = richness_at(77, x, y);
        assert_eq!(r, richness_at(77, x, y));
        assert!((0.0..1.0).contains(&r));
    }
}

/// In-world panning: mud always pans, an exposed tidal flat pans, dry sand away from
/// water refuses, and working a creek long enough turns up a find.
#[test]
fn panning_works_wet_ground_only() {
    let mut tw = TestWorld::infinite().build();

    // dry sand ringed by grass: the pan has nothing to work
    tw.place("sand", -2, 0);
    for (dx, dy) in [(-3, 0), (-1, 0), (-2, -1), (-2, 1)] {
        tw.place("grass", dx, dy);
    }
    assert!(
        !tw.interact_with("Prospector's Pan", -2, 0),
        "dry sand must not pan"
    );

    // an exposed tidal flat pans (inland elevation is always above the tide line)
    tw.place("Tidal Flat", 0, -2);
    tw.g.player_mut().player_mut().stamina = 10;
    assert!(
        tw.interact_with("Prospector's Pan", 0, -2),
        "exposed tidal flat must pan"
    );

    // work a mud bed until something shows in the pan (worst case ~36% per pan)
    tw.place("mud", 1, 0);
    for _ in 0..40 {
        tw.g.player_mut().player_mut().stamina = 10;
        assert!(tw.interact_with("Prospector's Pan", 1, 0), "mud must pan");
    }
    let drops = tw.dropped_items();
    let paying = ["Stone", "Coal", "Iron Ore", "Gold Ore", "gem"];
    assert!(
        drops
            .iter()
            .any(|d| paying.iter().any(|p| d.eq_ignore_ascii_case(p))),
        "40 pans of mud turned up nothing at all: {drops:?}"
    );
}

/* -------------------------------- rock character -------------------------------- */

/// The per-position hash lands near the design ratios: ~20% cracked, ~10% dense.
#[test]
fn rock_character_distribution() {
    let seed = 4242;
    let (mut cracked, mut dense) = (0usize, 0usize);
    let n = 200 * 200;
    for y in 0..200 {
        for x in 0..200 {
            match rock_character(seed, x, y) {
                RockCharacter::Cracked => cracked += 1,
                RockCharacter::Dense => dense += 1,
                RockCharacter::Normal => {}
            }
        }
    }
    let (cf, df) = (cracked as f64 / n as f64, dense as f64 / n as f64);
    assert!((0.17..0.23).contains(&cf), "cracked fraction {cf}");
    assert!((0.08..0.12).contains(&df), "dense fraction {df}");
    // pure per position
    assert_eq!(rock_character(seed, 5, 9), rock_character(seed, 5, 9));
}

/// Cracked rock breaks under 30 damage, plain under 50, dense under 80 — same tile,
/// modulated purely by the position hash.
#[test]
fn cracked_breaks_faster_dense_tougher() {
    let mut tw = TestWorld::infinite().build();
    let seed = tw.world_seed;
    let lvl = tw.current_level;
    let (px, py) = tw.player_tile();

    let find = |want: RockCharacter| -> (i32, i32) {
        for dy in -6..=6 {
            for dx in -6..=6 {
                if (dx, dy) == (0, 0) {
                    continue;
                }
                let (x, y) = (px + dx, py + dy);
                if rock_character(seed, x, y) == want && !highland_at(seed, x, y) {
                    return (x, y);
                }
            }
        }
        panic!("no {want:?} rock position near spawn");
    };

    let rock = tw.tiles.get("rock");
    for (want, survives) in [
        (RockCharacter::Cracked, 29),
        (RockCharacter::Normal, 49),
        (RockCharacter::Dense, 79),
    ] {
        let (x, y) = find(want);
        tw.place_at("rock", x, y);
        dispatch::hurt_dmg(&mut tw.g, &rock, lvl, x, y, survives);
        assert_eq!(
            tw.tile_at(lvl, x, y).name,
            "ROCK",
            "{want:?} rock broke a hit early"
        );
        dispatch::hurt_dmg(&mut tw.g, &rock, lvl, x, y, 1);
        assert_eq!(
            tw.tile_at(lvl, x, y).name,
            "DIRT",
            "{want:?} rock survived its breaking point"
        );
    }
}

/* --------------------------------- vein chasing --------------------------------- */

/// Mining out an ore tile pings hidden ore within 2 tiles with a sparkle particle.
#[test]
fn vein_ping_marks_hidden_ore() {
    let mut tw = TestWorld::infinite().build();
    let lvl = tw.current_level;
    let (px, py) = tw.player_tile();
    let (ax, ay) = (px + 2, py);
    let (bx, by) = (px + 4, py); // 2 tiles from the mined one: in ping range
    tw.place_at("gem ore", ax, ay);
    tw.place_at("gem ore", bx, by);

    let ore = tw.tiles.get("gem ore");
    let mut guard = 0;
    while matches!(tw.tile_at(lvl, ax, ay).kind, TileKind::Ore { .. }) {
        dispatch::hurt_dmg(&mut tw.g, &ore, lvl, ax, ay, 1);
        guard += 1;
        assert!(guard < 40, "ore tile never broke");
    }

    let pinged = tw.level(lvl).entities_to_add.iter().any(|e| {
        matches!(e.kind, EntityKind::Particle(_))
            && (e.c.x - bx * 16).abs() <= 4
            && (e.c.y - by * 16).abs() <= 4
    });
    assert!(pinged, "no sparkle at the hidden vein 2 tiles away");
}

/* ------------------------------ cave-ins + props ------------------------------ */

/// Breaking mine rock in a wide unpropped gallery arms a collapse (groan first,
/// rubble on the next tick); a Timber Prop within 3 tiles prevents it entirely.
#[test]
fn collapse_triggers_without_prop_not_with() {
    let mut tw = TestWorld::infinite().build();
    let mine = fdoom::level::lvl_idx(-1);
    fdoom::level::ensure_chunks_at(&mut tw.g, mine, 0, 0, true);

    let dirt = tw.tiles.get("dirt");
    let rock = tw.tiles.get("rock");
    let carve = |tw: &mut TestWorld| {
        for dy in -3..=3 {
            for dx in -3..=3 {
                let d = tw.tiles.get("dirt");
                tw.g.set_tile_default(mine, dx, dy, &d);
            }
        }
    };

    // no prop: the 1-in-4 roll must land well within 200 qualifying breaks
    carve(&mut tw);
    let mut fused = false;
    for _ in 0..200 {
        tw.g.set_tile_default(mine, 0, 0, &rock);
        dispatch::hurt_dmg(&mut tw.g, &rock, mine, 0, 0, 127);
        assert_eq!(tw.tile_at(mine, 0, 0).name, "DIRT");
        if tw.level(mine).get_data(0, 0) == fossick::COLLAPSE_FUSE {
            fused = true;
            break;
        }
    }
    assert!(fused, "collapse never armed in 200 unpropped breaks");
    assert!(
        tw.notifications.iter().any(|n| n.contains("groans")),
        "no ceiling-groan warning before the fall"
    );

    // the fuse fires on the tile's next tick: rubble rock falls nearby
    dispatch::tick(&mut tw.g, &dirt, mine, 0, 0);
    let mut rubble = Vec::new();
    for dy in -2..=2 {
        for dx in -2..=2 {
            if matches!(tw.tile_at(mine, dx, dy).kind, TileKind::Rock)
                && tw.level(mine).get_data(dx, dy) & fossick::RUBBLE_FLAG != 0
            {
                rubble.push((dx, dy));
            }
        }
    }
    assert!(!rubble.is_empty(), "armed collapse dropped no rubble");

    // rubble is weak and clearing it never re-arms a collapse
    let (rx, ry) = rubble[0];
    dispatch::hurt_dmg(&mut tw.g, &rock, mine, rx, ry, fossick::RUBBLE_HEALTH);
    assert_eq!(tw.tile_at(mine, rx, ry).name, "DIRT");
    assert_ne!(
        tw.level(mine).get_data(rx, ry),
        fossick::COLLAPSE_FUSE,
        "clearing rubble cascaded into another collapse"
    );

    // with a prop inside radius 3: the same 200 breaks never arm
    carve(&mut tw);
    let prop = tw.tiles.get("Timber Prop");
    tw.g.set_tile_default(mine, 2, 2, &prop);
    for _ in 0..200 {
        tw.g.set_tile_default(mine, 0, 0, &rock);
        dispatch::hurt_dmg(&mut tw.g, &rock, mine, 0, 0, 127);
        assert_ne!(
            tw.level(mine).get_data(0, 0),
            fossick::COLLAPSE_FUSE,
            "collapse armed despite a timber prop within 3 tiles"
        );
    }
}

/// Timber Prop round-trip: the tile item places on dirt, the tile is walk-through,
/// and one hit knocks it down, refunding wood and sticks.
#[test]
fn timber_prop_place_and_break_roundtrip() {
    let mut tw = TestWorld::infinite().build();
    let lvl = tw.current_level;
    let (tx, ty) = tw.place("dirt", 1, 0);

    // place through the real tile-item path
    let mut item = registry::get(&tw.g, "Timber Prop");
    let mut player = tw.g.entities.take(tw.g.player_id).expect("player");
    let placed = fdoom::item::interact::item_interact_on_tile(
        &mut tw.g,
        &mut item,
        lvl,
        tx,
        ty,
        &mut player,
        Direction::Down,
    );
    tw.g.entities.put_back(player);
    assert!(placed, "Timber Prop item refused to place on dirt");
    assert_eq!(tw.tile_at(lvl, tx, ty).name, "TIMBER PROP");

    // walk-through: you pass under the beams
    let def = tw.tile_at(lvl, tx, ty);
    let player = tw.g.entities.take(tw.g.player_id).expect("player");
    let passable = dispatch::may_pass(&tw.g, &def, lvl, tx, ty, &player);
    tw.g.entities.put_back(player);
    assert!(passable, "timber prop must not block the drive");

    // one hit knocks it down and refunds timber
    assert!(tw.hit(1, 0, 1));
    assert_eq!(tw.tile_at(lvl, tx, ty).name, "DIRT");
    let drops = tw.dropped_items();
    assert!(drops.iter().any(|d| d.eq_ignore_ascii_case("Wood")));
    assert!(drops.iter().any(|d| d.eq_ignore_ascii_case("Stick")));
}

/* ------------------------------- highland rock ------------------------------- */

/// Tier-2 summit rock takes double damage to break (100) and shatters into extra
/// stone (at least 3).
#[test]
fn highland_rock_takes_double_and_pays_extra() {
    let mut tw = TestWorld::infinite().build();
    let seed = tw.world_seed;

    // nearest highland position on a coarse outward sweep
    let mut found = None;
    'sweep: for ring in 0..400i32 {
        let r = ring * 8;
        for dy in (-r..=r).step_by(8) {
            for dx in (-r..=r).step_by(8) {
                if (dx.abs() == r || dy.abs() == r) && highland_at(seed, dx, dy) {
                    found = Some((dx, dy));
                    break 'sweep;
                }
            }
        }
    }
    let (hx, hy) = found.expect("no highland rock within 3200 tiles of the origin");

    tw.teleport(hx, hy + 1);
    tw.tick_n(8); // stream the chunks in
    let lvl = tw.current_level;
    tw.place_at("rock", hx, hy);

    let rock = tw.tiles.get("rock");
    dispatch::hurt_dmg(&mut tw.g, &rock, lvl, hx, hy, 99);
    assert_eq!(
        tw.tile_at(lvl, hx, hy).name,
        "ROCK",
        "highland rock broke below double damage"
    );
    dispatch::hurt_dmg(&mut tw.g, &rock, lvl, hx, hy, 1);
    assert_eq!(tw.tile_at(lvl, hx, hy).name, "DIRT");

    let stones = tw
        .dropped_items()
        .iter()
        .filter(|d| d.eq_ignore_ascii_case("Stone"))
        .count();
    assert!(stones >= 3, "highland break dropped only {stones} stone");
}
