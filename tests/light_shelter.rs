//! Light & shelter wave: occlusion-aware emitter stamping (walls stop light,
//! windows and open doors transmit it), the Window tile's wall-like solidity and
//! glass-shatter break, and the Glass/Window recipes.

use fdoom::core::renderer::Renderer;
use fdoom::gfx::{lighting, screen};
use fdoom::level::tile::dispatch;
use fdoom::testutil::{TestWorld, bare_game, find_recipe, renderer};

/// Build a sealed 5x5 stone room (wall ring, brick floor) centered `(4, 0)` tiles
/// from the player, with a Gold Lantern (light radius 15 -> 120 px) at its center.
/// Returns the room's center tile.
fn build_lit_room(tw: &mut TestWorld) -> (i32, i32) {
    let (ptx, pty) = tw.player_tile();
    let (cx, cy) = (ptx + 4, pty);
    for dy in -2i32..=2 {
        for dx in -2i32..=2 {
            let name = if dx.abs() == 2 || dy.abs() == 2 {
                "Stone Wall"
            } else {
                "Stone Bricks"
            };
            tw.place_at(name, cx + dx, cy + dy);
        }
    }
    let lvl = tw.current_level;
    let mut lantern = fdoom::entity::furniture::lantern::new(
        fdoom::entity::furniture::lantern::LanternType::Gold,
    );
    lantern.c.x = (cx << 4) + 8;
    lantern.c.y = (cy << 4) + 8;
    tw.g.level_mut(lvl).add(lantern, lvl);
    tw.tick_n(2); // flush the add-queue into the entity arena
    (cx, cy)
}

/// Stamp the frame's emitters with the camera centered on `(cx, cy)` and return the
/// raw light value (0-255) at the center of tile `(tx, ty)`.
fn light_at(tw: &TestWorld, r: &mut Renderer, cx: i32, cy: i32, tx: i32, ty: i32) -> i32 {
    let x_scroll = (cx << 4) + 8 - screen::W / 2;
    let y_scroll = (cy << 4) + 8 - screen::H / 2;
    lighting::stamp_emitters(
        &mut r.light_screen,
        &tw.g,
        tw.current_level,
        x_scroll,
        y_scroll,
    );
    let sx = (tx << 4) + 8 - x_scroll;
    let sy = (ty << 4) + 8 - y_scroll;
    assert!(
        (0..screen::W).contains(&sx) && (0..screen::H).contains(&sy),
        "sample tile ({tx}, {ty}) off screen"
    );
    r.light_screen.pixels[(sx + sy * screen::W) as usize]
}

#[test]
fn walls_contain_light_windows_beam_it_out() {
    let mut tw = TestWorld::infinite().seed(0xC0FFEE).build();
    tw.tick_n(8); // stream chunks around spawn
    let (cx, cy) = build_lit_room(&mut tw);
    let mut r = renderer();

    // Sealed room: the interior and the wall faces are lit...
    assert!(
        light_at(&tw, &mut r, cx, cy, cx + 1, cy) > 0,
        "interior dark"
    );
    assert!(
        light_at(&tw, &mut r, cx, cy, cx + 2, cy) > 0,
        "the wall's own face should catch light"
    );
    // ...and every tile just outside the ring is pitch black.
    for (tx, ty) in [(cx + 3, cy), (cx - 3, cy), (cx, cy + 3), (cx, cy - 3)] {
        assert_eq!(
            light_at(&tw, &mut r, cx, cy, tx, ty),
            0,
            "light leaked through the sealed wall to ({tx}, {ty})"
        );
    }

    // Swap the east wall's center for a Window: the tile straight beyond it now
    // catches a beam, the diagonally offset outside tiles stay in wall shadow, and
    // the other three sides stay dark.
    tw.place_at("Window", cx + 2, cy);
    assert!(
        light_at(&tw, &mut r, cx, cy, cx + 3, cy) > 0,
        "window should transmit a beam to the tile beyond it"
    );
    assert_eq!(
        light_at(&tw, &mut r, cx, cy, cx + 3, cy + 2),
        0,
        "the beam should stay a beam — wall shadow beside the window"
    );
    assert_eq!(light_at(&tw, &mut r, cx, cy, cx, cy - 3), 0);

    // Eyeball frame (target/verify): the lit room + window beam, at night.
    tw.g.set_time((fdoom::core::updater::DAY_LENGTH as f32 * 0.85) as i32);
    tw.screenshot("light_shelter_window_beam.png");
}

#[test]
fn closed_doors_block_light_open_doors_spill_it() {
    let mut tw = TestWorld::infinite().seed(0xC0FFEE).build();
    tw.tick_n(8);
    let (cx, cy) = build_lit_room(&mut tw);
    let lvl = tw.current_level;
    let mut r = renderer();

    // A closed door (default data 0) seals like the wall it replaced.
    tw.place_at("Stone Door", cx + 2, cy);
    assert_eq!(
        light_at(&tw, &mut r, cx, cy, cx + 3, cy),
        0,
        "closed door leaked light"
    );

    // Opening it (data 1, the same state `door::may_pass` reads) spills the beam.
    tw.g.level_mut(lvl).set_data(cx + 2, cy, 1);
    assert!(
        light_at(&tw, &mut r, cx, cy, cx + 3, cy) > 0,
        "open doorway should spill light"
    );
}

#[test]
fn window_is_solid_and_shatters_into_glass() {
    let mut tw = TestWorld::infinite().seed(0x91A55).build();
    tw.tick_n(4);
    let lvl = tw.current_level;

    // Solid like a wall: no entity may pass.
    let (wx, wy) = tw.place("Window", 1, 0);
    let def = tw.tile_at(lvl, wx, wy);
    let probe = fdoom::entity::mob::player::new(&tw.g, None);
    assert!(
        !dispatch::may_pass(&tw.g, &def, lvl, wx, wy, &probe),
        "windows must block movement"
    );

    // One hit shatters the pane, leaving the frame's planks; Glass drops ~50%.
    let mut breaks = 0;
    for _ in 0..30 {
        tw.place("Window", 1, 0);
        assert!(tw.hit(1, 0, 1), "window did not react to a hit");
        assert_eq!(tw.tile_at(lvl, wx, wy).name, "WOOD PLANKS");
        breaks += 1;
    }
    let glass_drops = tw
        .dropped_items()
        .iter()
        .filter(|n| n.as_str() == "glass")
        .count();
    assert!(
        glass_drops >= 1 && glass_drops < breaks,
        "expected ~50% glass drops from {breaks} breaks, got {glass_drops}"
    );
}

#[test]
fn glass_and_window_recipes() {
    let g = bare_game("light_shelter_recipes");

    // Furnace: 2 sand + coal (the fuel, as in the ore smelts) -> 1 Glass.
    let glass = find_recipe(&g.recipes.furnace, "glass");
    assert_eq!(glass.get_amount(), 1);
    let costs = glass.get_costs();
    assert!(
        costs.contains(&("SAND".to_string(), 2)),
        "glass costs {costs:?}"
    );
    assert!(
        costs.contains(&("COAL".to_string(), 1)),
        "glass costs {costs:?}"
    );

    // Workbench: 2 Glass + 2 Wood -> Window.
    let window = find_recipe(&g.recipes.workbench, "Window");
    assert_eq!(window.get_amount(), 1);
    let costs = window.get_costs();
    assert!(
        costs.contains(&("GLASS".to_string(), 2)),
        "window costs {costs:?}"
    );
    assert!(
        costs.contains(&("WOOD".to_string(), 2)),
        "window costs {costs:?}"
    );
}

/// Worst-case occlusion cost: a gold lantern (the widest emitter) inside a wall
/// ring forces the mask path every stamp. Keep it comfortably inside the frame
/// budget alongside `tests/lighting.rs`'s whole-pass assert.
#[test]
fn occluded_stamping_stays_fast() {
    let mut tw = TestWorld::infinite().seed(0xC0FFEE).build();
    tw.tick_n(8);
    let (cx, cy) = build_lit_room(&mut tw);
    let mut r = renderer();
    let x_scroll = (cx << 4) + 8 - screen::W / 2;
    let y_scroll = (cy << 4) + 8 - screen::H / 2;

    let iters = 100;
    let t0 = std::time::Instant::now();
    for _ in 0..iters {
        lighting::stamp_emitters(
            &mut r.light_screen,
            &tw.g,
            tw.current_level,
            x_scroll,
            y_scroll,
        );
    }
    let avg = t0.elapsed() / iters;
    println!("occluded stamp_emitters avg: {avg:?} over {iters} iters");
    // Debug builds are far slower than the ~2ms release frame budget; this ceiling
    // only catches an accidental blow-up in the mask math.
    assert!(
        avg < std::time::Duration::from_millis(15),
        "occluded emitter stamping too slow: {avg:?}"
    );
}
