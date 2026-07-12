//! Corner-HUD QOL checks (docs/UI_REDESIGN.md §2, lane L1): fixed-slot vitals rows
//! that hide while full, the held-item plate with its traffic-light durability bar,
//! the transient name label, the contextual ammo badge, creative's plate-only HUD,
//! and the removal of the old top-edge frame boxes. Renders real frames headlessly
//! and asserts on the framebuffer; PNGs land in target/verify for visual inspection.

use fdoom::core::updater::DAY_LENGTH;
use fdoom::gfx::screen;
use fdoom::item::{ItemKind, ToolType, registry};
use fdoom::testutil::TestWorld;

fn px(pixels: &[i32], (x, y): (i32, i32)) -> i32 {
    pixels[(y * screen::W + x) as usize]
}

/* Slot probes, mirroring renderer.rs geometry (mock-measured). Each vitals strip is
darken(2, row_y - 1, 84, 10); probes sit in the strip's left padding, clear of the
2px overlap rows shared with the neighboring strip. */
const HEARTS_PROBE: (i32, i32) = (2, 158);
const STAMINA_PROBE: (i32, i32) = (2, 170);
const FOOD_PROBE: (i32, i32) = (2, 178);
/// The reserved thirst row (y=182, L6): its slot must render nothing until the stat
/// ships. Probe below the food strip's last row (182).
const THIRST_PROBE: (i32, i32) = (2, 186);
/// Inside the transient name-label band (backing rows 145..154, right-aligned text).
const LABEL_PROBE: (i32, i32) = (282, 148);
/// Inside the count/ammo badge band above the plate (backing rows 161..170).
const AMMO_PROBE: (i32, i32) = (280, 164);
/// Left end of the durability gauge lane under the plate (filled while dur > 0).
const BAR_PROBE: (i32, i32) = (267, 189);

/// Nearest tile satisfying `pred` (outward ring search from the origin, same shape
/// as tests/temperature.rs).
fn find_spot(pred: impl Fn(i32, i32) -> bool) -> (i32, i32) {
    for r in 0i32..800 {
        let ring = r * 8;
        for dy in (-ring..=ring).step_by(8) {
            for dx in (-ring..=ring).step_by(8) {
                if (dx.abs() == ring || dy.abs() == ring) && pred(dx, dy) {
                    return (dx, dy);
                }
            }
        }
    }
    panic!("no matching staging tile near the origin");
}

fn full_stats(tw: &mut TestWorld) {
    let pd = tw.player_mut().player_mut();
    pd.mob.health = 10;
    pd.stamina = 10;
    pd.stamina_recharge_delay = 0;
    pd.hunger = 10;
}

#[test]
fn vitals_rows_hide_at_full_and_hold_their_slots() {
    let mut tw = TestWorld::infinite().name("hud_rows").build();
    // Pin the day clock: a pulse-on phase for the underline, and constant world
    // lighting so frame-vs-frame probes only see HUD changes.
    tw.g.tick_count = 30;
    full_stats(&mut tw);
    let base = tw.render(); // primes the HUD memory: full + settled = no rows

    // Damage: the hearts row appears in its fixed slot; nothing else does.
    tw.player_mut().player_mut().mob.health = 3;
    let hurt = tw.render();
    tw.screenshot("hud_rows_hurt.png");
    assert_ne!(
        px(&hurt, HEARTS_PROBE),
        px(&base, HEARTS_PROBE),
        "hearts row should appear when damaged"
    );
    assert_eq!(
        px(&hurt, STAMINA_PROBE),
        px(&base, STAMINA_PROBE),
        "stamina row must stay hidden at full"
    );
    assert_eq!(
        px(&hurt, FOOD_PROBE),
        px(&base, FOOD_PROBE),
        "food row must stay hidden at full"
    );
    // 3/10 hearts = at the 30% threshold: the 1px white pulse underline shows.
    assert_eq!(
        px(&hurt, (10, 166)),
        0xF0F0F0,
        "low hearts should draw the pulse underline"
    );

    // Heal back to full: the row lingers ~90 frames, then tucks away again.
    tw.player_mut().player_mut().mob.health = 10;
    let lingering = tw.render();
    assert_ne!(
        px(&lingering, HEARTS_PROBE),
        px(&base, HEARTS_PROBE),
        "a just-changed meter lingers briefly even at full"
    );
    for _ in 0..91 {
        tw.render();
    }
    let settled = tw.render();
    assert_eq!(
        px(&settled, HEARTS_PROBE),
        px(&base, HEARTS_PROBE),
        "healed hearts row must hide after the linger window"
    );

    // Hunger dips: the food row appears in ITS fixed slot — no reflowing upward
    // into the empty hearts/stamina slots.
    tw.player_mut().player_mut().hunger = 4;
    let hungry = tw.render();
    tw.screenshot("hud_rows_hungry.png");
    assert_ne!(
        px(&hungry, FOOD_PROBE),
        px(&base, FOOD_PROBE),
        "food row should appear when hungry"
    );
    assert_eq!(
        px(&hungry, HEARTS_PROBE),
        px(&base, HEARTS_PROBE),
        "hearts slot must stay empty — rows never reflow"
    );
    assert_eq!(
        px(&hungry, STAMINA_PROBE),
        px(&base, STAMINA_PROBE),
        "stamina slot must stay empty — rows never reflow"
    );

    // The reserved thirst slot renders nothing in any of these frames.
    for (name, frame) in [("base", &base), ("hurt", &hurt), ("hungry", &hungry)] {
        assert_eq!(
            px(frame, THIRST_PROBE),
            px(&base, THIRST_PROBE),
            "{name}: reserved thirst slot (y=182) must stay empty until L6"
        );
    }
}

#[test]
fn durability_bar_traffic_lights_on_the_plate() {
    let mut tw = TestWorld::infinite().name("hud").build();
    full_stats(&mut tw);

    let tool = registry::get(&tw, "Crude Pickaxe"); // max dur 28 * 1
    for (dur, name, check) in [
        (28, "hud_dur_full.png", "green"),
        (10, "hud_dur_mid.png", "amber"),
        (3, "hud_dur_low.png", "red"),
    ] {
        let mut t = tool.clone();
        if let ItemKind::Tool { dur: d, .. } = &mut t.kind {
            *d = dur;
        }
        tw.player_mut().player_mut().active_item = Some(t);
        let pixels = tw.render();
        tw.screenshot(name);

        let p = px(&pixels, BAR_PROBE);
        let (red, green) = ((p >> 16) & 0xFF, (p >> 8) & 0xFF);
        match check {
            "green" => assert!(
                green > red,
                "full bar should be green, got rgb {red},{green}"
            ),
            "amber" => assert!(
                green > 100 && red > 100,
                "mid bar should be amber, got rgb {red},{green}"
            ),
            "red" => assert!(red > green, "low bar should be red, got rgb {red},{green}"),
            _ => unreachable!(),
        }
    }

    // no bar without a tool held: the exact fill colors must vanish from the gauge spot
    let fills = [
        fdoom::gfx::color::upgrade(fdoom::gfx::color::get_byte(140)),
        fdoom::gfx::color::upgrade(fdoom::gfx::color::get_byte(540)),
        fdoom::gfx::color::upgrade(fdoom::gfx::color::get_byte(500)),
    ];
    tw.player_mut().player_mut().active_item = None;
    let pixels = tw.render();
    let p = px(&pixels, BAR_PROBE);
    assert!(
        !fills.contains(&p),
        "no tool held: the gauge lane should show plain world, got {p:#x}"
    );
}

#[test]
fn held_item_name_label_is_transient() {
    let mut tw = TestWorld::infinite().name("hud_label").build();
    full_stats(&mut tw);
    tw.player_mut().player_mut().active_item = None;
    let base = tw.render(); // primes with empty hands — no label

    // Switching items raises the right-aligned name label...
    let tool = registry::get(&tw, "Crude Pickaxe");
    tw.player_mut().player_mut().active_item = Some(tool);
    let switched = tw.render();
    tw.screenshot("hud_label_switch.png");
    assert_ne!(
        px(&switched, LABEL_PROBE),
        px(&base, LABEL_PROBE),
        "name label should appear on an item switch"
    );

    // ...which expires after ~90 frames (the old permanent, truncating text is gone).
    for _ in 0..92 {
        tw.render();
    }
    let settled = tw.render();
    tw.screenshot("hud_label_expired.png");
    assert_eq!(
        px(&settled, LABEL_PROBE),
        px(&base, LABEL_PROBE),
        "name label must expire"
    );

    // Switching again brings it right back.
    let wood = registry::get(&tw, "Wood");
    tw.player_mut().player_mut().active_item = Some(wood);
    let reswitched = tw.render();
    assert_ne!(
        px(&reswitched, LABEL_PROBE),
        px(&base, LABEL_PROBE),
        "label should reappear on the next switch"
    );
}

#[test]
fn ammo_badge_only_with_ranged_held() {
    let mut tw = TestWorld::infinite().name("hud_ammo").build();
    full_stats(&mut tw);
    tw.player_mut().player_mut().active_item = None;
    let base = tw.render();

    // A plain tool shows no counter — the permanent `X0` readout is impossible now.
    let pick = registry::get(&tw, "Crude Pickaxe");
    tw.player_mut().player_mut().active_item = Some(pick);
    let tool_frame = tw.render();
    assert_eq!(
        px(&tool_frame, AMMO_PROBE),
        px(&base, AMMO_PROBE),
        "non-ranged tool must not show an ammo badge"
    );

    // A bow attaches the arrow count to the plate.
    tw.give("arrow", 5);
    let bow = registry::new_tool_item(ToolType::Bow, 0);
    tw.player_mut().player_mut().active_item = Some(bow);
    let bow_frame = tw.render();
    tw.screenshot("hud_bow.png");
    assert_ne!(
        px(&bow_frame, AMMO_PROBE),
        px(&base, AMMO_PROBE),
        "bow should show the arrow count badge"
    );
}

#[test]
fn creative_shows_the_plate_only() {
    let mut tw = TestWorld::infinite()
        .name("hud_creative")
        .creative()
        .build();
    // Deplete everything: creative must still draw no vitals rows.
    {
        let pd = tw.player_mut().player_mut();
        pd.mob.health = 3;
        pd.stamina = 2;
        pd.hunger = 4;
        pd.active_item = None;
    }
    let creative = tw.render();
    tw.screenshot("hud_creative.png");

    // The plate is there (empty hands = the dim fist glyph)...
    let mut cell = Vec::new();
    for y in 171..187 {
        for x in 267..283 {
            cell.push(px(&creative, (x, y)));
        }
    }
    assert!(
        cell.contains(&0x3A3A3A) && cell.contains(&0x5A5A5A),
        "creative must still render the held plate (fist glyph missing)"
    );

    // ...and the depleted vitals are not: flipping to survival with the same stats
    // paints the rows the creative frame skipped.
    tw.g.settings.set("mode", "survival");
    let survival = tw.render();
    for (name, probe) in [
        ("hearts", HEARTS_PROBE),
        ("stamina", STAMINA_PROBE),
        ("food", FOOD_PROBE),
    ] {
        assert_ne!(
            px(&creative, probe),
            px(&survival, probe),
            "{name} row must be absent in creative and present in survival"
        );
    }
}

#[test]
fn top_frame_boxes_are_gone() {
    let mut tw = TestWorld::infinite().name("hud_frames").build();
    full_stats(&mut tw);
    tw.clear_notifications();
    let a = tw.render();

    // Move the camera; keep the clock (lighting) identical.
    let (tx, ty) = tw.player_tile();
    tw.teleport(tx + 160, ty);
    tw.clear_notifications();
    let b = tw.render();
    tw.screenshot("hud_top_edge_world.png");

    // The old chrome: health box x0..88 y0..40, held-item box x88..208 y0..24,
    // arrow box x208..288 y0..24 — each drew an opaque frame over the world every
    // frame. With them gone the world must show through: every region varies with
    // the camera. (This also covers the old temp-dot seam at x84..91, y13..20 and
    // the old `%` durability text.)
    for (name, x0, x1, y1) in [
        ("health box", 0, 88, 40),
        ("held-item box", 88, 208, 24),
        ("arrow box", 208, 288, 24),
    ] {
        let differs = (0..y1).any(|y| (x0..x1).any(|x| px(&a, (x, y)) != px(&b, (x, y))));
        assert!(
            differs,
            "{name} region identical across a teleport — frame chrome still drawn?"
        );
    }
}

#[test]
fn notifications_get_a_backing_band() {
    let mut tw = TestWorld::infinite().name("hud_note").build();
    full_stats(&mut tw);

    // render once without a notification, once with; the band must darken the backdrop.
    // The paragraph is anchored at (W/2, H*2/5) with RelPos::Top, i.e. the text block
    // sits just above the anchor — sample inside the band, left of the text.
    let probe = (screen::W / 2 - 44, screen::H * 2 / 5 - 4);
    let before = px(&tw.render(), probe);

    tw.push_warning("The ceiling groans...");
    let pixels = tw.render();
    tw.screenshot("hud_notification.png");
    let after = px(&pixels, probe);

    // compare summed channels: the band multiplies every channel down unless the pixel
    // is covered by the (brighter) text itself — sample a spot between glyph rows
    let sum = |p: i32| ((p >> 16) & 0xFF) + ((p >> 8) & 0xFF) + (p & 0xFF);
    assert!(
        sum(after) != sum(before),
        "notification band should change the backdrop (before={before:#x} after={after:#x})"
    );
}

/* ------------------------- mock-parity staging shots -------------------------
The frames the redesign is judged against (UI_REDESIGN §2): a calm frame that is
nearly HUD-free, and the worst-case alert frame with every system reporting at
once. Compared by eye against target/verify/ui_mock/mock_hud_{calm,alert}.png. */

#[test]
fn stage_mock_parity_screenshots() {
    // ---- calm: one meter shy of full, torch stack in hand, ambient ticker ----
    let mut tw = TestWorld::infinite().name("hud_calm").build();
    // a mild-climate land spot so noon sits in the comfort band (no temp dot, like
    // the mock) with actual terrain on screen
    let seed = tw.world_seed;
    let mild = find_spot(|x, y| {
        (0.45..0.55).contains(&fdoom::core::temperature::climate(seed, x, y))
            && matches!(
                fdoom::level::infinite_gen::biome_at(seed, x, y),
                fdoom::level::infinite_gen::Biome::Forest
                    | fdoom::level::infinite_gen::Biome::Plains
            )
    });
    tw.teleport(mild.0, mild.1);
    tw.tick_n(3); // stream the chunks in
    tw.set_time(DAY_LENGTH * 3 / 8 - 1); // noon
    tw.tick_n(1);
    full_stats(&mut tw);
    tw.player_mut().player_mut().hunger = 8;
    let mut torch = registry::get(&tw, "Torch");
    torch.set_count(8);
    tw.player_mut().player_mut().active_item = Some(torch);
    for _ in 0..95 {
        tw.render(); // let the switch label and change-linger windows expire
    }
    tw.clear_notifications();
    tw.notify_all("The tall grass holds fibers.");
    tw.render();
    println!("shot: {}", tw.screenshot("hud_calm.png").display());

    // ---- alert: hurt + winded + hungry + freezing + warning + fresh switch ----
    let mut tw = TestWorld::infinite().name("hud_alert").build();
    let seed = tw.world_seed;
    // hunt a properly cold spot, then pin deep night for the freeze
    let cold = find_spot(|x, y| fdoom::core::temperature::climate(seed, x, y) < 0.18);
    tw.teleport(cold.0, cold.1);
    tw.tick_n(3); // stream the chunks in
    tw.set_time(DAY_LENGTH * 7 / 8 - 1); // midnight
    tw.tick_n(1);
    {
        let pd = tw.player_mut().player_mut();
        pd.mob.health = 3;
        pd.stamina = 2;
        pd.stamina_recharge_delay = 0;
        pd.hunger = 4;
    }
    tw.render(); // prime the HUD memory bare-handed, so the knife counts as a switch
    let mut knife = registry::get(&tw, "Throwing Knife");
    knife.set_count(6);
    tw.player_mut().player_mut().active_item = Some(knife);
    tw.clear_notifications();
    tw.push_warning("The ceiling groans...");
    tw.render();
    println!("shot: {}", tw.screenshot("hud_alert.png").display());

    // ---- dev overlay: verify it still reads without the arrow box under it ----
    let mut tw = TestWorld::infinite().name("hud_dev").debug().build();
    tw.g.dev_overlay = true;
    tw.render();
    println!("shot: {}", tw.screenshot("hud_dev_overlay.png").display());
}
