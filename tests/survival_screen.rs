//! The survival screen (UI redesign lane L2): E opens the five-tab shell, tabs
//! navigate with wrap, ESC closes from anywhere, PACK categorizes with a detail
//! card and working hold/drop actions, SELF reads temperature bands and effects.
//! (NOTES, the fifth tab, is covered in tests/hunting_notes.rs.)

use fdoom::core::temperature;
use fdoom::core::updater::NORM_SPEED;
use fdoom::item::{PotionType, registry};
use fdoom::level::infinite_gen::Biome;
use fdoom::screen::survival_display::{self, SurvivalDisplay};
use fdoom::testutil::TestWorld;

const W: i32 = 288;
/// The active tab's gold underline row (survival_display::UNDERLINE_Y).
const UNDERLINE_Y: i32 = 22;
const GOLD: i32 = 0xE0C84A;

/// Leftmost x of the active tab's underline — identifies both "the survival screen
/// is on screen" and which tab is active.
fn underline_x(frame: &[i32]) -> Option<i32> {
    (0..W).find(|x| frame[(UNDERLINE_Y * W + x) as usize] == GOLD)
}

#[test]
fn e_opens_the_survival_screen_and_e_or_esc_closes_it() {
    let mut tw = TestWorld::infinite().name("ss_open").build();

    tw.press("E");
    assert!(
        tw.display.menu_active(),
        "E should open the survival screen"
    );
    let frame = tw.render();
    assert!(
        underline_x(&frame).is_some(),
        "the tab strip's active underline should render (this is the new screen, \
         not the old inventory panel)"
    );

    tw.press("E");
    assert!(!tw.display.menu_active(), "E should also close it");

    tw.press("E");
    tw.press("ESCAPE");
    assert!(!tw.display.menu_active(), "ESC should close it");
}

#[test]
fn tabs_cycle_left_right_with_wrap() {
    let mut tw = TestWorld::infinite().name("ss_tabs").build();
    tw.press("E");

    let mut xs = Vec::new();
    for _ in 0..5 {
        let frame = tw.render();
        xs.push(underline_x(&frame).expect("active tab underline"));
        tw.press("RIGHT");
    }
    let back = underline_x(&tw.render()).expect("active tab underline");

    assert_eq!(xs.len(), 5);
    for pair in xs.windows(2) {
        assert!(
            pair[1] > pair[0],
            "each RIGHT should move the underline right: {xs:?}"
        );
    }
    assert_eq!(back, xs[0], "five RIGHTs should wrap back to PACK");

    // LEFT from PACK wraps to NOTES (the last tab)
    tw.press("LEFT");
    let left_wrap = underline_x(&tw.render()).expect("active tab underline");
    assert_eq!(left_wrap, xs[4], "LEFT from PACK should wrap to NOTES");
}

#[test]
fn esc_closes_from_every_tab() {
    let mut tw = TestWorld::infinite().name("ss_esc").build();
    for i in 0..5 {
        tw.press("E");
        for _ in 0..i {
            tw.press("RIGHT");
        }
        assert!(tw.display.menu_active(), "screen open on tab {i}");
        tw.press("ESCAPE");
        assert!(!tw.display.menu_active(), "ESC should close from tab {i}");
    }
}

#[test]
fn pack_categorizes_items_without_baked_in_counts() {
    let mut tw = TestWorld::infinite().name("ss_pack").build();
    tw.give("Crude Axe", 1);
    tw.give("Plank", 30);
    tw.give("Cord", 3);
    tw.give("Sharp Stone", 2);
    tw.give("Torch", 8);
    tw.give("Apple", 2);
    tw.give("Leather Armor", 1);

    let pid = tw.player_id;
    let player = tw.entities.take(pid).unwrap();
    let display = SurvivalDisplay::new(&tw, &player);
    tw.entities.put_back(player);

    let labels = display.pack_row_labels(&tw);
    let pos = |s: &str| {
        labels
            .iter()
            .position(|l| l.eq_ignore_ascii_case(s))
            .unwrap_or_else(|| panic!("{s} missing from pack rows: {labels:?}"))
    };

    // headers appear in the doc's order, each before its items
    assert!(pos("TOOLS") < pos("Crude Axe"));
    assert!(pos("MATERIALS") < pos("Plank"));
    assert!(pos("Plank") < pos("FOOD"));
    assert!(pos("FOOD") < pos("Apple"));
    assert!(pos("GEAR") < pos("Leather Armor"));

    // counts live in their own column, not in the row label (J7)
    assert!(
        !labels.iter().any(|l| l.contains("30")),
        "counts must not be baked into names: {labels:?}"
    );

    // eyeball frame: mixed pack with the detail card
    tw.set_menu(display);
    tw.tick();
    tw.screenshot("survival_pack.png");
}

#[test]
fn pack_hold_action_swaps_without_item_loss() {
    let mut tw = TestWorld::infinite().name("ss_hold").build();
    tw.give("Torch", 8);
    tw.give("Apple", 2);

    // first item row is the torch stack (MATERIALS sorts before FOOD)
    tw.press("E");
    tw.press("ENTER");
    assert!(
        !tw.display.menu_active(),
        "holding an item closes the screen"
    );
    let held = tw.player().player().active_item.clone().expect("held item");
    assert_eq!(held.get_name(), "Torch", "ENTER should hold the torches");

    // holding the apples stashes the torches back into the pack (nothing is lost)
    tw.press("E");
    tw.press("ENTER");
    let held = tw.player().player().active_item.clone().expect("held item");
    assert_eq!(held.get_name(), "Apple");
    let torch = registry::get(&tw, "Torch");
    assert_eq!(
        tw.player().player().inventory.count(&torch),
        8,
        "the previously held stack must return to the pack"
    );
}

#[test]
fn pack_drops_one_then_the_stack() {
    let mut tw = TestWorld::infinite().name("ss_drop").build();
    tw.give("Plank", 30);

    tw.press("E");
    tw.press("Q");
    let plank = registry::get(&tw, "Plank");
    assert_eq!(
        tw.player().player().inventory.count(&plank),
        29,
        "Q drops a single plank"
    );

    tw.hold("SHIFT");
    tw.press("Q");
    tw.release("SHIFT");
    assert_eq!(
        tw.player().player().inventory.count(&plank),
        0,
        "SHIFT-Q drops the remaining stack"
    );
    let dropped = tw.dropped_items();
    assert!(
        dropped.iter().filter(|n| n.contains("Plank")).count() >= 2,
        "both drops should land on the level: {dropped:?}"
    );
}

#[test]
fn z_jumps_to_craft_and_enter_crafts_cord() {
    let mut tw = TestWorld::infinite().name("ss_craft").build();
    tw.give("Grass Fibers", 3);

    tw.press("Z");
    assert!(
        tw.display.menu_active(),
        "Z should open the survival screen"
    );
    let frame = tw.render();
    let craft_x = underline_x(&frame).expect("active tab underline");

    // CRAFT is the third of four tab slots — its underline sits right of center
    assert!(
        craft_x > W / 2 - 40,
        "Z should land on the CRAFT tab (underline at x={craft_x})"
    );
    tw.screenshot("survival_craft.png");

    // 3 fibers make Cord the only affordable recipe; the list sorts it first
    tw.press("ENTER");
    let cord = registry::get(&tw, "Cord");
    assert!(
        tw.player().player().inventory.count(&cord) >= 1,
        "ENTER on the CRAFT tab should craft the cord"
    );
}

#[test]
fn self_pane_shows_temperature_band_and_active_effects() {
    let mut tw = TestWorld::infinite().name("ss_self").build();
    tw.goto_biome(Biome::Tundra);
    tw.player_mut()
        .player_mut()
        .potioneffects
        .insert(PotionType::Swim, 84 * NORM_SPEED);

    // P (the freed effects-overlay key) opens the screen straight onto SELF
    tw.press("P");
    assert!(
        tw.display.menu_active(),
        "P should open the survival screen"
    );

    let lines = survival_display::effect_lines(tw.player().player());
    assert_eq!(lines.len(), 1, "one active effect: {lines:?}");
    assert!(
        lines[0].starts_with("SWIM 1:2"),
        "effect row should show the name and an m:ss timer: {lines:?}"
    );

    let steps = temperature::band_for(&tw, tw.player()).steps();
    assert!(
        steps <= -1,
        "tundra should read on the cold side, got {steps}"
    );

    let frame = tw.render();
    // warmth gauge geometry: cells at y=100..106 from x=24, 14px apart; the
    // freezing cell always renders in its band color
    assert_eq!(
        frame[(102 * W + 26) as usize],
        0x2B4FF0,
        "the freezing cell of the warmth gauge should render"
    );
    // the marker sits under the current band's cell
    let marker_x = 24 + (steps + 3) * 14 + 4;
    assert_eq!(
        frame[(107 * W + marker_x) as usize],
        0xFFFFFF,
        "the band marker should sit under the current band (steps {steps})"
    );

    tw.screenshot("survival_self_cold.png");
}

#[test]
fn wear_tab_renders_the_slot_list() {
    // the WEAR pane is real equip slots since L3; the deep coverage lives in
    // tests/wear_equip.rs — this pins the tab into the shell and the eyeball frame
    let mut tw = TestWorld::infinite().name("ss_wear").build();
    tw.give("Leather Armor", 1);
    tw.press("E");
    tw.press("ENTER"); // instant equip from the pack
    tw.press("RIGHT"); // PACK -> WEAR
    assert!(tw.display.menu_active());
    assert_eq!(
        tw.player()
            .player()
            .cur_armor
            .as_ref()
            .map(|a| a.get_name()),
        Some("Leather Armor")
    );
    tw.screenshot("survival_wear.png");
}

#[test]
fn empty_pack_keeps_the_onboarding_hint() {
    let mut tw = TestWorld::infinite().name("ss_empty").build();
    tw.press("E");
    assert!(tw.display.menu_active());
    tw.screenshot("survival_pack_empty.png");
    // the hint renders in the dim tier; presence is eyeballed via the screenshot,
    // navigation/hold/drop on an empty pack must simply not panic
    tw.press("DOWN");
    tw.press("ENTER");
    assert!(
        tw.display.menu_active(),
        "ENTER on an empty pack is a no-op"
    );
    tw.press("Q");
    assert!(tw.display.menu_active());
}

/// Regression: crafting consumes inventory items while the PACK row list is
/// stale; ENTER on a late row then indexed out of bounds (user crash report:
/// "index out of bounds: the len is 10 but the index is 10" holding an axe).
#[test]
fn pack_survives_crafting_shrinking_the_inventory() {
    let mut tw = TestWorld::infinite().name("ss_stale").build();
    // three exactly-consumable single stacks + 8 non-stackable axes = 11 slots;
    // crafting the Crude Axe eats all three stacks and adds one item (net -2)
    tw.give("Stick", 1);
    tw.give("Cord", 1);
    tw.give("Sharp Stone", 1);
    for _ in 0..8 {
        tw.give("Crude Axe", 1);
    }

    tw.press("E");
    assert!(tw.display.menu_active());
    // wrap-at-ends onto the LAST row: a material at inventory index 10 (give
    // fills tools first, so stick/cord/sharp stone land at inv 8..=10)
    tw.press("UP");
    tw.press("Z"); // jump to CRAFT — the list sorts affordable first, so the
    // Crude Axe (stick + cord + sharp stone, all in the pack) is the selection
    tw.press("ENTER"); // craft it: three stacks consumed, one item added, 11 -> 9
    tw.press("LEFT");
    tw.press("LEFT"); // CRAFT -> WEAR -> PACK
    // ENTER on the remembered last row must not index out of bounds
    tw.press("ENTER");
}

/// Regression (second live crash report): render/tick must survive ANY inventory
/// mutation the rows didn't see — here the last slot vanishes out from under the
/// row list, then both render and ENTER run on the stale selection.
#[test]
fn pack_self_heals_when_the_inventory_shrinks_externally() {
    let mut tw = TestWorld::infinite().name("ss_selfheal").build();
    for _ in 0..10 {
        tw.give("Crude Axe", 1);
    }
    tw.press("E");
    assert!(tw.display.menu_active());
    tw.press("UP"); // wrap to the last row (inventory index 9)

    // yank the last item out from under the row list (no display code runs)
    if let Some(p) = tw.g.entities.get_mut(tw.g.player_id) {
        let inv = &mut p.player_mut().inventory;
        let last = inv.inv_size() - 1;
        inv.remove(last);
    }

    // a frame renders with the stale rows (this alone panicked at len=idx), then
    // ENTER acts on the remembered selection
    let _ = tw.render();
    tw.press("ENTER");
}
