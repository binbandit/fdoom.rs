//! Fossickers Doom — Rust port of the Java game Fossicker.
//!
//! See PORTING.md for the architecture and Java→Rust conventions.

pub mod assets;
pub mod core;
pub mod entity;
pub mod gfx;
pub mod item;
pub mod java_random;
pub mod platform;
pub mod saveload;
pub mod screen;

use std::sync::Arc;

use crate::core::file_handler;
use crate::core::game::Game;
use crate::core::renderer::Renderer;
use crate::gfx::SpriteSheet;
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
            "--localclient" | "--server" => {
                eprintln!("multiplayer is not available in this build (see PORTING.md)");
                std::process::exit(1);
            }
            _ => {}
        }
        i += 1;
    }

    let game_dir = file_handler::determine_game_dir(&save_dir, debug);

    let mut game = Game::new(debug, true, game_dir);

    // TODO(port:level) Tiles.initTileList()
    // TODO(port:entity) World.resetGame(); player.eid = 0
    // TODO(port:saveload) new Load(true) — load saved preferences

    game.set_menu(SplashMenu::new()); // sets menu to the title screen

    let sheet = Arc::new(SpriteSheet::from_png(assets::ICONS_PNG));
    let renderer = Renderer::new(sheet);

    platform::run(game, renderer);
}
