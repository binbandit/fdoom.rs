//! The `--debug` dev console: an info overlay (F4) and a command line (`/`).
//!
//! Both are wired in the debug-only block of `Game::tick`, so neither exists without
//! `--debug`. The command line is a `Display` on the stack, which gives it input
//! isolation for free (`g.menu_open()` gates all player input). `run_command` is a
//! pure `&mut Game` function so tests can drive it without a window.

use crate::core::game::Game;
use crate::core::updater::{self, Time};
use crate::entity::mob::player::MAX_STAT;
use crate::gfx::screen::{self, Screen};
use crate::gfx::{color, font};

use super::display::{Display, DisplayBase};

/* ---------------------------------- command line ---------------------------------- */

pub struct DevConsole {
    base: DisplayBase,
    typing: String,
    /// Tick counter for the caret blink (the game clock pauses while a menu is open).
    ticks: i32,
}

/// Open the console (the `/` key in the `Game::tick` debug block).
pub fn open(g: &mut Game) {
    g.set_menu(DevConsole {
        base: DisplayBase::new(false, false, Vec::new()),
        typing: String::new(),
        ticks: 0,
    });
}

impl Display for DevConsole {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn tick(&mut self, g: &mut Game) {
        self.ticks += 1;
        if g.input.get_key("exit").clicked {
            g.exit_menu();
            return;
        }
        self.typing = g.input.add_key_typed(&self.typing, None);
        if g.input.get_key("select").clicked {
            let line = std::mem::take(&mut self.typing);
            g.exit_menu();
            if !line.trim().is_empty() {
                run_command(g, &line);
            }
        }
    }

    fn render(&mut self, s: &mut Screen, _g: &mut Game) {
        let h = font::text_height();
        let y = screen::H - h - 2;
        s.darken_rect_screen(0, y - 2, screen::W, h + 4, 200);
        let caret = if (self.ticks / 20) % 2 == 0 { "_" } else { "" };
        font::draw(&format!(">{}{caret}", self.typing), s, 2, y, color::WHITE);
    }
}

/// Parse and execute one console line. Every outcome (including errors) reports
/// through the notification system.
pub fn run_command(g: &mut Game, line: &str) {
    if g.try_player().is_none() {
        // give/tp/heal use the panicking player accessor; no player, no commands
        g.notify_all("No player");
        return;
    }
    let mut words = line.split_whitespace();
    let Some(cmd) = words.next() else { return };
    let args: Vec<&str> = words.collect();
    match cmd.to_lowercase().as_str() {
        "give" => give(g, &args),
        "tp" => tp(g, &args),
        "time" => time(g, &args),
        "heal" => heal(g),
        other => g.notify_all(&format!("Unknown command: {other}")),
    }
}

/// `give <item> [n]` — item names may contain spaces, so the count is the last word
/// only when it parses as a number and at least one name word remains.
fn give(g: &mut Game, args: &[&str]) {
    if args.is_empty() {
        g.notify_all("Usage: give <item> [n]");
        return;
    }
    let (name_words, n) = match args[args.len() - 1].parse::<i32>() {
        Ok(n) if args.len() > 1 => (&args[..args.len() - 1], n.max(1)),
        _ => (args, 1),
    };
    let name = name_words.join(" ");
    let Some(proto) = g
        .items
        .iter()
        .find(|i| i.get_name().eq_ignore_ascii_case(&name))
    else {
        g.notify_all(&format!("No such item: {name}"));
        return;
    };
    let mut item = proto.clone();
    let display_name = item.get_name().to_string();
    let inv = &mut g.player_mut().player_mut().inventory;
    if item.is_stackable() {
        item.set_count(n);
        inv.add(item);
    } else {
        inv.add_num(item, n);
    }
    g.notify_all(&format!("Gave {n} x {display_name}"));
}

/// `tp <x> <y>` — tile coordinates on the current level.
fn tp(g: &mut Game, args: &[&str]) {
    let (Some(x), Some(y)) = (
        args.first().and_then(|a| a.parse::<i32>().ok()),
        args.get(1).and_then(|a| a.parse::<i32>().ok()),
    ) else {
        g.notify_all("Usage: tp <x> <y>");
        return;
    };
    let p = g.player_mut();
    p.c.x = x.saturating_mul(16).saturating_add(8);
    p.c.y = y.saturating_mul(16).saturating_add(8);
    g.notify_all(&format!("Teleported to {x},{y}"));
}

/// `time <morning|noon|dusk|night>` (in-game names day/evening accepted too).
fn time(g: &mut Game, args: &[&str]) {
    let t = match args.first().map(|a| a.to_lowercase()).as_deref() {
        Some("morning") => Time::Morning,
        Some("noon") | Some("day") => Time::Day,
        Some("dusk") | Some("evening") => Time::Evening,
        Some("night") => Time::Night,
        _ => {
            g.notify_all("Usage: time <morning|noon|dusk|night>");
            return;
        }
    };
    g.change_time_of_day(t);
    g.notify_all(&format!("Time set to {t}"));
}

/// `heal` — full health, hunger, and stamina.
fn heal(g: &mut Game) {
    let pd = g.player_mut().player_mut();
    pd.mob.health = pd.mob.max_health;
    pd.hunger = MAX_STAT;
    pd.stamina = MAX_STAT;
    g.notify_all("Healed");
}

/* ------------------------------------ overlay ------------------------------------- */

/// The F4 info overlay: right-aligned under the HUD's arrow box, above the
/// notification band. Called from `Renderer::render_gui` so lighting never grades it.
pub fn render_overlay(s: &mut Screen, g: &Game) {
    let Some(p) = g.try_player() else { return };
    let (px, py) = (p.c.x >> 4, p.c.y >> 4);
    let lvl = g.current_level;
    let depth = crate::level::IDX_TO_DEPTH[lvl];

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("{} fps", g.fra));
    lines.push(format!("Seed {}", g.world_seed));
    // tick 0 is dawn; show it as 06:00 so the clock reads like a day
    let mins = (g.tick_count as i64 * 24 * 60 / updater::DAY_LENGTH as i64 + 6 * 60) % (24 * 60);
    lines.push(format!(
        "Day {} {:02}:{:02} ({})",
        g.events.day_number + 1,
        mins / 60,
        mins % 60,
        g.get_time()
    ));
    lines.push(format!(
        "{} (depth {depth})",
        crate::level::get_level_name(depth)
    ));
    if g.levels[lvl].is_some() {
        if depth == 0 && g.level(lvl).is_infinite() {
            let biome = crate::level::infinite_gen::biome_at(g.world_seed, px, py);
            lines.push(format!("Biome {biome:?}"));
        }
        lines.push(format!("Tile {px},{py}"));
        let tile = g.tile_at(lvl, px, py);
        let data = g.level(lvl).get_data(px, py);
        lines.push(format!("{} d{data}", tile.name));
    }

    let mut y = 18;
    for line in &lines {
        let w = font::text_width(line);
        let x = screen::W - 2 - w;
        s.darken_rect_screen(x - 2, y - 1, w + 4, font::text_height() + 1, 185);
        font::draw(line, s, x, y, color::WHITE);
        y += font::text_height();
    }
}
