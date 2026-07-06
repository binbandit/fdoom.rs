//! Port of `fdoom.core.Game` + the mutable statics of `Updater`/`World`/`Renderer`.
//!
//! Java's global state becomes this one struct, threaded through the code as `g`.
//! See PORTING.md ("One `Game` struct instead of Java statics").

use std::path::PathBuf;
use std::rc::Rc;

use crate::core::io::input_handler::InputHandler;
use crate::entity::furniture::bed::BedState;
use crate::entity::EntityArena;
use crate::item::recipe::Recipes;
use crate::item::Item;
use crate::level::tile::Tiles;
use crate::level::Level;
use crate::core::io::localization::Localization;
use crate::core::io::settings::Settings;
use crate::core::io::sound::{Sound, SoundPlayer};
use crate::core::updater::{self, Time};
use crate::java_random::JavaRandom;
use crate::saveload::version::Version;
use crate::screen::display::{Display, DisplayManager, PendingMenu};

/// Java `Game.NAME` — the name on the application window.
pub const NAME: &str = "Fossickers Doom";

/// Java `Game.VERSION`.
pub fn version() -> Version {
    Version::new("2.6")
}

pub struct Game {
    // Java `Game` statics
    pub debug: bool,
    pub has_gui: bool,
    /// JAVA: `continous` (sic) — disables the focus nagger.
    pub continous: bool,
    pub input: InputHandler,
    pub game_dir: PathBuf,
    pub notifications: Vec<String>,
    pub max_fps: i32,
    pub display: DisplayManager,
    pub game_over: bool,
    pub running: bool,

    // io
    pub settings: Settings,
    pub sound: SoundPlayer,
    /// Cached "sound" setting, refreshed each tick (see sound.rs for why).
    pub sound_enabled: bool,
    pub localization: Localization,

    // Java `Updater` statics
    pub gamespeed: f32,
    pub paused: bool,
    pub tick_count: i32,
    time: i32,
    pub game_time: i32,
    pub past_day1: bool,
    pub score_time: i32,
    pub note_tick: i32,
    pub as_tick: i32,
    pub saving: bool,
    pub save_cooldown: i32,
    /// Java `Tile.tickCount` (a static on Tile, but game state).
    pub tile_tick_count: i32,

    // Java `World` statics + levels
    pub levels: Vec<Option<Level>>,
    /// Java `Tiles` static registry.
    pub tiles: Tiles,
    pub player_dead_time: i32,
    pub pending_level_change: i32,
    pub world_size: i32,
    pub current_level: usize,

    // Java `Renderer`/`Initializer` statics
    pub ready_to_render_gameplay: bool,
    pub show_info: bool,
    /// Whether the window has focus (Java polled `canvas.hasFocus()`).
    pub has_focus: bool,
    /// Frames/ticks in the previous second (Java `Initializer.fra`/`tik`).
    pub fra: i32,
    pub tik: i32,

    /// Shared incidental RNG (see PORTING.md "JavaRandom").
    pub random: JavaRandom,

    /// The item prototype registry (Java `Items`' static list).
    pub items: Rc<Vec<Item>>,
    /// Java `Recipes`' static lists.
    pub recipes: Rc<Recipes>,

    /// All live entities (see PORTING.md).
    pub entities: EntityArena,
    /// eid of the main player (Java `Game.player`; main() sets it to 0).
    pub player_id: i32,

    /// Java `Bed`'s static sleep-tracking state.
    pub bed_state: BedState,
    /// Java `AirWizard.beaten` static.
    pub air_wizard_beaten: bool,
}

impl Game {
    pub fn new(debug: bool, has_gui: bool, game_dir: PathBuf) -> Game {
        let localization = Localization::new();
        localization.debug.set(debug);
        let settings = Settings::new(&localization);
        let max_fps = settings.get("fps").as_int();
        let mut input = InputHandler::new();
        input.debug = debug;

        let mut g = Game {
            debug,
            has_gui,
            continous: true,
            input,
            game_dir,
            notifications: Vec::new(),
            max_fps,
            display: DisplayManager::default(),
            game_over: false,
            running: true,
            sound: SoundPlayer::new(has_gui),
            sound_enabled: settings.get("sound").as_bool(),
            settings,
            localization,
            gamespeed: 1.0,
            paused: true,
            tick_count: 0,
            time: 0,
            game_time: 0,
            past_day1: true,
            score_time: 0,
            note_tick: 0,
            as_tick: 0,
            saving: false,
            save_cooldown: 0,
            tile_tick_count: 0,
            levels: (0..6).map(|_| None).collect(),
            tiles: Tiles::new(),
            player_dead_time: 0,
            pending_level_change: 0,
            world_size: 128,
            current_level: 3,
            ready_to_render_gameplay: false,
            show_info: false,
            has_focus: true,
            fra: 0,
            tik: 0,
            random: JavaRandom::from_time(),
            items: Rc::new(Vec::new()),
            recipes: Rc::new(Recipes::new()),
            entities: EntityArena::default(),
            player_id: 0,
            bed_state: BedState::default(),
            air_wizard_beaten: false,
        };
        // The item registry reads settings (difficulty) during construction, mirroring
        // Java's static-init ordering.
        g.items = Rc::new(crate::item::registry::build_registry(&g));
        g
    }

    /* ------------------------- menus (Java Game.setMenu etc.) ------------------------- */

    /// Java `Game.setMenu(display)`.
    pub fn set_menu(&mut self, display: impl Display + 'static) {
        self.display.pending = PendingMenu::Set(Box::new(display));
    }

    /// Java `Game.setMenu(null)`.
    pub fn clear_menu(&mut self) {
        self.display.pending = PendingMenu::Clear;
    }

    /// Java `Game.exitMenu()`.
    pub fn exit_menu(&mut self) {
        if !self.display.menu_active() {
            return; // no action required; cannot exit from no menu
        }
        self.play_sound(Sound::Back);
        self.display.pending = PendingMenu::Exit;
    }

    /// Java `Game.getMenu() != null` (which read the pending `newMenu`).
    pub fn menu_open(&self) -> bool {
        self.display.menu_open()
    }

    /// Java `Game.isMode(mode)`.
    pub fn is_mode(&self, mode: &str) -> bool {
        self.settings.get("mode").as_str().eq_ignore_ascii_case(mode)
    }

    /// Java `Sound.xyz.play()`.
    pub fn play_sound(&self, sound: Sound) {
        self.sound.play(sound, self.sound_enabled);
    }

    /// Java `Sound.xyz.loop(start)`.
    pub fn play_sound_loop(&self, sound: Sound, start: bool) {
        self.sound.play_loop(sound, start, self.sound_enabled);
    }

    /// Java `Game.quit()`.
    pub fn quit(&mut self) {
        self.running = false;
    }

    /* --------------------------------- multiplayer ---------------------------------- */
    // The network layer is a stub (see PORTING.md "Multiplayer"); these keep every ported
    // call site reading like the Java code.

    pub fn is_online(&self) -> bool {
        false
    }
    pub fn is_valid_client(&self) -> bool {
        false
    }
    pub fn is_connected_client(&self) -> bool {
        false
    }
    pub fn is_valid_server(&self) -> bool {
        false
    }
    pub fn has_connected_clients(&self) -> bool {
        false
    }

    /* ------------------------------ Updater (tick) ----------------------------------- */

    /// Java `Updater.tick()` — "VERY IMPORTANT METHOD!! Makes everything keep happening."
    pub fn tick(&mut self) {
        self.sound_enabled = self.settings.get("sound").as_bool();

        self.apply_menu_transition();

        // TODO(port:level) Bed sleeping fast-forward
        // TODO(port:saveload) autosave tick

        // Increment tickCount if the game is not paused
        if !self.paused {
            self.set_time(self.tick_count + 1);
        }

        // TODO(port:screen) score mode game-over check

        // This is the general action statement thing! Regulates menus, mostly.
        if !self.has_focus && self.has_gui {
            self.input.release_all();
        }
        if self.has_focus || !self.has_gui {
            if !self.game_over {
                // TODO(port:entity) player.isRemoved() check
                self.game_time += 1;
            }

            self.input.tick(); // INPUT TICK; no other class should call this

            if self.display.menu_active() {
                // a menu is active.
                // TODO(port:entity) player.tick() — CRUCIAL that it precedes menu.tick()
                self.tick_current_display();
                self.paused = true;
            } else {
                // no menu, currently.
                self.paused = false;

                // TODO(port:entity) death menu delay, pending level change, player tick
                // TODO(port:level) level tick, Tile.tickCount++

                if !self.display.menu_active() && self.input.get_key("F3").clicked {
                    self.show_info = !self.show_info;
                }

                // TODO(port:level,entity,item) debug cheat keys
            }
        }
    }

    /// The Java menu-transition block at the top of `Updater.tick()`.
    fn apply_menu_transition(&mut self) {
        let pending = std::mem::replace(&mut self.display.pending, PendingMenu::NoChange);
        match pending {
            PendingMenu::NoChange => {}
            PendingMenu::Set(mut display) => {
                display.init(self);
                self.display.stack.push(display);
            }
            PendingMenu::Clear => {
                if let Some(mut top) = self.display.stack.pop() {
                    top.on_exit(self);
                }
                self.display.stack.clear();
            }
            PendingMenu::Exit => {
                if let Some(mut top) = self.display.stack.pop() {
                    top.on_exit(self);
                }
            }
        }
    }

    /// Tick the current (top) display with the take-out pattern.
    pub fn tick_current_display(&mut self) {
        if let Some(mut top) = self.display.stack.pop() {
            top.tick(self);
            self.display.stack.push(top);
        }
    }

    /* ------------------------------ time of day -------------------------------------- */

    /// Java `Updater.setTime(ticks)`.
    pub fn set_time(&mut self, mut ticks: i32) {
        if ticks < Time::Morning.tick_time() {
            ticks = 0; // error correct
        }
        if ticks < Time::Day.tick_time() {
            self.time = 0; // morning
        } else if ticks < Time::Evening.tick_time() {
            self.time = 1; // day
        } else if ticks < Time::Night.tick_time() {
            self.time = 2; // evening
        } else if ticks < updater::DAY_LENGTH {
            self.time = 3; // night
        } else {
            // back to morning
            self.time = 0;
            ticks = 0;
            self.past_day1 = true;
        }
        self.tick_count = ticks;
    }

    /// Java `Updater.changeTimeOfDay(t)`.
    pub fn change_time_of_day(&mut self, t: Time) {
        self.set_time(t.tick_time());
    }

    /// Java `Updater.getTime()`.
    pub fn get_time(&self) -> Time {
        Time::VALUES[self.time as usize]
    }

    /// Java `Updater.notifyAll(msg)`.
    pub fn notify_all(&mut self, msg: &str) {
        self.notify_all_tick(msg, 0);
    }

    /// Java `Updater.notifyAll(msg, notetick)`.
    pub fn notify_all_tick(&mut self, msg: &str, note_tick: i32) {
        let msg = self.localization.get_localized(msg);
        self.notifications.push(msg);
        self.note_tick = note_tick;
    }
}
