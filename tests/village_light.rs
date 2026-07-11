//! Playtest improvement #8: generated village houses hold a lit lantern (and some
//! standing walls keep glazed window panes), so at night the interiors glow and the
//! light-occlusion system finally has a generated showcase.
//!
//! Set `VILLAGE_SHOT_DIR=/some/dir` to also dump night frames as PNGs.

use fdoom::core::updater::Time;
use fdoom::entity::EntityKind;
use fdoom::gfx::screen;
use fdoom::level::structures_gen::{
    Placement, StructureKind, lantern_positions, placements_in_rect,
};
use fdoom::testutil::{TestWorld, save_png};

/// Nearest Village placement to the origin for `seed`.
fn nearest_village(seed: i64) -> Placement {
    placements_in_rect(seed, -2048, -2048, 2048, 2048)
        .into_iter()
        .filter(|p| p.kind == StructureKind::Village)
        .min_by_key(|p| i64::from(p.x) * i64::from(p.x) + i64::from(p.y) * i64::from(p.y))
        .expect("no village within 2048 tiles")
}

fn shot(tw: &mut TestWorld, name: &str) {
    if let Ok(dir) = std::env::var("VILLAGE_SHOT_DIR") {
        let pixels = tw.render();
        let path = std::path::Path::new(&dir).join(name);
        save_png(&path, &pixels, screen::W as usize, screen::H as usize, 1);
    }
}

#[test]
fn village_houses_have_lanterns_and_windows() {
    let seed = 9;
    let mut tw = TestWorld::infinite().seed(seed).build();
    let v = nearest_village(seed);

    // stream the chunks around every building
    let expected = lantern_positions(seed, v);
    assert!(
        !expected.is_empty(),
        "a village must place at least one house lantern"
    );
    for &(tx, ty) in &expected {
        tw.teleport(tx, ty + 2);
        tw.tick_n(4);
    }
    let lvl = tw.g.current_level;

    // every house lantern position holds a real Lantern entity
    for &(tx, ty) in &expected {
        let found = tw.g.entities.iter().any(|e| {
            matches!(e.kind, EntityKind::Lantern(_)) && (e.c.x >> 4, e.c.y >> 4) == (tx, ty)
        });
        assert!(found, "no lantern entity at house tile ({tx},{ty})");
        // and it stands on sound plank floor, inside the house
        assert_eq!(
            tw.g.tile_at(lvl, tx, ty).name,
            "WOOD PLANKS",
            "lantern at ({tx},{ty}) must stand on plank floor"
        );
    }

    // the village keeps at least one glazed window pane in its standing walls
    let window_id = tw.g.tiles.get("Window").id;
    let mut windows = Vec::new();
    for dy in -24..=24i32 {
        for dx in -24..=24i32 {
            if tw.g.tile_at(lvl, v.x + dx, v.y + dy).id == window_id {
                windows.push((v.x + dx, v.y + dy));
            }
        }
    }
    assert!(
        !windows.is_empty(),
        "village at ({}, {}) generated no window panes",
        v.x,
        v.y
    );

    // night shots: from outside the window nearest a lantern (the glow-through-glass
    // beat scenario 9 asked for), then the same house's interior
    tw.g.change_time_of_day(Time::Night);
    let (wx, wy, lx, ly) = windows
        .iter()
        .flat_map(|&(wx, wy)| expected.iter().map(move |&(lx, ly)| (wx, wy, lx, ly)))
        .min_by_key(|&(wx, wy, lx, ly)| (wx - lx).pow(2) + (wy - ly).pow(2))
        .unwrap();
    // stand a few tiles on the far side of the window from the lantern
    let (ox, oy) = ((wx - lx).signum(), (wy - ly).signum());
    tw.teleport(wx + ox * 3, wy + oy * 3);
    tw.tick_n(4);
    shot(&mut tw, "village_house_night_outside.png");
    tw.teleport(lx, ly + 1);
    tw.tick_n(4);
    shot(&mut tw, "village_house_night_inside.png");
}
