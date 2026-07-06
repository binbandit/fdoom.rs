//! Port of `fdoom.screen.KeyInputDisplay` — the "Controls" key-binding screen.

use crate::core::game::Game;
use crate::gfx::{Point, Screen, color, font};

use super::display::{Display, DisplayBase, display_render_default, display_tick_default};
use super::entry::key_input_entry::KeyInputEntry;
use super::entry::{EntryHandle, StringEntry, handle};
use super::menu::MenuBuilder;
use super::rel_pos::RelPos;

pub struct KeyInputDisplay {
    base: DisplayBase,
    listening_for_bind: bool,
    confirm_reset: bool,
}

fn get_entries(g: &Game) -> Vec<EntryHandle> {
    g.input
        .get_key_prefs(g.debug)
        .iter()
        .map(|pref| handle(KeyInputEntry::new(pref)))
        .collect()
}

/// The retained `builder` static in Java — rebuilt on demand since ours is consumed.
fn main_menu_builder(entries: Vec<EntryHandle>) -> MenuBuilder {
    MenuBuilder::new(false, 0, RelPos::Center, entries)
        .set_title("Controls")
        .set_positioning(
            Point::new(
                crate::gfx::screen::W / 2,
                crate::gfx::screen::H - font::text_height() * 5,
            ),
            RelPos::Top,
        )
}

fn popup_builder(entries: Vec<EntryHandle>) -> MenuBuilder {
    MenuBuilder::new(true, 4, RelPos::Center, entries)
        .set_should_render(false)
        .set_selectable(false)
}

impl KeyInputDisplay {
    pub fn new(g: &Game) -> KeyInputDisplay {
        let menus = vec![
            main_menu_builder(get_entries(g)).create_menu(g),
            popup_builder(StringEntry::use_lines_color(
                color::YELLOW,
                &["Press the desired".to_string(), "key sequence".to_string()],
            ))
            .create_menu(g),
            popup_builder(StringEntry::use_lines_color(
                color::RED,
                &[
                    "Are you sure you want to reset all key bindings to the default keys?"
                        .to_string(),
                    "enter to confirm".to_string(),
                    "escape to cancel".to_string(),
                ],
            ))
            .set_title("Confirm Action")
            .create_menu(g),
        ];

        KeyInputDisplay {
            base: DisplayBase::new(true, true, menus),
            listening_for_bind: false,
            confirm_reset: false,
        }
    }
}

impl Display for KeyInputDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn tick(&mut self, g: &mut Game) {
        if self.listening_for_bind {
            if g.input.key_to_change.is_none() {
                // the key has just been set
                self.listening_for_bind = false;
                self.base.menus[1].should_render = false;
                let changed = g.input.get_changed_key();
                self.base.menus[0].update_selected_entry(handle(KeyInputEntry::new(&changed)));
                self.base.selection = 0;
            }

            return;
        }

        if self.confirm_reset {
            if g.input.get_key("exit").clicked {
                self.confirm_reset = false;
                self.base.menus[2].should_render = false;
                self.base.selection = 0;
            } else if g.input.get_key("select").clicked {
                self.confirm_reset = false;
                g.input.reset_key_bindings();
                self.base.menus[2].should_render = false;
                let sel = self.base.menus[0].get_selection();
                let disp_sel = self.base.menus[0].get_disp_selection();
                self.base.menus[0] = main_menu_builder(get_entries(g))
                    .set_selection_disp(sel, disp_sel)
                    .create_menu(g);
                self.base.selection = 0;
            }

            return;
        }

        display_tick_default(&mut self.base, g); // ticks menu

        if g.input.key_to_change.is_some() {
            self.listening_for_bind = true;
            self.base.selection = 1;
            self.base.menus[self.base.selection as usize].should_render = true;
        } else if g.input.get_key("shift-d").clicked && !self.confirm_reset {
            self.confirm_reset = true;
            self.base.selection = 2;
            self.base.menus[self.base.selection as usize].should_render = true;
        }
    }

    fn render(&mut self, screen: &mut Screen, g: &mut Game) {
        if self.base.selection == 0 {
            // JAVA: "not necessary ... but it's probably more efficient anyway"
            screen.clear(0);
        }

        display_render_default(&mut self.base, screen, g);

        if !self.listening_for_bind && !self.confirm_reset {
            let lines = [
                "Press C/Enter to change key binding".to_string(),
                "Press A to add key binding".to_string(),
                "Shift-D to reset all keys to default".to_string(),
                format!("{} to Return to menu", g.input.get_mapping("exit")),
            ];
            for (i, line) in lines.iter().enumerate() {
                font::draw_centered(
                    line,
                    screen,
                    crate::gfx::screen::H - font::text_height() * (4 - i as i32),
                    color::WHITE,
                );
            }
        }
    }
}
