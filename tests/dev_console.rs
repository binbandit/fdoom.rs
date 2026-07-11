//! The --debug dev console's command parser (`screen::dev_console::run_command`),
//! driven headlessly — the console UI is just typed-text capture over this.

use fdoom::core::updater::Time;
use fdoom::screen::dev_console::run_command;
use fdoom::testutil::TestWorld;

fn inv_count(tw: &TestWorld, name: &str) -> i32 {
    tw.g.player()
        .player()
        .inventory
        .items()
        .iter()
        .filter(|i| i.get_name().eq_ignore_ascii_case(name))
        .map(|i| i.count())
        .sum()
}

#[test]
fn give_stacks_and_confirms() {
    let mut tw = TestWorld::infinite().seed(42).debug().build();
    run_command(&mut tw.g, "give wood 5");
    assert_eq!(inv_count(&tw, "Wood"), 5);
    assert!(tw.g.notifications.last().unwrap().contains("Wood"));
}

#[test]
fn give_multiword_name_forgiving_case() {
    let mut tw = TestWorld::infinite().seed(42).debug().build();
    run_command(&mut tw.g, "GIVE cRuDe AxE 2");
    assert_eq!(inv_count(&tw, "Crude Axe"), 2);
}

#[test]
fn give_unknown_item_reports() {
    let mut tw = TestWorld::infinite().seed(42).debug().build();
    run_command(&mut tw.g, "give bogusite 3");
    assert_eq!(inv_count(&tw, "bogusite"), 0);
    assert!(tw.g.notifications.last().unwrap().contains("No such item"));
}

#[test]
fn tp_moves_to_tile_coords() {
    let mut tw = TestWorld::infinite().seed(42).debug().build();
    run_command(&mut tw.g, "tp 100 -42");
    assert_eq!(tw.player_tile(), (100, -42));

    run_command(&mut tw.g, "tp nowhere");
    assert!(tw.g.notifications.last().unwrap().contains("Usage"));
    assert_eq!(tw.player_tile(), (100, -42), "bad args must not move");
}

/// PLAYTEST suspected bug 5: "tp no-ops while swimming". Verify against the finished
/// console — a submerged player must still teleport (and keep moving after ticks).
#[test]
fn tp_works_while_swimming() {
    let mut tw = TestWorld::infinite().seed(42).debug().build();
    let (wx, wy) = tw.goto_biome(fdoom::level::infinite_gen::Biome::Ocean);
    let swimming = {
        let e = tw.g.entities.get(tw.g.player_id).unwrap();
        fdoom::entity::behavior::is_swimming(&tw.g, e)
    };
    assert!(
        swimming,
        "ocean teleport should leave the player swimming at ({wx}, {wy})"
    );

    run_command(&mut tw.g, "tp 7 9");
    tw.tick_n(3);
    let (tx, ty) = tw.player_tile();
    assert!(
        (tx - 7).abs() <= 1 && (ty - 9).abs() <= 1,
        "tp while swimming should move the player, got ({tx}, {ty})"
    );
}

#[test]
fn time_sets_time_of_day() {
    let mut tw = TestWorld::infinite().seed(42).debug().build();
    run_command(&mut tw.g, "time night");
    assert_eq!(tw.g.get_time(), Time::Night);
    run_command(&mut tw.g, "time noon");
    assert_eq!(tw.g.get_time(), Time::Day);
    run_command(&mut tw.g, "time dusk");
    assert_eq!(tw.g.get_time(), Time::Evening);
    run_command(&mut tw.g, "time whenever");
    assert!(tw.g.notifications.last().unwrap().contains("Usage"));
}

#[test]
fn heal_restores_stats() {
    let mut tw = TestWorld::infinite().seed(42).debug().build();
    {
        let pd = tw.g.player_mut().player_mut();
        pd.mob.health = 1;
        pd.hunger = 2;
        pd.stamina = 0;
    }
    run_command(&mut tw.g, "heal");
    let pd = tw.g.player().player();
    assert_eq!(pd.mob.health, pd.mob.max_health);
    assert_eq!(pd.hunger, fdoom::entity::mob::player::MAX_STAT);
    assert_eq!(pd.stamina, fdoom::entity::mob::player::MAX_STAT);
}

#[test]
fn unknown_command_reports() {
    let mut tw = TestWorld::infinite().seed(42).debug().build();
    run_command(&mut tw.g, "frobnicate the veeblefetzer");
    assert!(
        tw.g.notifications
            .last()
            .unwrap()
            .contains("Unknown command: frobnicate")
    );
}
