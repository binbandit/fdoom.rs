use std::sync::Arc;

use fdoom::entity::EntityKind;
use fdoom::gfx::screen::{H, Screen, W};
use fdoom::platform::logical_size_for_window;
use fdoom::screen::survival_display::Layout;
use fdoom::testutil::TestWorld;

const DIVIDER_RGB: i32 = 0x4A4A4A;
const SCROLLBAR_RGB: i32 = 0x9A9A9A;

#[test]
fn integer_scale_and_logical_size_follow_the_window_contract() {
    assert_eq!(logical_size_for_window(800, 600), (2, 400, 300));
    assert_eq!(logical_size_for_window(288, 192), (1, 288, 192));
    assert_eq!(logical_size_for_window(4000, 3000), (6, 640, 400));
    assert_eq!(logical_size_for_window(200, 100), (1, 288, 192));
}

#[test]
fn runtime_screen_keeps_classic_constructor_and_allocates_requested_size() {
    let sheet = Arc::new(fdoom::assets::sprite_sheet());
    let classic = Screen::new(sheet.clone());
    assert_eq!(
        (classic.w, classic.h, classic.pixels.len()),
        (W, H, (W * H) as usize)
    );

    let wide = Screen::with_size(384, 240, sheet);
    assert_eq!((wide.w, wide.h, wide.pixels.len()), (384, 240, 384 * 240));
    assert_eq!(wide.center().x, 192);
    assert_eq!(wide.center().y, 120);
}

/* --------------------- the survival/container shell layout --------------------- */

/// At the classic 288x192, every Layout field must equal the shipped constants —
/// this is what keeps the whole classic-coordinate test fleet (survival_screen,
/// ui_l4, wear_equip, bench) valid without modification.
#[test]
fn layout_at_classic_equals_the_shipped_geometry() {
    let l = Layout::new(288, 192);
    assert_eq!(
        (l.panel_x, l.panel_y, l.panel_w, l.panel_h),
        (8, 8, 272, 176)
    );
    assert_eq!((l.tab_y, l.underline_y), (13, 22));
    assert_eq!((l.body_y, l.body_bottom), (28, 166));
    assert_eq!(
        (
            l.list_x,
            l.list_right,
            l.divider_x,
            l.detail_x,
            l.detail_right
        ),
        (12, 146, 148, 154, 276)
    );
    assert_eq!((l.row_h, l.max_rows, l.legend_y), (10, 13, 170));
    assert_eq!((l.wear_box_x, l.wear_label_x, l.wear_port_x), (26, 48, 196));
    assert_eq!(
        (l.mid_x, l.rule_y, l.cont_list_y, l.cont_max_rows),
        (144, 25, 33, 13)
    );
    assert_eq!((l.l_right, l.r_left), (138, 152));
    assert_eq!(Layout::classic(), l);
}

/// A bigger logical screen grows the panel to the caps, keeps it centered, gives
/// the list ~55% of the extra width, and fits more rows.
#[test]
fn layout_at_384x240_grows_centers_and_fits_more_rows() {
    let l = Layout::new(384, 240);
    assert_eq!(
        (l.panel_x, l.panel_y, l.panel_w, l.panel_h),
        (24, 8, 336, 224)
    );
    // 64px extra width; the list takes 55% of it: 24 + 140 + 35
    assert_eq!(l.divider_x, 199);
    assert_eq!((l.list_right, l.detail_x, l.detail_right), (197, 205, 356));
    assert_eq!((l.row_h, l.max_rows), (11, 16));
    assert!(l.max_rows > 13, "a taller body must expose more rows");
    assert_eq!((l.mid_x, l.cont_max_rows), (192, 16));
    assert_eq!(l.legend_y, 218);
}

/* ------------------------- rendered panes at 384x240 ------------------------- */

/// The PACK pane genuinely uses the wider panel: the divider renders at the
/// proportional split, and a long item name paints past the CLASSIC divider x
/// (148) while still clipping inside the new one.
#[test]
fn survival_pack_widens_with_the_window() {
    let l = Layout::new(384, 240);
    let band = |px: &[i32], x0: i32, x1: i32, y0: i32, y1: i32| -> Vec<i32> {
        let mut out = Vec::new();
        for y in y0..y1 {
            for x in x0..x1 {
                out.push(px[(y * 384 + x) as usize]);
            }
        }
        out
    };

    let mut long_w = TestWorld::infinite().name("dyn_long").build();
    long_w.give("Prospector's Pan", 1);
    long_w.press("E");
    let long_px = long_w.render_at(384, 240);
    long_w.screenshot_at("dyn_pack_384.png", 384, 240);
    long_w.screenshot("dyn_pack_288.png");

    // the list|detail divider sits at the proportional x, inside the body
    assert_eq!(
        long_px[(100 * 384 + l.divider_x) as usize],
        DIVIDER_RGB,
        "divider must render at the widened split x={}",
        l.divider_x
    );

    let mut short_w = TestWorld::infinite().name("dyn_short").build();
    short_w.give("Wood", 1);
    short_w.press("E");
    let short_px = short_w.render_at(384, 240);

    // the item row band beyond the CLASSIC divider (x=148) up to the new list
    // edge: the 16-char name paints there, the 4-char one does not
    let row_y0 = l.body_y + l.row_h; // row 1: the item under its category header
    let long_name_band = band(&long_px, 149, l.list_right, row_y0, row_y0 + 8);
    let short_name_band = band(&short_px, 149, l.list_right, row_y0, row_y0 + 8);
    assert_ne!(
        long_name_band, short_name_band,
        "a long name must use the widened list column (paint past classic x=148)"
    );

    // ...and the guard band between the new divider and the detail card stays
    // clean (the clip really is the new divider, not unbounded)
    let long_guard = band(&long_px, l.divider_x + 1, l.detail_x, 0, 240);
    let short_guard = band(&short_px, l.divider_x + 1, l.detail_x, 0, 240);
    assert_eq!(
        long_guard, short_guard,
        "names must still clip before the widened divider"
    );
}

/// More rows really show: a 14-row pack overflows the classic 13-row body (its
/// scrollbar renders) but fits the 16-row body at 384x240 (no scrollbar).
#[test]
fn taller_pack_body_shows_more_rows() {
    let mut tw = TestWorld::infinite().name("dyn_rows").build();
    for _ in 0..13 {
        tw.give("Crude Axe", 1); // non-stackable: 13 item rows + 1 header = 14
    }
    tw.press("E");

    let classic = tw.render();
    let classic_bar = (28..166).any(|y| classic[(y * W + 148) as usize] == SCROLLBAR_RGB);
    assert!(classic_bar, "14 rows must overflow the classic 13-row body");

    let wide = tw.render_at(384, 240);
    let l = Layout::new(384, 240);
    let wide_bar =
        (l.body_y..l.body_bottom).any(|y| wide[(y * 384 + l.divider_x) as usize] == SCROLLBAR_RGB);
    assert!(
        !wide_bar,
        "the same 14 rows must fit the {} rows of the taller body",
        l.max_rows
    );
}

/// The container shell's two-pane split follows the widened panel too.
#[test]
fn container_shell_widens_with_the_window() {
    let mut tw = TestWorld::infinite().name("dyn_chest").build();
    let pid = tw.player_id;
    let lvl = tw.current_level;

    let mut chest = fdoom::entity::furniture::chest::new();
    if let EntityKind::Chest(c) = &mut chest.kind {
        c.inventory
            .add(fdoom::item::registry::get(&tw.g, "Prospector's Pan"));
    }
    let (px, py) = {
        let p = tw.player_mut();
        (p.c.x, p.c.y)
    };
    chest.c.x = px + 16;
    chest.c.y = py;
    tw.g.level_mut(lvl).add(chest, lvl);
    fdoom::level::tick_level(&mut tw.g, lvl, false);
    let chest_eid =
        tw.g.entities
            .entities_on_level(lvl)
            .find(|e| matches!(e.kind, EntityKind::Chest(_)))
            .map(|e| e.c.eid)
            .expect("chest placed");
    tw.give("Wood", 3);
    tw.g.with_entity(pid, |player, g| {
        g.with_entity(chest_eid, |chest, g| {
            fdoom::entity::furniture::chest_behavior::use_furniture(g, chest, player);
        });
    });
    tw.tick();
    assert!(tw.display.menu_active(), "chest should open the shell");

    let wide = tw.render_at(384, 240);
    let l = Layout::new(384, 240);
    assert_eq!(
        wide[(100 * 384 + l.mid_x) as usize],
        DIVIDER_RGB,
        "the two-pane divider must sit at the widened panel's middle x={}",
        l.mid_x
    );
    tw.screenshot_at("dyn_chest_384.png", 384, 240);
    tw.screenshot("dyn_chest_288.png");
}
