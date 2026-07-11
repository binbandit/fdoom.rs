use fdoom::level::tile::fossick::{COLLAPSE_FUSE, COLLAPSE_OPEN_MIN, CORRIDOR_PROP_RADIUS};
use fdoom::level::tile::{TileKind, dispatch};
use fdoom::rng::Rng;
use fdoom::testutil::TestWorld;

const CORRIDOR_LEN: i32 = 40;
const CORRIDOR_TRIALS: i64 = 64;

fn corridor_seeds() -> impl Iterator<Item = i64> {
    10_000..10_000 + CORRIDOR_TRIALS
}

fn stage_corridor(tw: &mut TestWorld, mine: usize, propped: bool) {
    fdoom::level::ensure_chunks_at(&mut tw.g, mine, 0, 0, true);
    fdoom::level::ensure_chunks_at(&mut tw.g, mine, CORRIDOR_LEN - 1, 0, true);

    let rock = tw.tiles.get("rock");
    for y in -CORRIDOR_PROP_RADIUS..=CORRIDOR_PROP_RADIUS {
        for x in -(CORRIDOR_PROP_RADIUS + 2)..=CORRIDOR_LEN + CORRIDOR_PROP_RADIUS + 2 {
            tw.g.set_tile_default(mine, x, y, &rock);
        }
    }

    if propped {
        let prop = tw.tiles.get("Timber Prop");
        let mut x = 0;
        while x < CORRIDOR_LEN {
            tw.g.set_tile_default(mine, x, CORRIDOR_PROP_RADIUS, &prop);
            x += CORRIDOR_PROP_RADIUS * 2;
        }
    }
}

fn open_for_collapse(kind: &TileKind) -> bool {
    !matches!(
        kind,
        TileKind::Rock | TileKind::Ore { .. } | TileKind::HardRock | TileKind::Wall { .. }
    )
}

fn open_count_5x5(tw: &TestWorld, mine: usize, x: i32, y: i32) -> i32 {
    let mut open = 0;
    for dy in -2..=2 {
        for dx in -2..=2 {
            if open_for_collapse(&tw.tile_at(mine, x + dx, y + dy).kind) {
                open += 1;
            }
        }
    }
    open
}

fn dig_corridor(seed: i64, propped: bool) -> bool {
    let mut tw = TestWorld::infinite().seed(seed).build();
    let mine = fdoom::level::lvl_idx(-1);
    stage_corridor(&mut tw, mine, propped);
    tw.g.random = Rng::new(seed ^ 0xC011_A95E);
    tw.g.level_mut(mine).random = Rng::new(seed ^ 0x1EAF_5EED);

    let rock = tw.tiles.get("rock");
    for x in 0..CORRIDOR_LEN {
        dispatch::hurt_dmg(&mut tw.g, &rock, mine, x, 0, 127);
        assert_eq!(tw.tile_at(mine, x, 0).name, "DIRT");
        assert!(
            open_count_5x5(&tw, mine, x, 0) < COLLAPSE_OPEN_MIN,
            "corridor test accidentally qualified the open-gallery path at x={x}"
        );
        if tw.level(mine).get_data(x, 0) == COLLAPSE_FUSE {
            assert!(
                tw.notifications.iter().any(|n| n.contains("groans")),
                "armed corridor collapse without the groan notification"
            );
            return true;
        }
    }
    false
}

#[test]
fn unpropped_corridor_caveins_are_reachable_but_not_saturated() {
    let mut armed = 0usize;
    let mut armed_seeds = Vec::new();
    for seed in corridor_seeds() {
        if dig_corridor(seed, false) {
            armed += 1;
            armed_seeds.push(seed);
        }
    }

    let rate = armed as f64 / CORRIDOR_TRIALS as f64;
    println!(
        "unpropped corridor collapse arm rate: {armed}/{CORRIDOR_TRIALS} ({:.1}%), seeds {armed_seeds:?}",
        rate * 100.0
    );
    assert!(armed > 0, "no unpropped corridor collapse armed");
    assert!(
        rate > 0.10 && rate < 0.70,
        "corridor arm rate should be reachable but not saturated: {armed}/{CORRIDOR_TRIALS}"
    );
}

#[test]
fn fully_propped_corridors_never_arm() {
    let mut armed_seeds = Vec::new();
    for seed in corridor_seeds() {
        if dig_corridor(seed, true) {
            armed_seeds.push(seed);
        }
    }

    println!(
        "fully propped corridor collapse arm rate: {}/{CORRIDOR_TRIALS}",
        armed_seeds.len()
    );
    assert!(
        armed_seeds.is_empty(),
        "propped corridors armed collapses for seeds {armed_seeds:?}"
    );
}

#[test]
fn every_successful_pan_reports_a_hit_or_miss() {
    let mut tw = TestWorld::infinite().seed(77_777).build();
    tw.g.random = Rng::new(0x51A7E);
    tw.place("mud", 1, 0);

    let mut saw_miss = false;
    let mut saw_find = false;
    let mut successful_pans = 0usize;
    for _ in 0..160 {
        tw.g.player_mut().player_mut().stamina = 10;
        let notes_before = tw.notifications.len();
        let drops_before = tw.dropped_items().len();

        assert!(tw.interact_with("Prospector's Pan", 1, 0), "mud must pan");
        successful_pans += 1;
        assert_eq!(
            tw.notifications.len(),
            notes_before + 1,
            "each worked pan should push exactly one notification"
        );

        let note = tw.notifications.last().expect("pan notification");
        if note == "Nothing but gray sand." {
            saw_miss = true;
        }
        if tw.dropped_items().len() > drops_before {
            saw_find = true;
            assert_ne!(
                note, "Nothing but gray sand.",
                "a paying pan should use a find notification"
            );
        }

        if saw_miss && saw_find {
            break;
        }
    }

    assert!(
        saw_miss,
        "no miss notification after {successful_pans} successful pans"
    );
    assert!(
        saw_find,
        "no paying-find notification after {successful_pans} successful pans"
    );
}
