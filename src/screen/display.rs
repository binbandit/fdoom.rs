//! Port of `fdoom.screen.Display` (base class → trait + `DisplayBase` struct) and the
//! `Game.setMenu`/`newMenu`/`menu` transition mechanic (→ `DisplayManager`).
//!
//! Java linked displays through `parent` references; every transition either entered a
//! child (`setMenu(new X())`, parent set to the old menu), returned to the parent
//! (`exitMenu()`), or cleared everything (`setMenu(null)`). That chain is this stack.

use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::gfx::Screen;

use super::menu::Menu;

pub struct DisplayBase {
    pub menus: Vec<Menu>,
    pub selection: i32,
    pub can_exit: bool,
    pub clear_screen: bool,
}

impl DisplayBase {
    /// Java `new Display(clearScreen, canExit, menus...)`.
    pub fn new(clear_screen: bool, can_exit: bool, menus: Vec<Menu>) -> DisplayBase {
        DisplayBase {
            menus,
            selection: 0,
            can_exit,
            clear_screen,
        }
    }
}

impl Default for DisplayBase {
    /// Java `new Display()` — no menus, no clear, can't exit... actually Java defaults to
    /// clearScreen=false, canExit=true.
    fn default() -> Self {
        DisplayBase::new(false, true, Vec::new())
    }
}

pub trait Display {
    fn base(&self) -> &DisplayBase;
    fn base_mut(&mut self) -> &mut DisplayBase;

    /// Java `init(parent)` — parent management lives in the manager; this is the hook.
    fn init(&mut self, g: &mut Game) {
        let _ = g;
    }

    /// Java `onExit()`.
    fn on_exit(&mut self, g: &mut Game) {
        let _ = g;
    }

    /// Java `Display.tick(input)`.
    fn tick(&mut self, g: &mut Game) {
        display_tick_default(self.base_mut(), g);
    }

    /// Java `Display.render(screen)`.
    fn render(&mut self, screen: &mut Screen, g: &mut Game) {
        display_render_default(self.base_mut(), screen, g);
    }
}

/// The default `Display.tick` implementation, callable by overriding impls.
pub fn display_tick_default(base: &mut DisplayBase, g: &mut Game) {
    if base.can_exit && g.input.get_key("exit").clicked {
        g.exit_menu();
        return;
    }

    if base.menus.is_empty() {
        return;
    }

    let mut changed_selection = false;

    if base.menus.len() > 1 && base.menus[base.selection as usize].is_selectable() {
        // if menu set is unselectable, it must have been intentional, so prevent the user
        // from setting it back
        let prev_sel = base.selection;
        let mut selection = base.selection;

        let shift = base.menus[selection as usize]
            .get_cur_entry()
            .map(|e| e.borrow().is_array_entry())
            .unwrap_or(false);
        let prefix = if shift { "shift-" } else { "" };
        if g.input.get_key(&format!("{prefix}left")).clicked {
            selection -= 1;
        }
        if g.input.get_key(&format!("{prefix}right")).clicked {
            selection += 1;
        }

        if prev_sel != selection {
            g.play_sound(Sound::Select);

            let delta = selection - prev_sel;
            let len = base.menus.len() as i32;
            selection = prev_sel;
            loop {
                selection += delta;
                if selection < 0 {
                    selection = len - 1;
                }
                selection %= len;
                if base.menus[selection as usize].is_selectable() || selection == prev_sel {
                    break;
                }
            }

            changed_selection = prev_sel != selection;
            if changed_selection {
                base.selection = selection; // Java onSelectionChange default
            }
        }
    }

    if !changed_selection {
        let sel = base.selection as usize;
        base.menus[sel].tick(g);
    }
}

/// The default `Display.render` implementation: renders each menu, selected one last/on top.
pub fn display_render_default(base: &mut DisplayBase, screen: &mut Screen, g: &mut Game) {
    if base.clear_screen {
        screen.clear(0);
    }

    if base.menus.is_empty() {
        return;
    }

    let len = base.menus.len();
    let mut idx = base.selection as usize;
    loop {
        idx = (idx + 1) % len;
        if base.menus[idx].should_render() {
            base.menus[idx].render(screen, g);
        }
        if idx == base.selection as usize {
            break;
        }
    }
}

/// Pending menu change (Java's `newMenu` slot; the last set before a tick wins).
pub enum PendingMenu {
    NoChange,
    /// `setMenu(display)` — enter a new display whose parent is the current one.
    Set(Box<dyn Display>),
    /// `setMenu(null)` — clear all menus (gameplay).
    Clear,
    /// `exitMenu()` — return to the parent display.
    Exit,
}

/// The current-display stack, replacing Java's `menu`/`newMenu` statics and the `parent`
/// chain. Only the top display ticks and renders (as in Java).
pub struct DisplayManager {
    pub stack: Vec<Box<dyn Display>>,
    pub pending: PendingMenu,
}

impl Default for DisplayManager {
    fn default() -> Self {
        DisplayManager {
            stack: Vec::new(),
            pending: PendingMenu::NoChange,
        }
    }
}

impl DisplayManager {
    /// Whether a menu is (or is about to be) open — Java `getMenu() != null`, which read
    /// the *pending* `newMenu`.
    pub fn menu_open(&self) -> bool {
        match &self.pending {
            PendingMenu::NoChange => !self.stack.is_empty(),
            PendingMenu::Set(_) => true,
            PendingMenu::Clear => false,
            PendingMenu::Exit => self.stack.len() > 1,
        }
    }

    /// Whether a menu is open right now (Java `menu != null` inside Updater.tick).
    pub fn menu_active(&self) -> bool {
        !self.stack.is_empty()
    }
}
