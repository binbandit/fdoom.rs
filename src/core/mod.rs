//! Port of the `fdoom.core` package: game state, the main loop, and the top-level
//! renderer/updater. The Java statics of `Game`/`Updater`/`Renderer`/`World` become the
//! `Game` struct (see PORTING.md).

pub mod my_utils;
pub mod updater;
