//! Field-notes recipe variants: found journals each teach one recipe VARIANT —
//! a cheaper stitch, a longer burn — never a family unlock, never gating
//! progression (UI_REDESIGN §4 "the runner-up"). Reading learns once (keepsake
//! book, idempotent), learned variants append after their originals on the craft
//! lists, the state rides the tolerant `Variants:v1:` save marker, and the
//! journals seed as modest scavenge finds.

use fdoom::core::field_notes::{
    ALL_VARIANTS, RecipeVariant, VARIANT_COUNT, append_learned_variants, variants_learned_count,
};
use fdoom::core::game::Game;
use fdoom::entity::furniture::scav_container::ScavKind;
use fdoom::entity::{Direction, mob};
use fdoom::item::recipe::Recipes;
use fdoom::item::{interact, registry};
use fdoom::level::structures_gen::{
    StructureKind, TownAge, fill_scav_container, fill_structure_chest,
};
use fdoom::saveload::{load, save};
use fdoom::screen::survival_display::{self, SurvivalDisplay, Tab};
use fdoom::testutil::{TestWorld, bare_game, find_recipe};

/// Use a journal item like the player does (item interact on the tile ahead).
fn read_book(tw: &mut TestWorld, name: &str) {
    let mut item = registry::get(&tw.g, name);
    let (px, py) = tw.player_tile();
    let lvl = tw.g.current_level;
    let mut player = tw.g.entities.take(tw.g.player_id).expect("player");
    interact::item_interact_on_tile(
        &mut tw.g,
        &mut item,
        lvl,
        px,
        py + 1,
        &mut player,
        Direction::Down,
    );
    tw.g.entities.put_back(player);
}

/// How many times the two-line learn toast fired for `title` (the header line
/// and the title line ride the ticker together; count the title line).
fn toast_count(g: &Game, title: &str) -> usize {
    let msg = format!("{title}.");
    g.notifications.iter().filter(|n| **n == msg).count()
}

fn learned(tw: &TestWorld) -> u8 {
    tw.g.entities
        .get(tw.g.player_id)
        .expect("player")
        .player()
        .variants_learned
}

#[test]
fn reading_learns_exactly_once_and_rereads_are_silent() {
    let mut tw = TestWorld::infinite().seed(0x11).build();
    assert_eq!(learned(&tw), 0, "a fresh player knows no variants");

    read_book(&mut tw, "Tanner's Notes");
    assert_eq!(learned(&tw), RecipeVariant::TannersStitch.bit());
    assert_eq!(
        toast_count(&tw.g, "TANNER'S STITCH"),
        1,
        "one toast on learn"
    );
    tw.tick_n(1); // the menu stack applies on the next tick
    assert!(
        tw.g.display.menu_active(),
        "the book still opens for reading"
    );

    // re-read: the book is a keepsake — no new bit, no second toast
    tw.g.exit_menu();
    read_book(&mut tw, "Tanner's Notes");
    assert_eq!(learned(&tw), RecipeVariant::TannersStitch.bit());
    assert_eq!(
        toast_count(&tw.g, "TANNER'S STITCH"),
        1,
        "re-read must be silent"
    );

    // the other journals learn their own bits, and a plain book teaches nothing
    tw.g.exit_menu();
    read_book(&mut tw, "Fletcher's Diary");
    tw.g.exit_menu();
    read_book(&mut tw, "Book");
    assert_eq!(
        learned(&tw),
        RecipeVariant::TannersStitch.bit() | RecipeVariant::FletchersFeathering.bit()
    );
    assert_eq!(toast_count(&tw.g, "FLETCHER'S FEATHERING"), 1);
}

#[test]
fn variants_append_after_originals_and_never_replace_them() {
    let base = Recipes::new();
    let all = ALL_VARIANTS.iter().fold(0u8, |m, v| m | v.bit());

    // approachability audit: every base recipe still exists at its original cost
    let originals = [
        ("LEATHER", &base.craft, vec![("HIDE", 2), ("CORD", 1)], 2),
        ("TORCH", &base.craft, vec![("WOOD", 1), ("COAL", 1)], 2),
        ("ARROW", &base.workbench, vec![("WOOD", 2), ("STONE", 2)], 3),
        ("FUR COAT", &base.craft, vec![("FUR", 5), ("CORD", 2)], 1),
    ];
    for (product, list, costs, amount) in originals {
        let r = find_recipe(list, product);
        assert!(
            !r.is_from_field_notes(),
            "{product} original must stay untagged"
        );
        assert_eq!(r.get_amount(), amount, "{product} original yield changed");
        let got: Vec<(String, i32)> = r.get_costs().to_vec();
        let want: Vec<(String, i32)> = costs.iter().map(|(n, a)| (n.to_string(), *a)).collect();
        assert_eq!(got, want, "{product} original cost changed");
    }

    // personal list: Tanner/Wickmaker/Trapper variants append AFTER their bases;
    // Fletcher's does not (no arrow original on the personal list)
    let mut craft = base.craft.clone();
    let n_base = craft.len();
    append_learned_variants(all, &mut craft);
    assert_eq!(
        craft.len(),
        n_base + 3,
        "personal list gains exactly 3 variants"
    );
    for r in &craft[..n_base] {
        assert!(!r.is_from_field_notes(), "originals must come first");
    }
    for r in &craft[n_base..] {
        assert!(
            r.is_from_field_notes(),
            "appended recipes must carry the tag"
        );
        assert_ne!(
            r.product_name(),
            "ARROW",
            "no arrow original here, no variant"
        );
    }

    // workbench list: Fletcher's and Wickmaker's variants land here (arrow + torch)
    let mut bench = base.workbench.clone();
    let n_bench = bench.len();
    append_learned_variants(all, &mut bench);
    assert_eq!(
        bench.len(),
        n_bench + 2,
        "workbench gains arrow + torch variants"
    );

    // a heat station without any of the originals gains nothing
    let mut oven = base.oven.clone();
    let n_oven = oven.len();
    append_learned_variants(all, &mut oven);
    assert_eq!(
        oven.len(),
        n_oven,
        "no variant without its original on the list"
    );

    // knowing nothing appends nothing
    let mut none = base.craft.clone();
    append_learned_variants(0, &mut none);
    assert_eq!(none.len(), n_base);
}

#[test]
fn learned_variant_shows_on_the_screen_and_crafts_at_its_cheaper_cost() {
    let mut tw = TestWorld::infinite().seed(0x12).build();
    let pid = tw.g.player_id;
    {
        let p = tw.g.entities.get_mut(pid).expect("player");
        p.player_mut().variants_learned = RecipeVariant::TannersStitch.bit();
    }
    tw.give("Hide", 2); // enough for the variant, not for the base (no cord)

    let player = tw.g.entities.take(pid).expect("player");
    let display = SurvivalDisplay::on_tab(&tw.g, &player, Tab::Craft);
    tw.g.entities.put_back(player);
    let leathers = display
        .craft_product_names(&tw.g)
        .iter()
        .filter(|n| n.eq_ignore_ascii_case("leather"))
        .count();
    assert_eq!(
        leathers, 2,
        "original + learned variant must both be listed"
    );

    // the variant crafts from Hide*2 alone; the original still wants its cord
    let variant = RecipeVariant::TannersStitch.recipe();
    let base = Recipes::new();
    let original = find_recipe(&base.craft, "LEATHER").clone();
    let mut player = tw.g.entities.take(pid).expect("player");
    {
        let inv = &mut player.player_mut().inventory;
        let mut orig = original.clone();
        assert!(!orig.check_can_craft(&tw.g, inv), "original needs a cord");
        assert!(variant.craft(&tw.g, inv), "variant crafts without cord");
        let leather = registry::get(&tw.g, "Leather");
        let hide = registry::get(&tw.g, "Hide");
        assert_eq!(inv.count(&leather), 2, "cheaper stitch still yields two");
        assert_eq!(inv.count(&hide), 0, "hides consumed");
    }
    tw.g.entities.put_back(player);

    // the improved yields, spelled out
    assert_eq!(RecipeVariant::WickmakersWick.recipe().get_amount(), 3);
    assert_eq!(RecipeVariant::FletchersFeathering.recipe().get_amount(), 5);
    let trapper = RecipeVariant::TrappersPattern.recipe();
    assert_eq!(
        trapper.get_costs(),
        &[("FUR".to_string(), 4), ("CORD".to_string(), 1)],
        "the leaner pattern: four furs, one cord"
    );
}

/* --------------------------------- save round-trip --------------------------------- */

fn reopen(g: &Game) -> Game {
    let mut g2 = Game::new(false, false, g.game_dir.clone());
    let mut player = mob::player::new(&g2, None);
    player.c.eid = 0;
    g2.entities.put_back(player);
    g2
}

#[test]
fn save_roundtrip_carries_learned_bits_and_old_saves_start_at_none() {
    let mut g1 = bare_game("variants_roundtrip");
    let dir = g1.game_dir.clone();

    let diff = g1.settings.get_idx("diff");
    for (i, &depth) in fdoom::level::IDX_TO_DEPTH.iter().enumerate() {
        let mut level = fdoom::level::Level::empty(128, 128, depth, diff);
        if depth == -4 {
            let obsidian = g1.tiles.get("Obsidian").id;
            level.tiles.iter_mut().for_each(|t| *t = obsidian);
        }
        g1.levels[i] = Some(level);
    }
    g1.settings.set_idx("mode", 0); // survival
    g1.current_level = 3;

    let bits = RecipeVariant::TannersStitch.bit() | RecipeVariant::FletchersFeathering.bit();
    {
        let p = g1.player_mut();
        p.c.level = Some(3);
        p.c.removed = false;
        p.player_mut().variants_learned = bits;
    }

    g1.world_name = "variantworld".to_string();
    save::save_world_named(&mut g1, "variantworld");

    let player_path = dir
        .join("saves/variantworld")
        .join(format!("Player{}", save::EXTENSION));
    let player_file = std::fs::read_to_string(&player_path).unwrap();
    assert!(
        player_file.contains(&format!("{}{}", save::VARIANTS_MARKER, bits)),
        "player save should carry the variants marker: {player_file}"
    );

    let mut g2 = reopen(&g1);
    load::load_world_named(&mut g2, "variantworld");
    assert_eq!(g2.player().player().variants_learned, bits);

    // old save: strip the marker — the player simply knows none, nothing panics
    let stripped: String = player_file
        .split(',')
        .filter(|f| !f.starts_with(save::VARIANTS_MARKER))
        .collect::<Vec<_>>()
        .join(",");
    std::fs::write(&player_path, stripped).unwrap();
    let mut g3 = reopen(&g1);
    load::load_world_named(&mut g3, "variantworld");
    assert_eq!(
        g3.player().player().variants_learned,
        0,
        "an old save without the marker starts with no variants"
    );
}

/* --------------------------------- loot seeding --------------------------------- */

fn chest_loot(g: &mut Game, kind: StructureKind, h: u64) -> Vec<String> {
    let mut chest = fdoom::entity::furniture::chest::new();
    fill_structure_chest(g, &mut chest, kind, h);
    let inv = &chest.chest().expect("chest").inventory;
    (0..inv.inv_size())
        .map(|i| inv.get(i).get_name().to_string())
        .collect()
}

fn scav_loot(
    g: &mut Game,
    structure: StructureKind,
    kind: ScavKind,
    age: TownAge,
    h: u64,
) -> Vec<String> {
    let mut c = fdoom::entity::furniture::scav_container::new(kind);
    fill_scav_container(g, &mut c, kind, structure, age, h);
    let inv = &c.chest().expect("scav container").inventory;
    (0..inv.inv_size())
        .map(|i| inv.get(i).get_name().to_string())
        .collect()
}

#[test]
fn loot_tables_seed_each_journal_in_its_place() {
    let mut g = bare_game("variants_loot");
    let sweep = 0..96u64; // ~1-in-8..12 odds: dozens of rolls land each journal

    let hits = |names: &[Vec<String>], want: &str| {
        names.iter().filter(|n| n.iter().any(|i| i == want)).count()
    };

    // Fletcher's Diary: ruins chests, and only there among the chest kinds
    let ruins: Vec<_> = sweep
        .clone()
        .map(|h| chest_loot(&mut g, StructureKind::Ruins, h))
        .collect();
    let village: Vec<_> = sweep
        .clone()
        .map(|h| chest_loot(&mut g, StructureKind::Village, h))
        .collect();
    let camp_chest: Vec<_> = sweep
        .clone()
        .map(|h| chest_loot(&mut g, StructureKind::Camp, h))
        .collect();
    assert!(
        hits(&ruins, "Fletcher's Diary") > 0,
        "ruins chests must seed the diary"
    );
    assert_eq!(hits(&village, "Fletcher's Diary"), 0);
    assert_eq!(hits(&camp_chest, "Fletcher's Diary"), 0);

    // Tanner's Notes: hamlet cupboards only (village cupboards stay pantry-plain)
    let hamlet_cup: Vec<_> = sweep
        .clone()
        .map(|h| {
            scav_loot(
                &mut g,
                StructureKind::Hamlet,
                ScavKind::Cupboard,
                TownAge::Settled,
                h,
            )
        })
        .collect();
    let village_cup: Vec<_> = sweep
        .clone()
        .map(|h| {
            scav_loot(
                &mut g,
                StructureKind::Village,
                ScavKind::Cupboard,
                TownAge::Settled,
                h,
            )
        })
        .collect();
    assert!(
        hits(&hamlet_cup, "Tanner's Notes") > 0,
        "hamlet cupboards must seed the notes"
    );
    assert_eq!(hits(&village_cup, "Tanner's Notes"), 0);

    // Wickmaker's Page: the camp supply crate
    let camp_crate: Vec<_> = sweep
        .clone()
        .map(|h| {
            scav_loot(
                &mut g,
                StructureKind::Camp,
                ScavKind::Crate,
                TownAge::Weathered,
                h,
            )
        })
        .collect();
    assert!(
        hits(&camp_crate, "Wickmaker's Page") > 0,
        "camp crates must seed the page"
    );

    // Trapper's Field Guide: overgrown-town time capsules — its own row, so the
    // Prospector's Note still rolls independently from the same containers
    let overgrown: Vec<_> = sweep
        .clone()
        .map(|h| {
            scav_loot(
                &mut g,
                StructureKind::Village,
                ScavKind::Barrel,
                TownAge::Overgrown,
                h,
            )
        })
        .collect();
    let settled: Vec<_> = sweep
        .clone()
        .map(|h| {
            scav_loot(
                &mut g,
                StructureKind::Village,
                ScavKind::Barrel,
                TownAge::Settled,
                h,
            )
        })
        .collect();
    assert!(
        hits(&overgrown, "Trapper's Field Guide") > 0,
        "time capsules must seed the guide"
    );
    assert!(
        hits(&overgrown, "Prospector's Note") > 0,
        "the note keeps its own slot"
    );
    assert_eq!(hits(&settled, "Trapper's Field Guide"), 0);
}

/* --------------------------------- notes pane --------------------------------- */

#[test]
fn notes_pane_counts_variants_learned() {
    assert_eq!(variants_learned_count(0), 0);
    assert_eq!(variants_learned_count(0b1111), VARIANT_COUNT);

    let g = bare_game("variants_notes_line");
    let mut player = mob::player::new(&g, None);
    player.player_mut().variants_learned =
        RecipeVariant::WickmakersWick.bit() | RecipeVariant::TrappersPattern.bit();
    let lines = survival_display::notes_lines(player.player());
    assert!(
        lines.contains(&("VARIANTS LEARNED".to_string(), "2/4".to_string())),
        "notes pane must carry the variants line: {lines:?}"
    );
}

/* --------------------------------- screenshots --------------------------------- */

#[test]
fn variants_screenshots() {
    let mut tw = TestWorld::infinite().seed(0x13).build();
    let pid = tw.g.player_id;
    // full daylight: the book reader draws black text over the world
    tw.g.change_time_of_day(fdoom::core::updater::Time::Day);

    // the journal open on its pages (voice check)
    read_book(&mut tw, "Tanner's Notes");
    tw.tick_n(1);
    tw.screenshot("variants_book.png");
    tw.g.exit_menu();

    // the toast moment, back in the world
    tw.tick_n(1);
    tw.screenshot("variants_toast.png");

    // CRAFT: original + variant side by side. With hides AND a cord both leather
    // recipes are craftable, so the sort puts them adjacent at the top (original
    // first); DOWN selects the variant so the card shows the FIELD NOTES tag.
    tw.give("Hide", 2);
    tw.give("Cord", 1);
    let player = tw.g.entities.take(pid).expect("player");
    let display = SurvivalDisplay::on_tab(&tw.g, &player, Tab::Craft);
    tw.g.entities.put_back(player);
    tw.g.set_menu(display);
    tw.tick_n(1);
    tw.press("DOWN");
    tw.screenshot("variants_craft.png");
    tw.g.exit_menu();

    // NOTES with the VARIANTS LEARNED line
    {
        let p = tw.g.entities.get_mut(pid).expect("player");
        p.player_mut().variants_learned |=
            fdoom::core::field_notes::RecipeVariant::TrappersPattern.bit();
    }
    let player = tw.g.entities.take(pid).expect("player");
    let display = SurvivalDisplay::on_tab(&tw.g, &player, Tab::Notes);
    tw.g.entities.put_back(player);
    tw.g.set_menu(display);
    tw.tick_n(1);
    tw.screenshot("variants_notes.png");
}
