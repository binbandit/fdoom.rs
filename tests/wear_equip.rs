//! UI redesign lane L3 — real wear slots. ENTER on a wearable in PACK or on a
//! WEAR slot equips/unequips instantly (no world interaction); HEAD takes
//! hat-class gear, BODY the armors and the Fur Coat; nothing is ever lost on a
//! swap; temperature and damage absorption read the split slots correctly; the
//! slots survive save/load and old saves stay welcome.

use fdoom::core::temperature;
use fdoom::entity::Direction;
use fdoom::entity::mob::player::{WearSlot, wear_slot_for};
use fdoom::entity::mob::player_behavior;
use fdoom::item::registry;
use fdoom::saveload::load::Load;
use fdoom::saveload::save;
use fdoom::testutil::{TestWorld, bare_game};

fn body_name(tw: &TestWorld) -> Option<String> {
    tw.player()
        .player()
        .cur_armor
        .as_ref()
        .map(|a| a.get_name().to_string())
}

fn head_name(tw: &TestWorld) -> Option<String> {
    tw.player()
        .player()
        .worn_head
        .as_ref()
        .map(|a| a.get_name().to_string())
}

fn pack_count(tw: &TestWorld, name: &str) -> i32 {
    let item = registry::get(tw, name);
    tw.player().player().inventory.count(&item)
}

#[test]
fn pack_enter_wears_armor_instantly() {
    let mut tw = TestWorld::infinite().name("we_pack").build();
    tw.give("Leather Armor", 1);

    tw.press("E");
    tw.press("ENTER");

    assert!(
        tw.display.menu_active(),
        "equipping keeps the screen open (the result is visible on WEAR)"
    );
    assert_eq!(body_name(&tw).as_deref(), Some("Leather Armor"));
    assert_eq!(tw.player().player().armor, 30, "leather = 30 hits");
    assert_eq!(
        pack_count(&tw, "Leather Armor"),
        0,
        "the armor left the pack"
    );
    assert!(
        tw.player().player().active_item.is_none(),
        "instant equip never routes through the hands"
    );
}

#[test]
fn wear_tab_enter_equips_and_unequips_slots() {
    let mut tw = TestWorld::infinite().name("we_slots").build();
    tw.give("Straw Hat", 1);
    tw.give("Leather Armor", 1);

    tw.press("E");
    tw.press("RIGHT"); // PACK -> WEAR, cursor on HEAD

    tw.press("ENTER"); // wear the first fitting head item
    assert_eq!(head_name(&tw).as_deref(), Some("Straw Hat"));
    assert_eq!(pack_count(&tw, "Straw Hat"), 0);
    assert_eq!(
        tw.player().player().armor,
        0,
        "head gear has no hit meter — the armor meter stays down"
    );

    tw.press("DOWN"); // -> BODY
    tw.press("ENTER");
    assert_eq!(body_name(&tw).as_deref(), Some("Leather Armor"));
    assert_eq!(tw.player().player().armor, 30);

    tw.press("ENTER"); // ENTER on an occupied slot takes it off
    assert_eq!(body_name(&tw), None);
    assert_eq!(tw.player().player().armor, 0);
    assert_eq!(
        pack_count(&tw, "Leather Armor"),
        1,
        "unequipped gear returns to the pack"
    );

    tw.press("UP"); // -> HEAD
    tw.press("Q"); // Q takes off too
    assert_eq!(head_name(&tw), None);
    assert_eq!(pack_count(&tw, "Straw Hat"), 1);
}

#[test]
fn wear_tab_stows_the_held_item() {
    let mut tw = TestWorld::infinite().name("we_held").build();
    tw.give("Torch", 8);

    tw.press("E");
    tw.press("ENTER"); // hold the torches (closes the screen)
    assert_eq!(
        tw.player()
            .player()
            .active_item
            .as_ref()
            .map(|i| i.get_name().to_string())
            .as_deref(),
        Some("Torch")
    );

    tw.press("E");
    tw.press("RIGHT"); // WEAR
    tw.press("DOWN");
    tw.press("DOWN"); // -> HELD
    tw.press("ENTER"); // stow the held stack back into the pack
    assert!(tw.player().player().active_item.is_none());
    assert_eq!(pack_count(&tw, "Torch"), 8, "the whole stack came back");
}

#[test]
fn head_body_class_rules() {
    let tw = TestWorld::infinite().name("we_class").build();
    for (name, want) in [
        ("Straw Hat", Some(WearSlot::Head)),
        ("Fur Coat", Some(WearSlot::Body)),
        ("Leather Armor", Some(WearSlot::Body)),
        ("Iron Armor", Some(WearSlot::Body)),
        ("Gem Armor", Some(WearSlot::Body)),
        ("Red Clothes", None), // clothing is a dye, not a slot
        ("Torch", None),
    ] {
        let item = registry::get(&tw, name);
        assert_eq!(wear_slot_for(&item), want, "slot class of {name}");
    }

    // ENTER on an empty HEAD with only body armor in the pack does nothing
    let mut tw = TestWorld::infinite().name("we_class2").build();
    tw.give("Leather Armor", 1);
    tw.press("E");
    tw.press("RIGHT"); // WEAR, cursor on HEAD
    tw.press("ENTER");
    assert_eq!(head_name(&tw), None, "body armor must not land on HEAD");
    assert_eq!(pack_count(&tw, "Leather Armor"), 1);

    // and a hat equipped from the pack lands on HEAD, never BODY
    tw.press("ESCAPE");
    tw.give("Straw Hat", 1);
    tw.press("E"); // reopen: the pack lists leather then the hat
    tw.press("DOWN"); // leather -> hat
    tw.press("ENTER");
    assert_eq!(head_name(&tw).as_deref(), Some("Straw Hat"));
    assert_eq!(body_name(&tw), None);
}

#[test]
fn swapping_gear_loses_nothing_and_splits_stacks() {
    let mut tw = TestWorld::infinite().name("we_swap").build();
    tw.give("Leather Armor", 2);

    // wearing from a stack takes one and leaves the rest in the pack
    tw.press("E");
    tw.press("ENTER");
    assert_eq!(body_name(&tw).as_deref(), Some("Leather Armor"));
    assert_eq!(
        pack_count(&tw, "Leather Armor"),
        1,
        "stack split, not swallowed"
    );

    // swapping to iron returns the worn leather to the pack
    tw.give("Iron Armor", 1);
    tw.press("ESCAPE");
    tw.press("E");
    tw.press("DOWN"); // leather -> iron
    tw.press("ENTER");
    assert_eq!(body_name(&tw).as_deref(), Some("Iron Armor"));
    assert_eq!(tw.player().player().armor, 50, "iron = 50 hits");
    assert_eq!(
        pack_count(&tw, "Leather Armor"),
        2,
        "displaced leather rejoins its stack — nothing lost"
    );
    assert_eq!(pack_count(&tw, "Iron Armor"), 0);
}

#[test]
fn legacy_use_to_wear_still_works_and_answers() {
    let mut tw = TestWorld::infinite().name("we_legacy").build();
    let armor = registry::get(&tw, "Leather Armor");
    let hat = registry::get(&tw, "Straw Hat");
    let pid = tw.player_id;

    // classic ritual: hold the armor, press attack — no facing-tile luck involved
    tw.player_mut().player_mut().active_item = Some(armor);
    let mut p = tw.entities.take(pid).unwrap();
    player_behavior::attack(&mut tw, &mut p);
    tw.entities.put_back(p);

    assert_eq!(body_name(&tw).as_deref(), Some("Leather Armor"));
    assert!(
        tw.notifications.iter().any(|n| n.contains("Worn")),
        "wearing must announce itself: {:?}",
        tw.notifications
    );

    // the ritual keeps its stamina toll — and now says so when you can't pay it
    // (the toll clamps, so only an empty tank blocks — classic pay_stamina rules)
    assert_eq!(tw.player().player().stamina, 1, "the toll was paid");
    tw.player_mut().player_mut().stamina = 0;
    tw.player_mut().player_mut().active_item = Some(hat);
    let mut p = tw.entities.take(pid).unwrap();
    player_behavior::attack(&mut tw, &mut p);
    tw.entities.put_back(p);
    assert_eq!(head_name(&tw), None, "too tired to put the hat on");
    assert!(
        tw.notifications.iter().any(|n| n.contains("Too tired")),
        "the blocked path must answer too: {:?}",
        tw.notifications
    );
}

#[test]
fn temperature_reads_the_split_slots_and_gear_stacks() {
    let mut tw = TestWorld::infinite().name("we_temp").build();
    tw.give("Straw Hat", 1);
    tw.give("Fur Coat", 1);

    // equip both through the pack — hat to HEAD, coat to BODY, simultaneously
    tw.press("E");
    tw.press("ENTER");
    tw.press("ENTER");
    assert_eq!(head_name(&tw).as_deref(), Some("Straw Hat"));
    assert_eq!(body_name(&tw).as_deref(), Some("Fur Coat"));

    let m = temperature::modifiers_for(&tw, tw.player());
    assert!(m.straw_hat, "temperature must see the hat on the HEAD slot");
    assert!(m.fur_coat, "temperature must see the coat on the BODY slot");

    // the shifts stack now that hat + coat no longer fight over one slot
    let stacked = temperature::Modifiers {
        straw_hat: true,
        fur_coat: true,
        ..Default::default()
    };
    assert_eq!(
        temperature::apply_modifiers(-3.0, &stacked),
        -1.0,
        "coat pulls two cold bands"
    );
    assert_eq!(
        temperature::apply_modifiers(2.0, &stacked),
        1.0,
        "hat pulls one heat band"
    );
}

#[test]
fn damage_absorption_reads_the_body_slot_only() {
    // body armor absorbs
    let mut tw = TestWorld::infinite().name("we_absorb").build();
    tw.give("Leather Armor", 1);
    tw.press("E");
    tw.press("ENTER");
    tw.press("ESCAPE");
    let pid = tw.player_id;
    let mut p = tw.entities.take(pid).unwrap();
    player_behavior::do_hurt(&mut tw, &mut p, 4, Direction::Down);
    tw.entities.put_back(p);
    assert_eq!(
        tw.player().player().armor,
        26,
        "the hit drains the armor meter"
    );
    assert_eq!(
        tw.player().player().mob.health,
        8,
        "leather (level 1) leaks half the damage through"
    );

    // a hat alone absorbs nothing — HEAD carries no hit meter
    let mut tw = TestWorld::infinite().name("we_absorb2").build();
    tw.give("Straw Hat", 1);
    tw.press("E");
    tw.press("ENTER");
    tw.press("ESCAPE");
    assert_eq!(head_name(&tw).as_deref(), Some("Straw Hat"));
    let pid = tw.player_id;
    let mut p = tw.entities.take(pid).unwrap();
    player_behavior::do_hurt(&mut tw, &mut p, 2, Direction::Down);
    tw.entities.put_back(p);
    assert_eq!(tw.player().player().mob.health, 8, "full damage lands");
    assert_eq!(tw.player().player().armor, 0);
}

#[test]
fn save_roundtrip_carries_both_slots() {
    let mut g1 = bare_game("wear_save");
    let hat = registry::get(&g1, "Straw Hat");
    let iron = registry::get(&g1, "Iron Armor");
    let mut p = g1.entities.take(0).unwrap();
    {
        let pd = p.player_mut();
        pd.worn_head = Some(hat);
        pd.cur_armor = Some(iron);
        pd.armor = 50;
        pd.armor_damage_buffer = 3;
    }
    let mut data = Vec::new();
    save::write_player(&g1, &p, &mut data);
    g1.entities.put_back(p);

    // the HEAD slot rides a tagged trailing entry (tolerant-marker scheme)
    assert_eq!(data.last().map(String::as_str), Some("WornHead:Straw Hat"));

    let mut g2 = bare_game("wear_load");
    let loader = Load::with_version(&g2, fdoom::core::game::version());
    loader.load_player(&mut g2, &data);
    let pd = g2.player().player();
    assert_eq!(
        pd.worn_head.as_ref().map(|a| a.get_name()),
        Some("Straw Hat")
    );
    assert_eq!(
        pd.cur_armor.as_ref().map(|a| a.get_name()),
        Some("Iron Armor")
    );
    assert_eq!(pd.armor, 50);
    assert_eq!(pd.armor_damage_buffer, 3);
}

#[test]
fn old_saves_load_with_armor_on_body_and_hats_migrated() {
    // classic save, no WornHead entry: worn armor lands on BODY, HEAD stays empty
    let legacy: Vec<String> = [
        "264",
        "152",
        "16",
        "9",
        "7",
        "5",
        "50",
        "3",
        "Iron Armor",
        "1234",
        "0",
        "PotionEffects[]",
        "520",
        "true",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let mut g = bare_game("wear_old");
    let loader = Load::with_version(&g, fdoom::core::game::version());
    loader.load_player(&mut g, &legacy);
    let pd = g.player().player();
    assert_eq!(
        pd.cur_armor.as_ref().map(|a| a.get_name()),
        Some("Iron Armor")
    );
    assert!(pd.worn_head.is_none());
    assert_eq!(pd.armor, 50);

    // a hat worn on the old single slot migrates to HEAD (its token hit meter
    // retires with the move)
    let legacy_hat: Vec<String> = [
        "264",
        "152",
        "16",
        "9",
        "7",
        "5",
        "10",
        "0",
        "Straw Hat",
        "1234",
        "0",
        "PotionEffects[]",
        "520",
        "true",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let mut g = bare_game("wear_old_hat");
    let loader = Load::with_version(&g, fdoom::core::game::version());
    loader.load_player(&mut g, &legacy_hat);
    let pd = g.player().player();
    assert_eq!(
        pd.worn_head.as_ref().map(|a| a.get_name()),
        Some("Straw Hat")
    );
    assert!(pd.cur_armor.is_none(), "the hat left the armor slot");
    assert_eq!(pd.armor, 0, "no hit meter for head gear");
}

#[test]
fn wear_pane_screenshots() {
    // slots empty
    let mut tw = TestWorld::infinite().name("we_shot_empty").build();
    tw.press("E");
    tw.press("RIGHT");
    tw.screenshot("wear_slots_empty.png");

    // hat + coat equipped via the slots
    let mut tw = TestWorld::infinite().name("we_shot_worn").build();
    tw.give("Straw Hat", 1);
    tw.give("Fur Coat", 1);
    tw.press("E");
    tw.press("ENTER");
    tw.press("ENTER");
    tw.press("RIGHT");
    assert_eq!(head_name(&tw).as_deref(), Some("Straw Hat"));
    assert_eq!(body_name(&tw).as_deref(), Some("Fur Coat"));
    tw.screenshot("wear_hat_coat.png");

    // mid-swap: leather worn, alternatives waiting in the pack, BODY selected
    let mut tw = TestWorld::infinite().name("we_shot_swap").build();
    tw.give("Leather Armor", 1);
    tw.press("E");
    tw.press("ENTER"); // wear the leather
    tw.give("Iron Armor", 1);
    tw.give("Fur Coat", 1);
    tw.give("Red Clothes", 1);
    tw.press("RIGHT"); // WEAR
    tw.press("DOWN"); // -> BODY
    assert_eq!(body_name(&tw).as_deref(), Some("Leather Armor"));
    tw.screenshot("wear_mid_swap.png");
}
