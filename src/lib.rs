//! Fossickers Doom — Rust port of the Java game Fossicker.
//!
//! See PORTING.md for the architecture and Java→Rust conventions.

pub mod assets;
pub mod core;
pub mod gfx;
pub mod java_random;
pub mod screen;

/// Entry point; equivalent of Java `fdoom.core.Game.main`.
pub fn run(_args: Vec<String>) {
    println!("fdoom: port in progress");
}
