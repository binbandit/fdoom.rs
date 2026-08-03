//! Robustness regressions for the render/entity/screen layer: states that used to
//! panic (odd entity kinds behind a container display, a Continue entry whose world
//! vanished) must now recover with a WARN diagnostic and sane behavior.
//!
//! Each test was written against the panicking code first and observed to panic —
//! the exact messages are recorded in the test bodies.

use fdoom::core::log::{Level, capture};
use fdoom::screen::container_display::ContainerDisplay;
use fdoom::screen::title_display::TitleDisplay;
use fdoom::testutil::TestWorld;

/// A free eid that is not currently used by the arena.
fn free_eid(tw: &TestWorld, start: i32) -> i32 {
    let mut eid = start;
    while tw.g.entities.contains(eid) {
        eid += 1;
    }
    eid
}

/// Transferring an item while the "container" entity is furniture but not a
/// chest-family entity (possible via the public constructor, or an eid handed to a
/// non-chest after churn) used to panic in `ContainerDisplay::transfer`:
///
///   panicked at src/screen/container_display.rs:
///   container must be a chest
#[test]
fn container_transfer_with_non_chest_container_recovers() {
    let mut tw = TestWorld::infinite().build();
    tw.give("Wood", 5);
    let pid = tw.player_id;

    // A Bed is furniture (so the constructor accepts it) but has no chest layer.
    let mut bed = fdoom::entity::furniture::bed::new();
    bed.c.eid = free_eid(&tw, 900_001);
    bed.c.removed = false; // a live entity, as if placed on the level
    bed.c.level = Some(tw.current_level);
    let (px, py) = tw.player_pos();
    bed.c.x = px + 16;
    bed.c.y = py;
    let bed_eid = bed.c.eid;
    tw.g.entities.put_back(bed);

    let display = {
        let g = &tw.g;
        let player = g.entities.get(pid).expect("player is in the arena");
        let bed_ref = g.entities.get(bed_eid).expect("bed just inserted");
        ContainerDisplay::new(g, player, bed_ref)
    };
    // the empty (chest-less) container greets us on the pack side, where the
    // player's wood is selectable
    assert_eq!(display.focused_side(), 1, "pack side focused");
    tw.set_menu(display);
    tw.tick();

    let wood_before = count_item(&tw, "Wood");
    let (_, lines) = capture(Level::Warn, || {
        tw.press("ENTER"); // whole-stack transfer into the non-chest
    });

    assert!(
        lines.iter().any(|l| l.starts_with("WARN ")),
        "expected a WARN about the non-chest container, got {lines:?}"
    );
    // nothing moved, nothing was lost, and the entity is back in the arena
    assert_eq!(count_item(&tw, "Wood"), wood_before, "no items lost");
    assert!(tw.g.entities.contains(bed_eid), "container put back");
}

/// Opening a container display over an entity that is not furniture at all used to
/// panic in `ContainerDisplay::new`:
///
///   panicked at src/screen/container_display.rs:
///   container must be furniture
#[test]
fn container_display_over_non_furniture_entity_recovers() {
    let tw = TestWorld::infinite().build();
    let pid = tw.player_id;

    let ((), lines) = capture(Level::Warn, || {
        let g = &tw.g;
        let player = g.entities.get(pid).expect("player is in the arena");
        // the player itself is the handiest non-furniture entity
        let _display = ContainerDisplay::new(g, player, player);
    });
    assert!(
        lines.iter().any(|l| l.starts_with("WARN ")),
        "expected a WARN about the non-furniture container, got {lines:?}"
    );
}

/// Clicking Continue after the world it pointed at vanished from disk (deleted by
/// another program between menu build and click) used to panic in the title's
/// Continue closure:
///
///   panicked at src/screen/title_display.rs:
///   recent world existed when the title was built
#[test]
fn title_continue_after_world_vanishes_recovers() {
    let mut tw = TestWorld::infinite().build();

    // fabricate a loadable world folder so the title builds a Continue entry
    let saves = tw.g.game_dir.join("saves");
    let world_dir = saves.join("ghosttown");
    std::fs::create_dir_all(&world_dir).expect("create fake world dir");
    std::fs::write(world_dir.join("Game.miniplussave"), "2.1.0,survival").expect("write save");

    let title = TitleDisplay::new(&tw.g);
    // the world vanishes between menu build and click
    std::fs::remove_dir_all(&saves).expect("remove saves dir");

    tw.set_menu(title);
    tw.tick();
    let (_, lines) = capture(Level::Warn, || {
        tw.press("ENTER"); // Continue is the first entry
    });

    assert!(
        lines.iter().any(|l| l.starts_with("WARN ")),
        "expected a WARN about the vanished world, got {lines:?}"
    );
    // still on a menu (the rebuilt title), and no world load was started
    assert!(tw.g.display.menu_active(), "a menu is still open");
    assert!(
        tw.g.world_name.is_empty() || tw.g.world_name.starts_with("tw"),
        "no vanished world name was adopted: {:?}",
        tw.g.world_name
    );
}

/// The world-select screen can now carry a red status line for a failed
/// delete/copy/rename (those failures were stdout-only before — invisible, since
/// the notification tiers don't render on title-flow menus). Screenshot it so the
/// new line ships with a visual (house rule).
#[test]
fn world_select_error_line_screenshot() {
    let mut tw = TestWorld::infinite().build();
    tw.set_menu(fdoom::screen::world_select::WorldSelectDisplay::with_error(
        "Could not delete \"claim 1\"!",
    ));
    tw.tick();
    let path = tw.screenshot("world_select_error.png");
    assert!(path.exists(), "screenshot written to {path:?}");
}

/// World names run to 36 characters (the world-name input's cap), which makes the
/// error line ~56 characters — roughly 450px on a 288px screen. The line must clip
/// with an ellipsis rather than paint off the edge, so screenshot the worst case.
#[test]
fn world_select_error_line_clips_a_max_length_name() {
    let long_name = "w".repeat(36);
    let mut tw = TestWorld::infinite().build();
    tw.set_menu(fdoom::screen::world_select::WorldSelectDisplay::with_error(
        &format!("Could not delete \"{long_name}\"!"),
    ));
    tw.tick();
    let path = tw.screenshot("world_select_error_long.png");
    assert!(path.exists(), "screenshot written to {path:?}");
}

/// Count how many of `name` the player carries.
fn count_item(tw: &TestWorld, name: &str) -> i32 {
    let item = fdoom::item::registry::get(&tw.g, name);
    tw.g.try_player()
        .map(|p| p.player().inventory.count(&item))
        .unwrap_or(0)
}
