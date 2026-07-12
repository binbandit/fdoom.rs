//! The excavation system: adjacent digs merge into one continuous excavated space,
//! water floods connected pit networks tile by tile and assumes their depth, dug
//! floors take furniture and floor material (base-in-a-hole), and dig-descent still
//! breaks through exactly as before.

use fdoom::entity::{Direction, EntityKind};
use fdoom::gfx::screen;
use fdoom::item::{interact as item_interact, registry};
use fdoom::level::tile::{depth, dispatch};
use fdoom::testutil::{TestWorld, renderer, save_png, verify_path};

/// Screen pixel where the probed tile's top-left corner lands.
const PX: i32 = 32;

/// Render only the tile registered as `def_name` at `(tx, ty)` through the real
/// dispatch and return its 16x16 pixel patch (row-major).
fn patch(
    tw: &mut TestWorld,
    scr: &mut fdoom::gfx::Screen,
    tx: i32,
    ty: i32,
    def_name: &str,
) -> Vec<i32> {
    let def = tw.g.tiles.get(def_name);
    let lvl = tw.g.current_level;
    scr.set_offset(tx * 16 - PX, ty * 16 - PX);
    dispatch::render(&mut tw.g, scr, &def, lvl, tx, ty);
    scr.set_offset(0, 0);
    let mut out = Vec::with_capacity(256);
    for y in 0..16 {
        for x in 0..16 {
            out.push(scr.pixels[((PX + x) + (PX + y) * screen::W) as usize]);
        }
    }
    out
}

fn at(p: &[i32], x: usize, y: usize) -> i32 {
    p[x + y * 16]
}

fn luma(p: i32) -> i32 {
    ((p >> 16) & 0xFF) + ((p >> 8) & 0xFF) + (p & 0xFF)
}

/* ------------------------------ merged rendering ------------------------------ */

/// Two adjacent max-stage pits: the shared boundary is open (both halves darken the
/// same seam rows — no ragged lip pixels between them), while the outer corners of
/// the merged dig stay untouched dirt.
#[test]
fn adjacent_pits_connect_into_one_excavation() {
    let mut tw = TestWorld::infinite().seed(555).name("exc_merge").build();
    let mut r = renderer();
    let lvl = tw.g.current_level;
    let (ptx, pty) = tw.player_tile();
    let (ax, ay) = (ptx + 2, pty);
    let (bx, by) = (ax + 1, ay);

    let base_a = patch(&mut tw, &mut r.screen, ax, ay, "dirt");
    let base_b = patch(&mut tw, &mut r.screen, bx, by, "dirt");
    tw.place_at("Dug Pit", ax, ay);
    tw.place_at("Dug Pit", bx, by);
    tw.g.level_mut(lvl).set_data(ax, ay, depth::MAX_STAGE);
    tw.g.level_mut(lvl).set_data(bx, by, depth::MAX_STAGE);

    let a = patch(&mut tw, &mut r.screen, ax, ay, "Dug Pit");
    let b = patch(&mut tw, &mut r.screen, bx, by, "Dug Pit");

    let mut open_rows = 0;
    for row in 0..16 {
        let a_dug = luma(at(&a, 15, row)) < luma(at(&base_a, 15, row));
        let b_dug = luma(at(&b, 0, row)) < luma(at(&base_b, 0, row));
        assert_eq!(
            a_dug, b_dug,
            "row {row}: the two halves of the shared boundary must agree"
        );
        if a_dug {
            open_rows += 1;
        }
    }
    assert!(
        open_rows >= 6,
        "shared boundary must be genuinely open, got {open_rows} darkened rows"
    );

    // the merged dig's outer corners are still plain dirt (ragged outline survives
    // on non-shared sides)
    for &(cx, cy) in &[(0usize, 0usize), (0, 15)] {
        assert_eq!(at(&a, cx, cy), at(&base_a, cx, cy), "left outer corner");
    }
    for &(cx, cy) in &[(15usize, 0usize), (15, 15)] {
        assert_eq!(at(&b, cx, cy), at(&base_b, cx, cy), "right outer corner");
    }

    // determinism: pure f(seed, x, y, neighbors)
    let again = patch(&mut tw, &mut r.screen, ax, ay, "Dug Pit");
    assert_eq!(a, again, "merged pit render must be deterministic");
}

/// The center tile of a 3x3 max-stage dig is open floor wall to wall: every pixel
/// darkened, no ragged edge pixels and no leftover dirt nubs at tile corners.
#[test]
fn interior_of_a_merged_dig_is_open_floor() {
    let mut tw = TestWorld::infinite().seed(556).name("exc_interior").build();
    let mut r = renderer();
    let lvl = tw.g.current_level;
    let (ptx, pty) = tw.player_tile();
    let (cx, cy) = (ptx + 3, pty - 3);

    let base = patch(&mut tw, &mut r.screen, cx, cy, "dirt");
    for dy in -1..=1 {
        for dx in -1..=1 {
            tw.place_at("Dug Pit", cx + dx, cy + dy);
            tw.g.level_mut(lvl)
                .set_data(cx + dx, cy + dy, depth::MAX_STAGE);
        }
    }
    let mid = patch(&mut tw, &mut r.screen, cx, cy, "Dug Pit");
    for y in 0..16 {
        for x in 0..16 {
            assert!(
                luma(at(&mid, x, y)) < luma(at(&base, x, y)),
                "interior pixel ({x},{y}) must be dug floor, not dirt"
            );
        }
    }
}

/* --------------------------------- flooding --------------------------------- */

/// Ring the channel with dirt so no natural water or terrain feature interferes.
fn dirt_apron(tw: &mut TestWorld, x0: i32, x1: i32, y0: i32, y1: i32) {
    for y in y0..=y1 {
        for x in x0..=x1 {
            tw.place_at("dirt", x, y);
        }
    }
}

/// Drive the random tile tick of every cell in the channel once (the level does
/// this on a ~1-in-50-per-tile cadence; calling dispatch::tick directly is the same
/// machinery, paced for a test).
fn tick_channel(tw: &mut TestWorld, lvl: usize, cells: &[(i32, i32)]) {
    for &(x, y) in cells {
        let def = tw.g.tile_at(lvl, x, y);
        dispatch::tick(&mut tw.g, &def, lvl, x, y);
    }
}

/// Water at the head of a four-pit channel floods the network over ticks, one tile
/// at a time outward from the source, and each flooded tile assumes the pit's
/// depth: shallow stages become water, bottomed-out pits become Deep Water.
#[test]
fn water_floods_a_pit_channel_and_assumes_depth() {
    let mut tw = TestWorld::infinite().seed(808).name("exc_flood").build();
    let lvl = tw.g.current_level;
    let (ptx, pty) = tw.player_tile();
    let (wx, y) = (ptx + 2, pty - 2);

    dirt_apron(&mut tw, wx - 1, wx + 6, y - 1, y + 1);
    tw.place_at("water", wx, y);
    let stages = [0, 1, depth::MAX_STAGE, depth::MAX_STAGE];
    for (i, &st) in stages.iter().enumerate() {
        let x = wx + 1 + i as i32;
        tw.place_at("Dug Pit", x, y);
        tw.g.level_mut(lvl).set_data(x, y, st);
    }
    let cells: Vec<(i32, i32)> = (0..=4).map(|dx| (wx + dx, y)).collect();

    let mut flooded_at = [None::<usize>; 4];
    for step in 0..20_000 {
        tick_channel(&mut tw, lvl, &cells);
        for (i, slot) in flooded_at.iter_mut().enumerate() {
            if slot.is_none() {
                let name = tw.g.tile_at(lvl, wx + 1 + i as i32, y).name.clone();
                if name.eq_ignore_ascii_case("water") || name.eq_ignore_ascii_case("Deep Water") {
                    *slot = Some(step);
                }
            }
        }
        if flooded_at.iter().all(Option::is_some) {
            break;
        }
    }
    assert!(
        flooded_at.iter().all(Option::is_some),
        "channel must fully flood: {flooded_at:?}"
    );
    // tile-by-tile spread: a pit can only flood after the one nearer the source
    for w in flooded_at.windows(2) {
        assert!(
            w[0].unwrap() <= w[1].unwrap(),
            "flood must spread outward from the source: {flooded_at:?}"
        );
    }
    // the water assumes the depth of each hole
    for (i, want) in ["water", "water", "Deep Water", "Deep Water"]
        .iter()
        .enumerate()
    {
        let name = tw.g.tile_at(lvl, wx + 1 + i as i32, y).name.clone();
        assert!(
            name.eq_ignore_ascii_case(want),
            "pit {i} (stage {}) must flood as {want}, got {name}",
            stages[i]
        );
    }
}

/// A chasm next to water floods to Deep Water — it assumes full depth and no longer
/// drops whoever crosses it (the level-transition tile is simply gone).
#[test]
fn a_flooded_chasm_becomes_deep_water_and_stops_dropping() {
    let mut tw = TestWorld::infinite()
        .seed(909)
        .name("exc_chasm_flood")
        .build();
    let lvl = tw.g.current_level;
    let (ptx, pty) = tw.player_tile();
    let (wx, y) = (ptx + 2, pty + 2);

    dirt_apron(&mut tw, wx - 1, wx + 2, y - 1, y + 1);
    tw.place_at("water", wx, y);
    tw.place_at("Chasm", wx + 1, y);

    for _ in 0..20_000 {
        let def = tw.g.tile_at(lvl, wx, y);
        dispatch::tick(&mut tw.g, &def, lvl, wx, y);
        if !tw
            .g
            .tile_at(lvl, wx + 1, y)
            .name
            .eq_ignore_ascii_case("Chasm")
        {
            break;
        }
    }
    assert!(
        tw.g.tile_at(lvl, wx + 1, y)
            .name
            .eq_ignore_ascii_case("Deep Water"),
        "flooded chasm must become Deep Water"
    );

    // standing on the flooded tile no longer descends
    tw.teleport(wx + 1, y);
    tw.g.player_mut().player_mut().on_stair_delay = 0;
    let before = tw.g.current_level;
    for _ in 0..100 {
        tw.tick_recover();
        tw.teleport(wx + 1, y);
    }
    assert_eq!(
        tw.g.current_level, before,
        "a flooded chasm must not drop the player a layer"
    );
}

/* ------------------------------- base in the hole ------------------------------- */

/// Furniture (a chest) places directly on a dug-pit floor via the ordinary item
/// pass; the tile stays a pit and the chest lands in it.
#[test]
fn furniture_places_on_a_pit_floor() {
    let mut tw = TestWorld::infinite()
        .seed(303)
        .name("exc_furniture")
        .build();
    let lvl = tw.g.current_level;
    let (tx, ty) = tw.place("Dug Pit", 1, 0);
    tw.g.level_mut(lvl).set_data(tx, ty, depth::MAX_STAGE);

    let mut chest = registry::get(&tw.g, "chest");
    let mut player = tw.g.entities.take(tw.g.player_id).expect("player");
    let placed = item_interact::item_interact_on_tile(
        &mut tw.g,
        &mut chest,
        lvl,
        tx,
        ty,
        &mut player,
        Direction::Down,
    );
    tw.g.entities.put_back(player);
    assert!(placed, "chest must place on a dug-pit floor");
    assert!(
        tw.g.tile_at(lvl, tx, ty)
            .name
            .eq_ignore_ascii_case("Dug Pit"),
        "placing furniture must not consume the pit"
    );
    let in_pit =
        tw.g.level(lvl).entities_to_add.iter().any(|e| {
            matches!(e.kind, EntityKind::Chest(_)) && (e.c.x >> 4, e.c.y >> 4) == (tx, ty)
        });
    assert!(in_pit, "the chest entity must sit in the pit tile");
}

/// Floor material boards a dug pit over (the classic hole behavior), and dirt
/// backfills it one stage per shovel-load until the ground is whole again.
#[test]
fn floors_and_backfill_work_in_pits() {
    let mut tw = TestWorld::infinite().seed(404).name("exc_floors").build();
    let lvl = tw.g.current_level;

    // planks lay a floor over the dig
    let (px, py) = tw.place("Dug Pit", 1, 0);
    tw.g.level_mut(lvl).set_data(px, py, depth::MAX_STAGE);
    assert!(tw.interact_with("Plank", 1, 0), "plank must lay on a pit");
    assert!(
        tw.g.tile_at(lvl, px, py)
            .name
            .eq_ignore_ascii_case("Wood Planks"),
        "pit floored with planks"
    );

    // dirt backfills stage by stage
    let (dx, dy) = tw.place("Dug Pit", 2, 0);
    tw.g.level_mut(lvl).set_data(dx, dy, depth::MAX_STAGE);
    for want in (0..depth::MAX_STAGE).rev() {
        assert!(tw.interact_with("Dirt", 2, 0), "dirt backfill");
        assert_eq!(tw.g.level(lvl).get_data(dx, dy), want, "one stage per load");
    }
    assert!(tw.interact_with("Dirt", 2, 0), "final fill");
    assert!(
        tw.g.tile_at(lvl, dx, dy).name.eq_ignore_ascii_case("dirt"),
        "a fully backfilled pit is dirt again"
    );
}

/* ------------------------------ dig-descent regression ------------------------------ */

/// MAX_STAGE + pickaxe still opens a chasm and stamps the ladder on the layer below
/// (the full descend/ascend loop is covered in tests/multi_level_terrain.rs).
#[test]
fn dig_descent_still_breaks_through_and_stamps_the_ladder() {
    let mut tw = TestWorld::infinite().seed(31337).name("exc_dig").build();
    let lvl = tw.g.current_level;
    let (tx, ty) = tw.place("Dug Pit", 1, 0);
    tw.g.level_mut(lvl).set_data(tx, ty, depth::MAX_STAGE);

    assert!(
        tw.interact_with("Gem Pickaxe", 1, 0),
        "pickaxe must break through a bottomed-out pit"
    );
    assert!(
        tw.g.tile_at(lvl, tx, ty).name.eq_ignore_ascii_case("Chasm"),
        "breakthrough opens a chasm"
    );
    assert!(
        tw.g.tile_at(lvl - 1, tx, ty)
            .name
            .eq_ignore_ascii_case("Ladder"),
        "matching ladder stamped one layer down"
    );
}

/* --------------------------------- screenshots --------------------------------- */

/// Pin the clock to midday so shots are judged in flat daylight (settling ticks
/// advance time, so pin, settle, pin again — the visuals.rs pattern).
fn pin_noon(tw: &mut TestWorld) {
    tw.tick_n(8);
    let noon = fdoom::core::updater::DAY_LENGTH / 3;
    tw.g.set_time(noon);
    tw.tick_n(2);
    tw.g.set_time(noon);
}

fn shot(tw: &mut TestWorld, name: &str) {
    // weather/event banners would block the terrain being judged
    tw.g.warnings.clear();
    tw.g.clear_notifications();
    let path = tw.screenshot(&format!("{name}.png"));
    assert!(path.exists());
    let pixels = tw.render();
    save_png(
        verify_path(&format!("{name}_6x.png")),
        &pixels,
        screen::W as usize,
        screen::H as usize,
        6,
    );
}

/// Eyeball material: a merged terraced 3x2 excavation with a breakthrough chasm,
/// and a channel from open water caught mid-flood, then fully flooded into a deep
/// pool. Dumped at 1x and 6x into target/verify for a human to actually look at.
#[test]
fn excavation_screenshots_for_eyeballing() {
    let mut tw = TestWorld::infinite()
        .seed(20260712)
        .name("exc_shots")
        .build();
    pin_noon(&mut tw);
    let lvl = tw.g.current_level;
    let (ptx, pty) = tw.player_tile();

    // a merged 3x2 excavation, terraced 0/1/2 with a chasm broken through one cell
    // (NE of the player, clear of the HUD panel)
    dirt_apron(&mut tw, ptx + 1, ptx + 5, pty - 4, pty - 1);
    for (i, &st) in [0, 1, 2, 1, 2, 2].iter().enumerate() {
        let (tx, ty) = (ptx + 2 + (i as i32 % 3), pty - 3 + (i as i32 / 3));
        tw.place_at("Dug Pit", tx, ty);
        tw.g.level_mut(lvl).set_data(tx, ty, st);
    }
    tw.place_at("Chasm", ptx + 3, pty - 2);
    shot(&mut tw, "excavation_merged_3x2");

    // a channel of pits off a pond, caught mid-flood and then fully flooded
    dirt_apron(&mut tw, ptx - 2, ptx + 4, pty + 1, pty + 3);
    tw.place_at("water", ptx - 1, pty + 2);
    tw.place_at("water", ptx - 2, pty + 2);
    let stages = [0, 1, 1, depth::MAX_STAGE, depth::MAX_STAGE];
    for (i, &st) in stages.iter().enumerate() {
        let x = ptx + i as i32;
        tw.place_at("Dug Pit", x, pty + 2);
        tw.g.level_mut(lvl).set_data(x, pty + 2, st);
    }
    let cells: Vec<(i32, i32)> = (-2..=4).map(|dx| (ptx + dx, pty + 2)).collect();
    let flooded = |tw: &TestWorld, upto: i32| {
        (0..upto).all(|i| {
            let t = tw.g.tile_at(lvl, ptx + i, pty + 2);
            t.connects_to_water
        })
    };
    for _ in 0..20_000 {
        tick_channel(&mut tw, lvl, &cells);
        if flooded(&tw, 2) {
            break;
        }
    }
    pin_noon(&mut tw);
    shot(&mut tw, "excavation_flood_midway");
    for _ in 0..20_000 {
        tick_channel(&mut tw, lvl, &cells);
        if flooded(&tw, 5) {
            break;
        }
    }
    pin_noon(&mut tw);
    shot(&mut tw, "excavation_pool_full");
}

/// Eyeball material: a furnished dug basin — a 3x2 max-stage excavation with a
/// chest and a lantern standing on the darkened floor.
#[test]
fn basement_screenshot_for_eyeballing() {
    let mut tw = TestWorld::infinite().seed(424242).name("exc_base").build();
    pin_noon(&mut tw);
    let lvl = tw.g.current_level;
    let (ptx, pty) = tw.player_tile();

    dirt_apron(&mut tw, ptx - 4, ptx + 1, pty - 3, pty + 1);
    for dy in 0..2 {
        for dx in 0..3 {
            let (tx, ty) = (ptx - 3 + dx, pty - 2 + dy);
            tw.place_at("Dug Pit", tx, ty);
            tw.g.level_mut(lvl).set_data(tx, ty, depth::MAX_STAGE);
        }
    }
    for (item, dx, dy) in [("chest", -3, -2), ("Lantern", -1, -1)] {
        let mut it = registry::get(&tw.g, item);
        let mut player = tw.g.entities.take(tw.g.player_id).expect("player");
        let placed = item_interact::item_interact_on_tile(
            &mut tw.g,
            &mut it,
            lvl,
            ptx + dx,
            pty + dy,
            &mut player,
            Direction::Down,
        );
        tw.g.entities.put_back(player);
        assert!(placed, "{item} must place in the basin");
    }
    tw.tick_n(2);
    let noon = fdoom::core::updater::DAY_LENGTH / 3;
    tw.g.set_time(noon);
    shot(&mut tw, "excavation_basement");
}
