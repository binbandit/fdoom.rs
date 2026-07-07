//! Port of `fdoom.screen.WorldGenDisplay` — the "World Gen Options" screen.

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::game::Game;
use crate::gfx::{Screen, color, font};

use super::display::{Display, DisplayBase};
use super::entry::input_entry::{self, InputEntry, Validation};
use super::entry::{
    BlankEntry, EntryFlags, EntryHandle, ListEntry, SelectEntry, StringEntry, handle,
};
use super::loading_display::LoadingDisplay;
use super::menu::MenuBuilder;
use super::rel_pos::RelPos;

thread_local! {
    /// The current text of the seed input — Java's static `worldSeed` InputEntry
    /// (kept live by a change listener on the entry in the menu).
    static WORLD_SEED: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Java `WorldGenDisplay.getSeed()`.
pub fn get_seed(g: &mut Game) -> i64 {
    let seed_str = WORLD_SEED.with(|s| s.borrow().clone());
    if seed_str.is_empty() {
        // JAVA: new Random().nextLong() — incidental randomness, so g.random (PORTING.md).
        g.random.next_long()
    } else {
        // JAVA: Long.parseLong(seedStr) threw NumberFormatException past 19 digits; we
        // fall back to 0 instead of crashing.
        seed_str.parse().unwrap_or(0)
    }
}

/// Java `WorldGenDisplay.makeWorldNameInput(prompt, takenNames, initValue)`.
pub fn make_world_name_input(
    prompt: &str,
    taken_names: Vec<String>,
    init_value: &str,
) -> InputEntry {
    let mut entry =
        InputEntry::with_init(prompt, Some(input_entry::world_name_char), 36, init_value);
    entry.set_validation(Validation::UniqueName(taken_names));
    entry
}

/// Java's anonymous `SelectEntry` override: "Create World" always renders cyan.
struct CreateWorldEntry {
    inner: SelectEntry,
}

impl ListEntry for CreateWorldEntry {
    fn flags(&self) -> EntryFlags {
        self.inner.flags()
    }

    fn flags_mut(&mut self) -> &mut EntryFlags {
        self.inner.flags_mut()
    }

    fn tick(&mut self, g: &mut Game) {
        self.inner.tick(g);
    }

    fn to_display_string(&self, g: &Game) -> String {
        self.inner.to_display_string(g)
    }

    fn render(&mut self, screen: &mut Screen, g: &mut Game, x: i32, y: i32, _is_selected: bool) {
        let text = self.to_display_string(g);
        font::draw(&text, screen, x, y, color::CYAN);
    }
}

pub struct WorldGenDisplay {
    base: DisplayBase,
}

impl WorldGenDisplay {
    pub fn new(g: &Game) -> WorldGenDisplay {
        let name_field: Rc<RefCell<InputEntry>> = Rc::new(RefCell::new(make_world_name_input(
            "Enter World Name",
            super::world_select::get_world_names(g),
            "",
        )));

        // (The Java "Trouble with world name?" help entry is gone: text rows now capture
        // typing, and menu navigation while typing uses the physical arrow keys.)

        // JAVA: worldSeed = new InputEntry("World Seed", "[0-9]+", 20) { isValid() → true }
        WORLD_SEED.with(|s| s.borrow_mut().clear());
        let mut world_seed = InputEntry::new("World Seed", Some(input_entry::digit_char), 20);
        world_seed.set_validation(Validation::Always);
        // Mirrors the Java static: getSeed() reads whatever is typed here.
        world_seed.set_change_listener(Box::new(|text: &str| {
            WORLD_SEED.with(|s| *s.borrow_mut() = text.to_string());
        }));

        let create_world = {
            let name_field = name_field.clone();
            CreateWorldEntry {
                inner: SelectEntry::new("Create World", move |g: &mut Game| {
                    if !name_field.borrow().is_valid() {
                        return;
                    }
                    let name = name_field.borrow().get_user_input();
                    super::world_select::set_world_name(g, &name, false);
                    // Java's LevelGen read `WorldGenDisplay.getSeed()` at generation
                    // time; the Rust world gen reads `g.world_seed`, so capture it here.
                    g.world_seed = get_seed(g);
                    g.set_menu(LoadingDisplay::new());
                }),
            }
        };

        // Worlds are always infinite (user direction): a world is fully described by
        // its name and seed. The screen floats over the title flyover like the main
        // menu, centered, with breathing room and a hint about random seeds.
        let entries: Vec<EntryHandle> = vec![
            name_field,
            handle(BlankEntry::new()),
            handle(world_seed),
            handle(StringEntry::with_color(
                "(leave empty for a random seed)",
                color::get(-1, 222),
            )),
            handle(BlankEntry::new()),
            handle(create_world),
        ];

        let menu = MenuBuilder::new(false, 4, RelPos::Center, entries)
            .set_title("NEW WORLD")
            .create_menu(g);

        WorldGenDisplay {
            base: DisplayBase::new(false, true, vec![menu]),
        }
    }
}

impl Display for WorldGenDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    // JAVA: init forces the parent to a TitleDisplay when not opened from one; with the
    // explicit display stack, exiting simply returns to whatever opened this screen.
}
