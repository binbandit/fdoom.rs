//! Port of `fdoom.screen.TitleDisplay`.

use crate::core::game::{self, Game};
use crate::gfx::{Point, Screen, color, font};

use super::book_display::BookDisplay;
use super::display::{Display, DisplayBase, display_render_default};
use super::entry::{EntryHandle, SelectEntry, handle};
use super::menu::MenuBuilder;
use super::options_display::OptionsDisplay;
use super::rel_pos::RelPos;
use super::world_gen_display::WorldGenDisplay;
use super::world_select::WorldSelectDisplay;

pub struct TitleDisplay {
    base: DisplayBase,
}

impl TitleDisplay {
    pub fn new(g: &Game) -> TitleDisplay {
        // Modern flat flow: Continue (most recent world) / New World / Load World —
        // no intermediate Play submenu (user request).
        let has_worlds = !crate::screen::world_select::get_world_names(g).is_empty();
        let mut entries: Vec<EntryHandle> = Vec::new();
        if let Some(recent) = crate::screen::world_select::most_recent_world(g) {
            let label = format!("Continue ({recent})");
            entries.push(handle(SelectEntry::new(&label, move |g: &mut Game| {
                let name = crate::screen::world_select::most_recent_world(g)
                    .expect("recent world existed when the title was built");
                crate::screen::world_select::set_world_name(g, &name, true);
                g.set_menu(crate::screen::loading_display::LoadingDisplay::new());
            })));
        }
        entries.push(handle(SelectEntry::new("New World", |g: &mut Game| {
            g.set_menu(WorldGenDisplay::new(g));
        })));
        if has_worlds {
            entries.push(handle(SelectEntry::new("Load World", |g: &mut Game| {
                g.set_menu(WorldSelectDisplay::new());
            })));
        }
        entries.extend([
            handle(SelectEntry::new("Options", |g: &mut Game| {
                g.set_menu(OptionsDisplay::new(g));
            })),
            // straight to the instructions book: the old Help submenu was one
            // empty black page holding a single entry (found playing)
            handle(SelectEntry::new("Help", |g: &mut Game| {
                g.set_menu(BookDisplay::new(g, super::book_data::INSTRUCTIONS));
            })),
            handle(SelectEntry::new("Quit", |g: &mut Game| g.quit())),
        ]);

        let menu = MenuBuilder::new(false, 2, RelPos::Center, entries)
            .set_positioning(
                Point::new(g.screen_size.0 / 2, g.screen_size.1 * 3 / 5),
                RelPos::Center,
            )
            .create_menu(g);

        TitleDisplay {
            // clear_screen=false: the renderer draws the drone-flyover world behind us
            base: DisplayBase::new(false, false, vec![menu]),
        }
    }
}

impl Display for TitleDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn init(&mut self, g: &mut Game) {
        // the title screen never has a parent display
        g.display.stack.clear();
        g.ready_to_render_gameplay = false;

        // drop any loaded world
        for level in g.levels.iter_mut() {
            *level = None;
        }
    }

    fn render(&mut self, screen: &mut Screen, g: &mut Game) {
        display_render_default(&mut self.base, screen, g);

        // Full sheet-art title lockup (artgen `logo`): the "FOSSICKERS" kicker strip
        // at cells (15..31,6..7) over the "DOOM" strip at cells (0..14,6..7). Both are
        // true-color art, so the palette word is ignored.
        let title_color = color::get4(-1, 0, color::hex("#2c2c2c"), color::hex("#ff0000"));
        let kicker_w = 17; // strip width in sheet cells
        let doom_w = 15;
        let kicker_x = (screen.w - kicker_w * 8) / 2;
        let kicker_y = 14;
        let doom_x = (screen.w - doom_w * 8) / 2;
        let doom_y = kicker_y + 18; // kicker art is 15px tall + 3px gap

        for y in 0..2 {
            for x in 0..kicker_w {
                screen.render(
                    kicker_x + x * 8,
                    kicker_y + y * 8,
                    15 + x + (y + 6) * 32,
                    title_color,
                    0,
                );
            }
            for x in 0..doom_w {
                screen.render(
                    doom_x + x * 8,
                    doom_y + y * 8,
                    x + (y + 6) * 32,
                    title_color,
                    0,
                );
            }
        }

        font::draw(
            &format!("Version {}", game::version()),
            screen,
            1,
            1,
            color::get(-1, 111),
        );

        // Navigation hints — title screen only (in-game menus skip them), and dimmed
        // past the palette floor with a post-draw multiplicative darken.
        let hints = [
            format!(
                "({}, {}{})",
                g.input.get_mapping("up"),
                g.input.get_mapping("down"),
                g.localization.get_localized(" to select")
            ),
            format!(
                "({}{})",
                g.input.get_mapping("select"),
                g.localization.get_localized(" to accept")
            ),
            format!(
                "({}{})",
                g.input.get_mapping("exit"),
                g.localization.get_localized(" to return")
            ),
        ];
        for (i, hint) in hints.iter().enumerate() {
            let y = screen.h - 32 + i as i32 * 10;
            font::draw_centered(hint, screen, y, color::get(-1, 111));
        }
        // one multiplicative darken over the whole hint block: drops the text below the
        // palette's dimmest gray without leaving per-line boxes
        let w = hints.iter().map(|h| font::text_width(h)).max().unwrap_or(0) + 8;
        screen.darken_rect_screen((screen.w - w) / 2, screen.h - 35, w, 32, 110);
    }
}
