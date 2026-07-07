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

/// Java `displayFactory(entryText, entries...)` — a submenu-in-a-plain-Display button.
/// The entry handles are shared (cloned Rc's), matching Java reusing the same objects.
fn display_factory(entry_text: &str, entries: Vec<EntryHandle>) -> EntryHandle {
    handle(SelectEntry::new(entry_text, move |g: &mut Game| {
        let menu = MenuBuilder::new(false, 2, RelPos::Center, entries.clone()).create_menu(g);
        g.set_menu(super::plain_display(true, true, vec![menu]));
    }))
}

impl TitleDisplay {
    pub fn new(g: &Game) -> TitleDisplay {
        // (Post-port cleanup: the dead "Checking for updates..." line and the stubbed
        // "Join Online World" entry are gone — multiplayer was never wired up.)
        let entries: Vec<EntryHandle> = vec![
            handle(SelectEntry::new("Play", |g: &mut Game| {
                if !crate::screen::world_select::get_world_names(g).is_empty() {
                    let menu = MenuBuilder::new(
                        false,
                        2,
                        RelPos::Center,
                        vec![
                            handle(SelectEntry::new("Load World", |g: &mut Game| {
                                g.set_menu(WorldSelectDisplay::new());
                            })),
                            handle(SelectEntry::new("New World", |g: &mut Game| {
                                g.set_menu(WorldGenDisplay::new(g));
                            })),
                        ],
                    )
                    .create_menu(g);
                    g.set_menu(super::plain_display(true, true, vec![menu]));
                } else {
                    g.set_menu(WorldGenDisplay::new(g));
                }
            })),
            handle(SelectEntry::new("Options", |g: &mut Game| {
                g.set_menu(OptionsDisplay::new(g));
            })),
            display_factory(
                "Help",
                vec![handle(SelectEntry::new("Instructions", |g: &mut Game| {
                    g.set_menu(BookDisplay::new(g, super::book_data::INSTRUCTIONS));
                }))],
            ),
            handle(SelectEntry::new("Quit", |g: &mut Game| g.quit())),
        ];

        let menu = MenuBuilder::new(false, 2, RelPos::Center, entries)
            .set_positioning(
                Point::new(crate::gfx::screen::W / 2, crate::gfx::screen::H * 3 / 5),
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
        // JAVA: super.init(null) — the TitleScreen never has a parent.
        g.display.stack.clear();
        g.ready_to_render_gameplay = false;

        // (Post-port cleanup: the Java splash-text list, its "r" reroll, and the unused
        // logo fade counters are gone — none of them were ever rendered in this fork.)

        // JAVA: World.levels = new Level[World.levels.length];
        for level in g.levels.iter_mut() {
            *level = None;
        }

        // JAVA: World.resetGame(false) only ran when the player was null or a
        // RemotePlayer (after online play); the singleplayer player always exists here.
    }

    fn render(&mut self, screen: &mut Screen, g: &mut Game) {
        display_render_default(&mut self.base, screen, g);

        let h = 2; // Height of squares (on the spritesheet)
        let w = 14; // Width of squares (on the spritesheet)
        let title_color = color::get4(-1, 0, color::hex("#2c2c2c"), color::hex("#ff0000"));
        let xo = (crate::gfx::screen::W - w * 8) / 2; // X location of the title
        let yo = 22; // Y location of the title

        font::draw_centered(
            "* F O S S I C K E R S *",
            screen,
            yo - 8,
            color::get(-1, 511),
        );
        for y in 0..h {
            for x in 0..w {
                screen.render(xo + x * 8, yo + y * 8, x + (y + 6) * 32, title_color, 0);
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
            let y = crate::gfx::screen::H - 32 + i as i32 * 10;
            font::draw_centered(hint, screen, y, color::get(-1, 111));
        }
        // one multiplicative darken over the whole hint block: drops the text below the
        // palette's dimmest gray without leaving per-line boxes
        let w = hints.iter().map(|h| font::text_width(h)).max().unwrap_or(0) + 8;
        screen.darken_rect_screen(
            (crate::gfx::screen::W - w) / 2,
            crate::gfx::screen::H - 35,
            w,
            32,
            110,
        );
    }
}
