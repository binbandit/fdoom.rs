//! Port of the `fdoom.core` package: game state, the main loop, and the top-level
//! renderer/updater. The Java statics of `Game`/`Updater`/`Renderer`/`World` become the
//! `Game` struct (see PORTING.md).

pub mod events;
pub mod file_handler;
pub mod game;
pub mod io;
pub mod my_utils;
pub mod renderer;
pub mod updater;
pub mod weather;
pub mod world;
