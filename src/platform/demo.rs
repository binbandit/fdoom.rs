//! Development/verification driver: `FDOOM_DEMO="wait:30;shot:/tmp/a.png;key:ENTER;quit"`
//! scripts the real game (key events + frame dumps) so rendering and flows can be verified
//! headlessly-ish. Not part of the game; does nothing unless the env var is set.

use crate::core::game::Game;
use crate::core::renderer::Renderer;
use crate::gfx::screen::{H, W};

#[derive(Debug, Clone)]
enum Step {
    Wait(i32),
    Shot(String),
    /// Tap: press this tick, release next tick.
    Key(String),
    Down(String),
    Up(String),
    Type(char),
    Quit,
}

pub struct Demo {
    steps: Vec<Step>,
    idx: usize,
    wait_left: i32,
    release_next: Option<String>,
    pub pending_shot: Option<String>,
}

impl Demo {
    pub fn from_env() -> Option<Demo> {
        let script = std::env::var("FDOOM_DEMO").ok()?;
        let steps = script
            .split(';')
            .filter(|s| !s.trim().is_empty())
            .map(|s| {
                let (cmd, arg) = s.split_once(':').unwrap_or((s, ""));
                match cmd.trim() {
                    "wait" => Step::Wait(arg.parse().unwrap_or(1)),
                    "shot" => Step::Shot(arg.to_string()),
                    "key" => Step::Key(arg.to_string()),
                    "down" => Step::Down(arg.to_string()),
                    "up" => Step::Up(arg.to_string()),
                    "type" => Step::Type(arg.chars().next().unwrap_or(' ')),
                    "quit" => Step::Quit,
                    other => panic!("unknown FDOOM_DEMO step: {other}"),
                }
            })
            .collect();
        Some(Demo {
            steps,
            idx: 0,
            wait_left: 0,
            release_next: None,
            pending_shot: None,
        })
    }

    /// Called once per game tick, before `game.tick()`.
    pub fn on_tick(&mut self, game: &mut Game) {
        // scripted runs must not depend on the OS granting window focus
        game.has_focus = true;
        if self.pending_shot.is_some() {
            return; // hold the script until the frame is actually rendered
        }
        if let Some(key) = self.release_next.take() {
            game.input.key_toggled(&key, false);
        }
        if self.wait_left > 0 {
            self.wait_left -= 1;
            return;
        }
        while self.idx < self.steps.len() {
            let step = self.steps[self.idx].clone();
            self.idx += 1;
            match step {
                Step::Wait(n) => {
                    self.wait_left = n;
                    return;
                }
                Step::Shot(path) => {
                    self.pending_shot = Some(path);
                    return;
                }
                Step::Key(name) => {
                    game.input.key_toggled(&name, true);
                    // typed-char side channel for text inputs
                    if name.chars().count() == 1 {
                        game.input.key_typed(name.chars().next().unwrap());
                    }
                    self.release_next = Some(name);
                    return;
                }
                Step::Down(name) => game.input.key_toggled(&name, true),
                Step::Up(name) => game.input.key_toggled(&name, false),
                Step::Type(ch) => game.input.key_typed(ch),
                Step::Quit => {
                    game.quit();
                    return;
                }
            }
        }
    }

    /// Called after a frame has been rendered; dumps the shot if one is pending.
    pub fn on_frame(&mut self, renderer: &Renderer) {
        if let Some(path) = self.pending_shot.take() {
            if let Err(e) = dump_png(&path, &renderer.screen.pixels) {
                eprintln!("FDOOM_DEMO: could not write {path}: {e}");
            } else {
                println!("FDOOM_DEMO: wrote {path}");
            }
        }
    }
}

fn dump_png(path: &str, pixels: &[i32]) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(path)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), W as u32, H as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header()?;
    let mut data = Vec::with_capacity((W * H * 3) as usize);
    for &p in pixels {
        data.push(((p >> 16) & 0xff) as u8);
        data.push(((p >> 8) & 0xff) as u8);
        data.push((p & 0xff) as u8);
    }
    writer.write_image_data(&data)?;
    Ok(())
}
