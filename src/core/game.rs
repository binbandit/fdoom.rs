//! Port of `fdoom.core.Game` + the mutable statics of `Updater`/`World`/`Renderer`.
//!
//! Java's global state becomes this one struct, threaded through the code as `g`.
//! See PORTING.md ("One `Game` struct instead of Java statics").

use std::path::PathBuf;
use std::rc::Rc;

use crate::core::io::input_handler::InputHandler;
use crate::core::io::localization::Localization;
use crate::core::io::settings::Settings;
use crate::core::io::sound::{Sound, SoundPlayer};
use crate::core::updater::{self, Time};
use crate::entity::EntityArena;
use crate::entity::furniture::bed::BedState;
use crate::item::Item;
use crate::item::recipe::Recipes;
use crate::level::Level;
use crate::level::tile::Tiles;
use crate::rng::Rng;
use crate::saveload::version::Version;
use crate::screen::display::{Display, DisplayManager, PendingMenu};

/// Java `Game.NAME` — the name on the application window.
pub const NAME: &str = "Fossickers Doom";

/// Java `Game.VERSION`.
pub fn version() -> Version {
    // 3.0: the sandbox pivot (sky level/Air Wizard/Score mode removed; worlds have five
    // layers). Worlds saved before 3.0 are refused on load.
    Version::new("3.0")
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

    /// Shared incidental RNG (see PORTING.md "Rng").
    pub random: Rng,

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
    /// Java `PlayerDeathDisplay.shouldRespawn` static.
    pub should_respawn: bool,
    /// Java `LoadingDisplay` percentage static (also used by the save HUD text).
    pub loading_percentage: f32,
    /// Java `LoadingDisplay` message static ("Level B3" etc.).
    pub loading_message: String,
    /// Java `WorldSelectDisplay.worldName` static.
    pub world_name: String,
    /// Java `WorldSelectDisplay.loadedWorld` static.
    pub loaded_world: bool,
    /// Java `WorldGenDisplay.getSeed()` — the seed for the next world generation.
    pub world_seed: i64,
}

impl Game {
    pub fn new(debug: bool, has_gui: bool, game_dir: PathBuf) -> Game {
        let localization = Localization::new();
        localization.debug.set(debug);
        let settings = Settings::new(localization.get_languages());
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
            note_tick: 0,
            as_tick: 0,
            saving: false,
            save_cooldown: 0,
            tile_tick_count: 0,
            levels: (0..crate::level::IDX_TO_DEPTH.len())
                .map(|_| None)
                .collect(),
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
            random: Rng::from_time(),
            items: Rc::new(Vec::new()),
            recipes: Rc::new(Recipes::new()),
            entities: EntityArena::default(),
            player_id: 0,
            bed_state: BedState::default(),
            air_wizard_beaten: false,
            should_respawn: true,
            loading_percentage: 0.0,
            loading_message: String::new(),
            world_name: String::new(),
            loaded_world: false,
            world_seed: 0,
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
        self.settings
            .get("mode")
            .as_str()
            .eq_ignore_ascii_case(mode)
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
        self.max_fps = self.settings.get("fps").as_int();

        self.apply_menu_transition();

        // IN BED (Java: Bed.sleeping() fast-forward)
        if self.bed_state.players_awake == 0 {
            if self.gamespeed != 20.0 {
                self.gamespeed = 20.0;
            }
            if self.tick_count > updater::SLEEP_END_TIME {
                if self.debug {
                    println!("passing midnight in bed");
                }
                self.past_day1 = true;
                self.tick_count = 0;
            }
            if self.tick_count <= updater::SLEEP_START_TIME
                && self.tick_count >= updater::SLEEP_END_TIME
            {
                // it has reached morning
                if self.debug {
                    println!("reached morning, getting out of bed");
                }
                self.gamespeed = 1.0;
                crate::entity::furniture::bed_behavior::restore_players(self);
            }
        }

        // auto-save tick; marks when to do autosave
        const ASTIME: i32 = 7200;
        if !self.paused {
            self.as_tick += 1;
        }
        if self.as_tick > ASTIME {
            let player_alive = self
                .try_player()
                .map(|p| p.player().mob.health > 0)
                .unwrap_or(false);
            if self.settings.get("autosave").as_bool() && !self.game_over && player_alive {
                crate::saveload::save::save_world_named(
                    self,
                    &crate::screen::world_select::get_world_name(self),
                );
            }
            self.as_tick = 0;
        }

        // Increment tickCount if the game is not paused. The day-cycle setting slows
        // the day clock: Classic advances every tick (~18min days), Long every 4th
        // (~72min), Realtime every 80th (a full 24 real hours per in-game day).
        if !self.paused {
            let divisor = match self.settings.get("daycycle").as_str() {
                "Long" => 4,
                "Realtime" => 80,
                _ => 1,
            };
            if self.game_time % divisor == 0 {
                self.set_time(self.tick_count + 1);
            }
        }

        // This is the general action statement thing! Regulates menus, mostly.
        if !self.has_focus && self.has_gui {
            self.input.release_all();
        }
        if self.has_focus || !self.has_gui {
            let player_removed = self.try_player().map(|p| p.c.removed).unwrap_or(true);
            if !player_removed && !self.game_over {
                self.game_time += 1;
            }

            self.input.tick(); // INPUT TICK; no other class should call this

            if self.display.menu_active() {
                // a menu is active.
                // CRUCIAL that the player is ticked HERE, before the menu is ticked
                let pid = self.player_id;
                self.with_entity(pid, |e, g| crate::entity::behavior::entity_tick(g, e));
                self.tick_current_display();
                self.paused = true;
            } else {
                // no menu, currently.
                self.paused = false;

                let player_removed = self.try_player().map(|p| p.c.removed).unwrap_or(true);
                let in_bed = crate::entity::furniture::bed_behavior::in_bed(self, self.player_id);
                if player_removed && self.ready_to_render_gameplay && !in_bed {
                    // makes delay between death and death menu
                    self.player_dead_time += 1;
                    if self.player_dead_time > 60 {
                        crate::screen::player_death_display::open(self);
                    }
                } else if self.pending_level_change != 0 {
                    let change = self.pending_level_change;
                    self.pending_level_change = 0;
                    crate::screen::level_transition_display::open(self, change);
                }

                // ticks the player when there's no menu
                let pid = self.player_id;
                self.with_entity(pid, |e, g| crate::entity::behavior::entity_tick(g, e));

                if self.levels[self.current_level].is_some() {
                    let lvl = self.current_level;
                    crate::level::ensure_chunks(self, lvl);
                    crate::level::tick_level(self, lvl, true);
                    self.tile_tick_count += 1;
                }

                if !self.display.menu_active() && self.input.get_key("F3").clicked {
                    // shows debug info in upper-left
                    self.show_info = !self.show_info;
                }

                // for debugging only
                if self.debug && self.has_gui {
                    if self.input.get_key("ctrl-p").clicked {
                        // print all players on all levels, and their coordinates
                        println!("printing players on all levels");
                        for e in self.entities.iter() {
                            if e.is_player() {
                                println!(
                                    "Player on level {:?} ({},{})",
                                    e.c.level,
                                    e.c.x >> 4,
                                    e.c.y >> 4
                                );
                            }
                        }
                    }

                    // host-only cheats
                    if self.input.get_key("Shift-r").clicked {
                        crate::core::world::init_world(self); // for single-player use only
                    }

                    if self.input.get_key("1").clicked {
                        self.change_time_of_day(Time::Morning);
                    }
                    if self.input.get_key("2").clicked {
                        self.change_time_of_day(Time::Day);
                    }
                    if self.input.get_key("3").clicked {
                        self.change_time_of_day(Time::Evening);
                    }
                    if self.input.get_key("4").clicked {
                        self.change_time_of_day(Time::Night);
                    }

                    if self.input.get_key("creative").clicked {
                        self.settings.set("mode", "creative");
                        self.fill_player_creative_inv(false);
                    }
                    if self.input.get_key("survival").clicked {
                        self.settings.set("mode", "survival");
                    }
                    if self.input.get_key("shift-0").clicked {
                        self.gamespeed = 1.0;
                    }
                    if self.input.get_key("shift-equals").clicked {
                        if self.gamespeed < 1.0 {
                            self.gamespeed *= 2.0;
                        } else if updater::NORM_SPEED as f32 * self.gamespeed < 2000.0 {
                            self.gamespeed += 1.0;
                        }
                    }
                    if self.input.get_key("shift-minus").clicked {
                        if self.gamespeed > 1.0 {
                            self.gamespeed -= 1.0;
                        } else if updater::NORM_SPEED as f32 * self.gamespeed > 5.0 {
                            self.gamespeed /= 2.0;
                        }
                    }

                    // client-only cheats, since they are player-specific
                    if self.input.get_key("shift-g").clicked {
                        self.fill_player_creative_inv(true);
                    }

                    if self.input.get_key("ctrl-h").clicked {
                        if let Some(m) = self.player_mut().mob_mut() {
                            m.health -= 1;
                        }
                    }
                    if self.input.get_key("ctrl-b").clicked {
                        self.player_mut().player_mut().hunger -= 1;
                    }

                    if self.input.get_key("0").clicked {
                        self.player_mut().player_mut().move_speed = 1.0;
                    }
                    if self.input.get_key("equals").clicked {
                        self.player_mut().player_mut().move_speed += 1.0;
                    }
                    if self.input.get_key("minus").clicked
                        && self.player_mut().player_mut().move_speed > 1.0
                    {
                        self.player_mut().player_mut().move_speed -= 1.0;
                    }

                    if self.input.get_key("shift-u").clicked {
                        let (x, y) = {
                            let p = self.player();
                            (p.c.x >> 4, p.c.y >> 4)
                        };
                        let t = self.tiles.get("Stairs Up");
                        let lvl = self.current_level;
                        self.set_tile_default(lvl, x, y, &t);
                    }
                    if self.input.get_key("shift-d").clicked {
                        let (x, y) = {
                            let p = self.player();
                            (p.c.x >> 4, p.c.y >> 4)
                        };
                        let t = self.tiles.get("Stairs Down");
                        let lvl = self.current_level;
                        self.set_tile_default(lvl, x, y, &t);
                    }
                } // end debug only cond
            } // end "menu-null" conditional
        } // end hasfocus conditional
    }

    /// Java `Items.fillCreativeInv(player.getInventory(), addAll)` on the live player.
    pub fn fill_player_creative_inv(&mut self, add_all: bool) {
        let Some(mut p) = self.entities.take(self.player_id) else {
            return;
        };
        {
            let mut inv = std::mem::take(&mut p.player_mut().inventory);
            inv.creative = self.is_mode("creative");
            crate::item::registry::fill_creative_inv(self, &mut inv, add_all);
            p.player_mut().inventory = inv;
        }
        self.entities.put_back(p);
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
            self.display.taken_out = true;
            top.tick(self);
            self.display.taken_out = false;
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
