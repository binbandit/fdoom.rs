//! Fossickers Doom — Rust port of the Java game Fossicker.
//!
//! See PORTING.md for the architecture and Java→Rust conventions.

pub mod assets;
pub mod core;
pub mod entity;
pub mod gfx;
pub mod item;
pub mod level;
pub mod network;
pub mod platform;
pub mod rng;
pub mod saveload;
pub mod screen;
pub mod testutil;

use std::sync::Arc;

use crate::core::file_handler;
use crate::core::game::Game;
use crate::core::renderer::Renderer;
use crate::screen::splash_menu::SplashMenu;

/// Entry point; equivalent of Java `fdoom.core.Game.main` + `Initializer.parseArgs`.
pub fn run(args: Vec<String>) {
    // parse command line arguments (Java Initializer.parseArgs)
    let mut debug = false;
    let mut save_dir = file_handler::system_game_dir();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--debug" => debug = true,
            "--savedir" if i + 1 < args.len() => {
                i += 1;
                save_dir = args[i].clone();
            }
            _ => {}
        }
        i += 1;
    }

    let game_dir = file_handler::determine_game_dir(&save_dir, debug);

    let mut game = Game::new(debug, true, game_dir);
    // (Tiles.initTileList(), Sound.init(), Settings.init() all happen in Game::new)

    // World.resetGame() — "half"-starts a new game, to set up initial variables;
    // the player entity gets eid 0 (g.player_id).
    core::world::reset_game(&mut game, true);

    // this loads any saved preferences (Java `new Load(true)`)
    saveload::load::load_prefs(&mut game);

    game.set_menu(SplashMenu::new()); // sets menu to the title screen

    let sheet = Arc::new(assets::sprite_sheet());
    let renderer = Renderer::new(sheet);

    platform::run(game, renderer);
}
