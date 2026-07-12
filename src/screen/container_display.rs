//! The container screen — the survival shell's two-column variant (UI_REDESIGN
//! §3.5, `mock_chest.png`): container list | pack list in equal fixed panes, the
//! container's name and PACK as the two titles. LEFT/RIGHT switches side, ENTER
//! moves the stack, Q moves one, ESC returns to the world. Replaces the old
//! edge-pinned double-inventory dance (J10); move semantics are unchanged,
//! including the creative keep-a-copy rule.

use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Entity;
use crate::gfx::{Rectangle, Screen, color, font};
use crate::item::Item;

use super::display::{Display, DisplayBase};
use super::menu::{Menu, MenuBuilder};
use super::rel_pos::RelPos;
use super::survival_display::{
    BODY_BOTTOM, DETAIL_RIGHT, DIVIDER_RGB, GOLD_RGB, LEGEND_Y, LIST_X, PANEL_H, PANEL_W, PANEL_X,
    PANEL_Y, ROW_H, SCROLLBAR_RGB, TAB_Y, bare_name, fill_rect,
};

/* ------------------------- geometry (mock_chest, exact) ------------------------- */

/// The vertical divider splits the panel into two equal panes.
const MID_X: i32 = PANEL_X + PANEL_W / 2; // 144
/// The horizontal rule under the two titles.
const RULE_Y: i32 = 25;
/// First list row.
const LIST_Y: i32 = 33;
const MAX_ROWS: i32 = (BODY_BOTTOM - LIST_Y) / ROW_H; // 13

/// Left pane list span (cursor at LIST_X, count right-aligned at the pane edge).
const L_RIGHT: i32 = MID_X - 6;
/// Right pane list span.
const R_LEFT: i32 = MID_X + 8;
const R_RIGHT: i32 = DETAIL_RIGHT;

const SIDE_CONTAINER: usize = 0;
const SIDE_PACK: usize = 1;

pub struct ContainerDisplay {
    base: DisplayBase,
    player_eid: i32,
    chest_eid: i32,
    /// The container's display title (its furniture name, upcased at draw time).
    title: String,
    /// The glass panel + 9-slice edge (house shell styling).
    shell_menu: Menu,
    /// Focused side: 0 = the container, 1 = the pack.
    side: usize,
    sel: [i32; 2],
    off: [i32; 2],
}

impl ContainerDisplay {
    pub fn new(g: &Game, player: &Entity, chest: &Entity) -> ContainerDisplay {
        let title = chest
            .furniture()
            .expect("container must be furniture")
            .name
            .clone();
        let shell_menu = MenuBuilder::new(true, 0, RelPos::Center, Vec::new())
            .set_bounds(Rectangle::new(
                PANEL_X,
                PANEL_Y,
                PANEL_W,
                PANEL_H,
                Rectangle::CORNER_DIMS,
            ))
            .set_selectable(false)
            .create_menu(g);

        // an emptied container greets you on the pack side — there is nothing to
        // take, so the cursor starts where the action is
        let chest_empty = chest.chest().is_none_or(|c| c.inventory.inv_size() == 0);

        ContainerDisplay {
            base: DisplayBase::new(false, true, Vec::new()),
            player_eid: player.c.eid,
            chest_eid: chest.c.eid,
            title,
            shell_menu,
            side: if chest_empty {
                SIDE_PACK
            } else {
                SIDE_CONTAINER
            },
            sel: [0, 0],
            off: [0, 0],
        }
    }

    /// The focused side — public for tests.
    pub fn focused_side(&self) -> usize {
        self.side
    }

    fn side_items(&self, g: &Game, side: usize) -> Vec<Item> {
        let eid = if side == SIDE_CONTAINER {
            self.chest_eid
        } else {
            self.player_eid
        };
        let Some(e) = g.entities.get(eid) else {
            return Vec::new();
        };
        if side == SIDE_CONTAINER {
            e.chest()
                .map(|c| c.inventory.items().to_vec())
                .unwrap_or_default()
        } else {
            e.player().inventory.items().to_vec()
        }
    }

    /// Keep the selection and scroll offset inside the (shrinking) list.
    fn clamp(&mut self, g: &Game) {
        for side in 0..2 {
            let n = self.side_items(g, side).len() as i32;
            self.sel[side] = self.sel[side].min(n - 1).max(0);
            let max_off = (n - MAX_ROWS).max(0);
            self.off[side] = self.off[side].min(max_off).max(0);
        }
    }

    fn scroll(&mut self) {
        let side = self.side;
        if self.sel[side] < self.off[side] {
            self.off[side] = self.sel[side];
        }
        if self.sel[side] >= self.off[side] + MAX_ROWS {
            self.off[side] = self.sel[side] - MAX_ROWS + 1;
        }
    }

    /// Move the selected item to the other side: the whole stack on ENTER, one
    /// unit on Q. Same rules as the classic container: non-stackables always move
    /// whole; in creative, whole-stack moves out of the pack leave the original
    /// (the keep-a-copy duplication rule).
    fn transfer(&mut self, g: &mut Game, whole_stack: bool) {
        let from_side = self.side;
        let sel = self.sel[from_side];
        let creative = g.is_mode("creative");

        // Take the chest out so both inventories can be borrowed (see PORTING.md).
        let Some(mut chest) = g.entities.take(self.chest_eid) else {
            return;
        };
        let mut moved_ok = false;
        if let Some(player) = g.entities.get_mut(self.player_eid) {
            let chest_inv = &mut chest
                .chest_mut()
                .expect("container must be a chest")
                .inventory;
            let player_inv = &mut player.player_mut().inventory;
            let (from, to, from_is_player) = if from_side == SIDE_CONTAINER {
                (chest_inv, player_inv, false)
            } else {
                (player_inv, chest_inv, true)
            };

            if sel < from.inv_size() {
                let from_item = from.get_mut(sel);
                let move_all = whole_stack || !from_item.is_stackable() || from_item.count() == 1;
                let mut moved = from_item.clone();
                if !move_all {
                    from_item.set_count(from_item.count() - 1);
                    moved.set_count(1);
                } else if !(creative && from_is_player) {
                    from.remove(sel);
                }
                to.add_at(self.sel[1 - from_side], moved);
                moved_ok = true;
            }
        }
        g.entities.put_back(chest);
        if moved_ok {
            g.play_sound(Sound::Select);
            self.clamp(g);
        }
    }

    fn render_side(&self, screen: &mut Screen, g: &Game, side: usize) {
        let (x0, x1) = if side == SIDE_CONTAINER {
            (LIST_X, L_RIGHT)
        } else {
            (R_LEFT, R_RIGHT)
        };
        let items = self.side_items(g, side);

        if items.is_empty() {
            let msg = if side == SIDE_CONTAINER {
                "NOTHING LEFT."
            } else {
                "PACK IS EMPTY."
            };
            let x = x0 + (x1 - x0 - font::text_width(msg)) / 2;
            font::draw(msg, screen, x, LIST_Y + 4, color::DARK_GRAY);
            return;
        }

        let focused = side == self.side;
        let off = self.off[side] as usize;
        let end = (off + MAX_ROWS as usize).min(items.len());
        for (slot, idx) in (off..end).enumerate() {
            let item = &items[idx];
            let y = LIST_Y + slot as i32 * ROW_H;
            let selected = focused && idx as i32 == self.sel[side];
            if selected {
                font::draw(">", screen, x0, y, color::YELLOW);
            }
            item.sprite.render(screen, x0 + 6, y);
            let col = if selected {
                color::WHITE
            } else if focused {
                color::GRAY
            } else {
                color::DARK_GRAY
            };
            // name clips before the count column — both columns are narrow, so
            // long names ("Prospector's Pan") ellipsize instead of colliding
            let mut name_w = x1 - (x0 + 15);
            if item.count() > 1 {
                let count = item.count().min(999).to_string();
                let cx = x1 - font::text_width(&count);
                font::draw(&count, screen, cx, y, col);
                name_w = cx - 4 - (x0 + 15);
            }
            font::draw_fit(&bare_name(g, item), screen, x0 + 15, y, col, name_w);
        }

        // 1px scrollbar on the pane's divider edge when the list overflows
        let rows = items.len() as i32;
        if rows > MAX_ROWS {
            let body_h = BODY_BOTTOM - LIST_Y;
            let bar_h = (body_h * MAX_ROWS / rows).max(8);
            let bar_y = LIST_Y + body_h * self.off[side] / rows;
            let bar_x = if side == SIDE_CONTAINER {
                MID_X - 1
            } else {
                MID_X + 1
            };
            fill_rect(screen, bar_x, bar_y, 1, bar_h, SCROLLBAR_RGB);
        }
    }
}

impl Display for ContainerDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn tick(&mut self, g: &mut Game) {
        // ESC, X, and E all return to the world; a broken/taken container closes too
        let chest_gone = g
            .entities
            .get(self.chest_eid)
            .map(|c| c.c.removed)
            .unwrap_or(true);
        if chest_gone {
            g.clear_menu();
            return;
        }
        if g.input.get_key("exit").clicked
            || g.input.get_key("menu").clicked
            || g.input.get_key("inventory").clicked
        {
            g.exit_menu();
            return;
        }

        // LEFT/RIGHT switch side (fixed panes — nothing shifts, fixes J10)
        if g.input.get_key("left").clicked || g.input.get_key("right").clicked {
            self.side = 1 - self.side;
            g.play_sound(Sound::Select);
            return;
        }

        self.clamp(g);
        let n = self.side_items(g, self.side).len() as i32;
        if n > 0 {
            let mut next = self.sel[self.side];
            if g.input.get_key("up").clicked {
                next -= 1;
            }
            if g.input.get_key("down").clicked {
                next += 1;
            }
            if next != self.sel[self.side] {
                self.sel[self.side] = next.rem_euclid(n);
                self.scroll();
                g.play_sound(Sound::Select);
            }

            if g.input.get_key("select").clicked || g.input.get_key("attack").clicked {
                self.transfer(g, true);
            } else if g.input.get_key("drop-one").clicked {
                self.transfer(g, false);
            }
        }
    }

    fn render(&mut self, screen: &mut Screen, g: &mut Game) {
        // deepen the frame's default 185 glass to the shell's 200 spec
        screen.darken_rect_screen(PANEL_X, PANEL_Y, PANEL_W, PANEL_H, 55);
        self.shell_menu.render(screen, g);

        // the two titles, centered over their panes; the focused one carries the
        // gold underline (the container variant's stand-in for the tab strip)
        let titles = [self.title.to_uppercase(), "PACK".to_string()];
        for (side, title) in titles.iter().enumerate() {
            let center = if side == SIDE_CONTAINER {
                (PANEL_X + MID_X) / 2
            } else {
                (MID_X + PANEL_X + PANEL_W) / 2
            };
            let w = font::text_width(title);
            let x = center - w / 2;
            let active = side == self.side;
            let col = if active {
                color::WHITE
            } else {
                color::DARK_GRAY
            };
            font::draw(title, screen, x, TAB_Y, col);
            if active {
                fill_rect(screen, x - 1, TAB_Y + 9, w + 2, 1, GOLD_RGB);
            }
        }

        // the rule under the titles and the center divider
        fill_rect(
            screen,
            LIST_X,
            RULE_Y,
            DETAIL_RIGHT - LIST_X,
            1,
            DIVIDER_RGB,
        );
        fill_rect(
            screen,
            MID_X,
            RULE_Y,
            1,
            BODY_BOTTOM - RULE_Y + 8,
            DIVIDER_RGB,
        );

        self.render_side(screen, g, SIDE_CONTAINER);
        self.render_side(screen, g, SIDE_PACK);

        font::draw_centered(
            "ENTER MOVE   Q ONE   < > SIDE   ESC",
            screen,
            LEGEND_Y,
            color::GRAY,
        );
    }
}
