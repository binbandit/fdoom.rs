//! Port of `fdoom.screen.EndGameDisplay` — the score-mode game-over summary.

use crate::core::game::Game;
use crate::core::updater::NORM_SPEED;
use crate::gfx::{Screen, color};
use crate::screen::entry::array_entry::Value;

use super::display::{Display, DisplayBase, display_render_default, display_tick_default};
use super::entry::{EntryHandle, SelectEntry, StringEntry, handle};
use super::menu::MenuBuilder;
use super::rel_pos::RelPos;
use super::title_display::TitleDisplay;

const SCORED_ITEMS: &[&str] = &["Cloth", "Slime", "Bone", "Arrow", "Gunpowder", "Antidious"];

fn max_len() -> usize {
    SCORED_ITEMS.iter().map(|s| s.len()).max().unwrap_or(0)
}

/// The `Game.setMenu(new EndGameDisplay(player))` call site in `Updater.tick`.
pub fn open(g: &mut Game) {
    let display = EndGameDisplay::new(g);
    g.set_menu(display);
}

pub struct EndGameDisplay {
    base: DisplayBase,
    /// variable to delay the input of the player, so they won't skip the won menu by accident.
    input_delay: i32,
    display_timer: i32,
}

impl EndGameDisplay {
    pub fn new(g: &mut Game) -> EndGameDisplay {
        let display_timer = NORM_SPEED; // wait 3 seconds before rendering the menu.
        let input_delay = NORM_SPEED / 2; // wait a half-second after rendering before allowing user input.

        let mut entries: Vec<EntryHandle> = Vec::new();

        // calculate the score
        let score = g.player().player().get_score();
        entries.push(handle(StringEntry::with_color(
            &format!("Player Score: {score}"),
            color::WHITE,
        )));
        entries.push(handle(StringEntry::with_color("<Bonuses>", color::YELLOW)));

        let mut finalscore = score;
        for item in SCORED_ITEMS {
            // JAVA: addBonus returns the entry but the result is never added to the menu;
            // only the final score accumulates.
            let _ = add_bonus(g, &mut finalscore, item);
        }

        entries.push(handle(StringEntry::new(&format!(
            "Final Score: {finalscore}"
        ))));

        // add any unlocks
        entries.extend(get_and_write_unlocks(g, finalscore));

        entries.push(handle(SelectEntry::new("Exit to Menu", |g: &mut Game| {
            g.set_menu(TitleDisplay::new(g));
        })));

        let menu = MenuBuilder::new(true, 0, RelPos::Left, entries).create_menu(g);

        EndGameDisplay {
            base: DisplayBase::new(false, false, vec![menu]),
            input_delay,
            display_timer,
        }
    }
}

/// Java `addBonus(item)`.
fn add_bonus(g: &mut Game, finalscore: &mut i32, item: &str) -> EntryHandle {
    let proto = crate::item::registry::get(g, item);
    let count = g.player().player().inventory.count(&proto);
    let score = count * (g.random.next_int_bound(2) + 1) * 10;
    *finalscore += score;

    let mut buffer = String::new();
    while item.len() + buffer.len() < max_len() {
        buffer.push(' ');
    }
    handle(StringEntry::with_color(
        &format!("{count} {item}s: {buffer}+{score}"),
        color::YELLOW,
    ))
}

/// Java `getAndWriteUnlocks()`.
fn get_and_write_unlocks(g: &mut Game, finalscore: i32) -> Vec<EntryHandle> {
    let score_time = g.settings.get("scoretime").as_int();
    let mut unlocks: Vec<i32> = Vec::new();

    if score_time == 20
        && !g.settings.get("scoretime").matches(&Value::Int(10))
        && finalscore > 1000
    {
        unlocks.push(10);
        g.settings.unlock_scoretime(10);
    }

    if score_time == 60
        && !g.settings.get("scoretime").matches(&Value::Int(120))
        && finalscore > 100000
    {
        unlocks.push(120);
        g.settings.unlock_scoretime(120);
    }

    crate::saveload::save::save_prefs(g); // JAVA: new Save() — persists the unlocks

    unlocks
        .into_iter()
        .map(|u| handle(StringEntry::new(&format!("Unlocked! {u} Score Time"))))
        .collect()
}

impl Display for EndGameDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn tick(&mut self, g: &mut Game) {
        if self.display_timer > 0 {
            self.display_timer -= 1;
        } else if self.input_delay > 0 {
            self.input_delay -= 1;
        } else {
            display_tick_default(&mut self.base, g);
        }
    }

    fn render(&mut self, screen: &mut Screen, g: &mut Game) {
        if self.display_timer <= 0 {
            display_render_default(&mut self.base, screen, g);
        }
    }
}
