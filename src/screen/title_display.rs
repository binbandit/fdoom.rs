//! Port of `fdoom.screen.TitleDisplay`.

use crate::core::game::{self, Game};
use crate::gfx::{Point, Screen, color, font};
use crate::rng::Rng;

use super::book_display::BookDisplay;
use super::display::{Display, DisplayBase, display_render_default, display_tick_default};
use super::entry::{BlankEntry, EntryHandle, SelectEntry, StringEntry, handle};
use super::menu::MenuBuilder;
use super::multiplayer_display::MultiplayerDisplay;
use super::options_display::OptionsDisplay;
use super::rel_pos::RelPos;
use super::world_gen_display::WorldGenDisplay;
use super::world_select::WorldSelectDisplay;

pub struct TitleDisplay {
    base: DisplayBase,
    #[allow(dead_code)]
    rand: i32,
    count: i32, // this and reverse are for the logo; they produce the fade-in/out effect
    reverse: bool,
    random: Rng,
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
        let entries: Vec<EntryHandle> = vec![
            handle(StringEntry::with_color(
                "Checking for updates...",
                color::BLUE,
            )),
            handle(BlankEntry::new()),
            handle(BlankEntry::new()),
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
            handle(SelectEntry::new("Join Online World", |g: &mut Game| {
                g.set_menu(MultiplayerDisplay::new());
            })),
            handle(SelectEntry::new("Options", |g: &mut Game| {
                g.set_menu(OptionsDisplay::new(g));
            })),
            display_factory(
                "Help",
                vec![
                    handle(SelectEntry::new("Instructions", |g: &mut Game| {
                        g.set_menu(BookDisplay::new(g, super::book_data::INSTRUCTIONS));
                    })),
                    handle(BlankEntry::new()),
                    handle(BlankEntry::new()),
                ],
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
            base: DisplayBase::new(true, false, vec![menu]),
            rand: 0,
            count: 0,
            reverse: false,
            random: Rng::from_time(),
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

        // JAVA: checkVersion() — the update check is disabled upstream (findLatestVersion
        // is commented out), so the "Checking for updates..." entry stays as-is.

        self.rand = self.random.next_int_bound(SPLASHES.len() as i32);

        // JAVA: World.levels = new Level[World.levels.length];
        for level in g.levels.iter_mut() {
            *level = None;
        }

        // JAVA: World.resetGame(false) only ran when the player was null or a
        // RemotePlayer (after online play); the singleplayer player always exists here.
    }

    fn tick(&mut self, g: &mut Game) {
        if g.input.get_key("r").clicked {
            self.rand = self.random.next_int_bound(SPLASHES.len() as i32);
        }

        if !self.reverse {
            self.count += 1;
            if self.count == 25 {
                self.reverse = true;
            }
        } else {
            self.count -= 1;
            if self.count == 0 {
                self.reverse = false;
            }
        }

        display_tick_default(&mut self.base, g);
    }

    fn render(&mut self, screen: &mut Screen, g: &mut Game) {
        display_render_default(&mut self.base, screen, g);

        let h = 2; // Height of squares (on the spritesheet)
        let w = 14; // Width of squares (on the spritesheet)
        let title_color = color::get4(-1, 0, color::hex("#2c2c2c"), color::hex("#ff0000"));
        let xo = (crate::gfx::screen::W - w * 8) / 2; // X location of the title
        let yo = 28; // Y location of the title

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

        let up_string = format!(
            "({}, {}{})",
            g.input.get_mapping("up"),
            g.input.get_mapping("down"),
            g.localization.get_localized(" to select")
        );
        let select_string = format!(
            "({}{})",
            g.input.get_mapping("select"),
            g.localization.get_localized(" to accept")
        );
        let exit_string = format!(
            "({}{})",
            g.input.get_mapping("exit"),
            g.localization.get_localized(" to return")
        );

        font::draw_centered(
            &up_string,
            screen,
            crate::gfx::screen::H - 32,
            color::get(-1, 111),
        );
        font::draw_centered(
            &select_string,
            screen,
            crate::gfx::screen::H - 22,
            color::get(-1, 111),
        );
        font::draw_centered(
            &exit_string,
            screen,
            crate::gfx::screen::H - 12,
            color::get(-1, 111),
        );
    }
}

#[allow(dead_code)]
static SPLASHES: &[&str] = &[
    "Multiplayer Now Included!",
    "Only on PlayMinicraft.com!",
    "Playminicraft.com is the bomb!",
    "MinicraftPlus on Youtube",
    "The Wiki is weak! Help it!",
    "Notch is Awesome!",
    "Dillyg10 is cool as Ice!",
    "Shylor is the man!",
    "AntVenom loves cows! Honest!",
    "You should read Antidious Venomi!",
    "Oh Hi Mark",
    "Use the force!",
    "Keep calm!",
    "Get him, Steve!",
    "Forty-Two!",
    "Kill Creeper, get Gunpowder!",
    "Kill Cow, get Beef!",
    "Kill Zombie, get Cloth!",
    "Kill Slime, get Slime!",
    "Kill Skeleton, get Bones!",
    "Kill Sheep, get Wool!",
    "Kill Pig, get Porkchop!",
    "Gold > Iron",
    "Gem > Gold",
    "Test == InDev!",
    "Story? Uhh...",
    "Infinite terrain? What's that?",
    "Redstone? What's that?",
    "Minecarts? What are those?",
    "Windows? I prefer Doors!",
    "2.5D FTW!",
    "3rd dimension not included!",
    "Mouse not included!",
    "No spiders included!",
    "No Endermen included!",
    "No chickens included!",
    "Grab your friends!",
    "Creepers included!",
    "Skeletons included!",
    "Knights included!",
    "Snakes included!",
    "Cows included!",
    "Sheep included!",
    "Pigs included!",
    "Bigger Worlds!",
    "World types!",
    "World themes!",
    "Sugarcane is a Idea!",
    "Milk is an idea!",
    "So we back in the mine,",
    "pickaxe swinging from side to side",
    "Life itself suspended by a thread",
    "In search of Gems!",
    "saying ay-oh, that creeper's KO'd!",
    "Gimmie a bucket!",
    "Farming with water!",
    "Press \"R\"!",
    "Get the High-Score!",
    "Potions ftw!",
    "Beds ftw!",
    "Defeat the Air Wizard!",
    "Conquer the Dungeon!",
    "One down, one to go...",
    "Loom + Wool = String!",
    "String + Wood = Rod!",
    "Sand + Gunpowder = TNT!",
    "Sleep at Night!",
    "Farm at Day!",
    "Explanation Mark!",
    "!sdrawkcab si sihT",
    "This is forwards!",
    "Why is this blue?",
    "Green is a nice color!",
    "Red is my favorite color!",
    "Y U NO BOAT!?",
    "Made with 10000% Vitamin Z!",
    "Too much DP!",
    "Punch the Moon!",
    "This is String qq!",
    "Why?",
    "You are null!",
    "hello down there!",
    "That guy is such a sly fox!",
    "Hola senor!",
    "Sonic Boom!",
    "Hakuna Matata!",
    "One truth prevails!",
    "Awesome!",
    "Sweet!",
    "Cool!",
    "Radical!",
    "011011000110111101101100!",
    "001100010011000000110001!",
    "011010000110110101101101?",
    "...zzz...",
];
