//! Port of `fdoom.core.Renderer` — draws each frame into the software screen, then the
//! platform layer scales/blits `self.screen.pixels` to the window.
//!
//! The framebuffer size is decided at runtime (288x192 up to 640x400, see
//! `platform::logical_size_for_window`), so layout math must read the live
//! `screen.w`/`screen.h` and never the classic [`WIDTH`]/[`HEIGHT`] constants.

use std::sync::Arc;

use crate::core::game::{self, Game};
use crate::core::updater;
use crate::entity::furniture::bed_behavior;
use crate::gfx::screen::{self, Screen};
use crate::gfx::sprite_sheet::SpriteSheet;
use crate::gfx::{Dimension, FontStyle, Point, color, font};
use crate::item::{Item, ItemKind, ToolType};
use crate::screen::RelPos;

/// The classic screen size, kept only as the *minimum* logical resolution the window
/// may shrink to (`platform::logical_size_for_window` clamps against these). Never use
/// them for layout — the live framebuffer is `Renderer::screen.{w,h}`.
pub const HEIGHT: i32 = screen::H;
pub const WIDTH: i32 = screen::W;

/// Title-screen drone-flyover state: a throwaway infinite surface slowly panned under
/// the main menu.
struct Flyover {
    seed: i64,
    cam_x: f64,
    cam_y: f64,
    heading: f64,
}

pub struct Renderer {
    flyover: Option<Flyover>,
    pub screen: Screen,
    /// The light buffer (raw 0-255 brightness); `gfx::lighting` clears it and stamps
    /// emitters into it each frame before the atmosphere pass reads it.
    pub light_screen: Screen,
    /// Sheet handle for the HUD's 2x held-item icon (Screen only blits at 1x).
    sheet: Arc<SpriteSheet>,
    hud: HudMem,
}

/// Corner-HUD transient state (docs/UI_REDESIGN.md §2). The frameless HUD only shows a
/// meter that needs attention, so it has to remember what changed recently: vitals rows
/// linger ~90 frames after moving, the held-item name label shows briefly on a switch
/// then fades, and the armor chip flashes when it absorbs a hit. Counted in render
/// frames, the same clock the notification ticker ages on.
struct HudMem {
    /// First-frame guard: priming adopts the current stats silently so loading a save
    /// (or booting a test world) does not flash every full meter at once.
    primed: bool,
    last_health: i32,
    last_stamina: i32,
    last_hunger: i32,
    last_thirst: i32,
    health_show: i32,
    stamina_show: i32,
    hunger_show: i32,
    thirst_show: i32,
    last_armor: i32,
    armor_flash: i32,
    held_label: Option<String>,
    label_ticks: i32,
}

/// How long a changed meter / fresh item label stays up, in render frames (matches the
/// ambient ticker's 90-frame line lifetime).
const HUD_SHOW_FRAMES: i32 = 90;

impl HudMem {
    fn new() -> HudMem {
        HudMem {
            primed: false,
            last_health: 0,
            last_stamina: 0,
            last_hunger: 0,
            last_thirst: 0,
            health_show: 0,
            stamina_show: 0,
            hunger_show: 0,
            thirst_show: 0,
            last_armor: 0,
            armor_flash: 0,
            held_label: None,
            label_ticks: 0,
        }
    }

    /// Advance one stat's change-memory: reset the linger timer on movement, otherwise
    /// count it down.
    fn track(last: &mut i32, show: &mut i32, cur: i32) {
        if cur != *last {
            *last = cur;
            *show = HUD_SHOW_FRAMES;
        } else if *show > 0 {
            *show -= 1;
        }
    }

    /// Fold one frame's stats into the change-memory. The first call primes silently:
    /// loading a save (or booting a test world) must not flash every full meter at once.
    fn update(&mut self, v: &Vitals, held_label: Option<String>) {
        if !self.primed {
            self.primed = true;
            self.last_health = v.health;
            self.last_stamina = v.stamina;
            self.last_hunger = v.hunger;
            self.last_thirst = v.thirst;
            self.last_armor = v.armor;
            self.held_label = held_label;
            return;
        }
        HudMem::track(&mut self.last_health, &mut self.health_show, v.health);
        HudMem::track(&mut self.last_stamina, &mut self.stamina_show, v.stamina);
        HudMem::track(&mut self.last_hunger, &mut self.hunger_show, v.hunger);
        HudMem::track(&mut self.last_thirst, &mut self.thirst_show, v.thirst);
        if v.armor < self.last_armor {
            self.armor_flash = 24; // the chip blinks while it soaks a hit
        }
        self.last_armor = v.armor;
        if self.armor_flash > 0 {
            self.armor_flash -= 1;
        }
        if held_label != self.held_label {
            self.label_ticks = if held_label.is_some() {
                HUD_SHOW_FRAMES
            } else {
                0
            };
            self.held_label = held_label;
        } else if self.label_ticks > 0 {
            self.label_ticks -= 1;
        }
    }
}

/* ---------------------------- corner-HUD primitives ----------------------------
UI_REDESIGN §2: frameless, corner-anchored, fixed slots. All geometry below is
measured against target/verify/ui_mock/mock_hud_{calm,alert}.png. */

const PLATE_BORDER_RGB: i32 = 0x666666;

/// One frameless vitals row: icons on a light smoked strip, plus the 1px white pulse
/// underline when the meter is at/below 30%.
fn vitals_row(
    screen: &mut Screen,
    y: i32,
    sprite_cell: i32,
    cols: impl Fn(i32) -> i32,
    low: bool,
    pulse_on: bool,
) {
    let n = crate::entity::mob::player::MAX_STAT;
    screen.darken_rect_screen(2, y - 1, n * 8 + 4, 10, 90);
    for i in 0..n {
        screen.render(4 + i * 8, y, sprite_cell + 12 * 32, cols(i), 0);
    }
    if low && pulse_on {
        screen.fill_rect(4, y + 8, n * 8, 1, 0xF0F0F0);
    }
}

/// Small shadowed text (the corner HUD draws over raw world, so every naked string
/// gets a 1px black drop shadow instead of a backing band).
fn draw_text_shadowed(screen: &mut Screen, msg: &str, x: i32, y: i32, col: i32) {
    font::draw(msg, screen, x + 1, y + 1, color::get(-1, 0));
    font::draw(msg, screen, x, y, col);
}

/// Shadowed text in a literal RGB color (the temperature label matches the dot's
/// exact band color, which the packed 0-5 palette cannot express). Draws white, then
/// recolors the glyph pixels: capture-before/compare-after keeps it exact.
fn draw_text_rgb(screen: &mut Screen, msg: &str, x: i32, y: i32, rgb: i32) {
    let w = font::text_width(msg) + 2;
    let screen_w = screen.w;
    let (x0, y0) = (x.max(0), y.max(0));
    let (x1, y1) = ((x + w).min(screen.w), (y + 9).min(screen.h));
    let before: Vec<i32> = (y0..y1)
        .flat_map(|yy| (x0..x1).map(move |xx| (yy * screen_w + xx) as usize))
        .map(|i| screen.pixels[i])
        .collect();
    draw_text_shadowed(screen, msg, x, y, color::get(-1, 555));
    let white = color::upgrade(color::get_byte(555));
    let mut i = 0;
    for yy in y0..y1 {
        for xx in x0..x1 {
            let d = (yy * screen.w + xx) as usize;
            if screen.pixels[d] != before[i] && screen.pixels[d] == white {
                screen.pixels[d] = rgb;
            }
            i += 1;
        }
    }
}

/// A 7x7 badge dot (black rim, 5x5 fill) — the temperature indicator's shape, shared
/// by the potion-effect pips so the badge row reads as one instrument cluster.
fn badge_dot(screen: &mut Screen, x: i32, y: i32, rgb: i32) {
    screen.fill_rect(x, y, 7, 7, 0x000000);
    screen.fill_rect(x + 1, y + 1, 5, 5, rgb);
}

/// Blit a 1x1-cell sprite at 2x into the held plate (Screen has no scaled path; this
/// reads the sheet directly with the same palette/true-color rules as Screen::render).
fn render_sprite_2x(
    screen: &mut Screen,
    sheet: &SpriteSheet,
    sprite: &crate::gfx::sprite::Sprite,
    x: i32,
    y: i32,
) {
    let px = &sprite.sprite_pixels[0][0];
    let toffs = (px.sheet_pos % 32) * 8 + (px.sheet_pos / 32) * 8 * sheet.width;
    for sy in 0..8 {
        let ys = if px.mirror & 0x02 > 0 { 7 - sy } else { sy };
        for sx in 0..8 {
            let xs = if px.mirror & 0x01 > 0 { 7 - sx } else { sx };
            let rgb = match sheet.pixels[(toffs + xs + ys * sheet.width) as usize] {
                crate::gfx::sprite_sheet::SheetPixel::Palette(shade) => {
                    let col = (sprite.color >> ((3 - shade as i32) * 8)) & 0xFF;
                    if col >= 255 {
                        continue;
                    }
                    color::upgrade(col)
                }
                crate::gfx::sprite_sheet::SheetPixel::Rgb(rgb) => rgb,
                crate::gfx::sprite_sheet::SheetPixel::Transparent => continue,
            };
            screen.fill_rect(x + sx * 2, y + sy * 2, 2, 2, rgb);
        }
    }
}

/// Empty hands: a dim fist glyph in the plate (2x, like the item icons) instead of
/// the old bare `-`.
fn render_fist(screen: &mut Screen, x: i32, y: i32) {
    const FIST: [&str; 8] = [
        "........", ".oo.oo..", "offoffo.", "offfffo.", "offfffo.", "offfffo.", ".ooooo..",
        "........",
    ];
    for (r, row) in FIST.iter().enumerate() {
        for (c, ch) in row.bytes().enumerate() {
            let rgb = match ch {
                b'o' => 0x3A3A3A,
                b'f' => 0x5A5A5A,
                _ => continue,
            };
            screen.fill_rect(x + c as i32 * 2, y + r as i32 * 2, 2, 2, rgb);
        }
    }
}

/* ------------------------------- corner-HUD layout -------------------------------
Every corner-HUD anchor, derived from the LIVE framebuffer. The window runs anywhere
from 288x192 to 640x400, so nothing here may read the classic WIDTH/HEIGHT constants —
a slot pinned to 192 lands in the middle of a 400-tall screen. Offsets are measured
against target/verify/ui_mock/mock_hud_{calm,alert}.png. */

#[derive(Clone, Copy)]
struct HudLayout {
    /// live framebuffer width — the right-aligned slots measure back from it
    w: i32,
    /// vitals rows, top to bottom
    hearts_y: i32,
    stamina_y: i32,
    food_y: i32,
    /// the L6-reserved slot, below hunger
    thirst_y: i32,
    /// badge tier above the vitals: temperature dot, potion pips, held-item name
    badge_y: i32,
    /// top-left corner of the 18x18 held-item plate
    plate_x: i32,
    plate_y: i32,
}

impl HudLayout {
    fn for_screen(w: i32, h: i32) -> HudLayout {
        HudLayout {
            w,
            hearts_y: h - 34,
            stamina_y: h - 26,
            food_y: h - 18,
            thirst_y: h - 10,
            badge_y: h - 46,
            plate_x: w - 22,
            plate_y: h - 22,
        }
    }
}

/// Left edge of the vitals strips.
const VITALS_X: i32 = 2;
/// A vitals strip is `MAX_STAT` 8px icon cells plus 2px of padding either side. The
/// armor chip docks flush against its right edge, so both derive from this.
const VITALS_STRIP_W: i32 = crate::entity::mob::player::MAX_STAT * 8 + 4;
/// Badge-tier slot right of the temperature dot and its optional COLD/HOT word.
const EFFECT_PIPS_X: i32 = 48;

/// The player numbers the HUD reads, sampled once per frame: the change-memory, the
/// vitals rows and the armor chip all want them, and `Game` cannot stay borrowed while
/// the draws run.
struct Vitals {
    health: i32,
    stamina: i32,
    stamina_recharge_delay: i32,
    hunger: i32,
    thirst: i32,
    armor: i32,
    /// the worn armor's sprite color — the chip's tint when it is not flashing
    armor_color: Option<i32>,
}

impl Vitals {
    fn sample(g: &Game) -> Vitals {
        let p = g.player();
        let pd = p.player();
        Vitals {
            health: pd.mob.health,
            stamina: pd.stamina,
            stamina_recharge_delay: pd.stamina_recharge_delay,
            hunger: pd.hunger,
            thirst: pd.thirst,
            armor: pd.armor,
            armor_color: pd.cur_armor.as_ref().map(|a| a.sprite.color),
        }
    }
}

/// The held item's HUD name. Tools carry their level ("Crude Pickaxe"); everything else
/// uses the bare localized name — the count prefix the old text carried now lives in the
/// badge, so the label never needs truncating.
fn held_label(g: &Game, active: Option<&Item>) -> Option<String> {
    active.map(|item| match &item.kind {
        ItemKind::Tool { .. } => item.get_display_name(g).trim().to_string(),
        _ => g.localization.get_localized(item.get_name()).to_string(),
    })
}

/* -------------------------------- vitals + badges -------------------------------- */

/// The three vitals rows in their permanent slots. A row is drawn when its meter is
/// below max, moved in the last ~90 frames, or (stamina only) is mid-recharge-blink;
/// at/below 30% it gains the pulse underline. Full and settled = absent.
fn draw_vitals(screen: &mut Screen, l: HudLayout, v: &Vitals, mem: &HudMem, pulse_on: bool) {
    use crate::entity::mob::player::{MAX_HEALTH, MAX_HUNGER, MAX_STAMINA};

    let (health, stamina, hunger) = (v.health, v.stamina, v.hunger);
    let recharge_delay = v.stamina_recharge_delay;

    if health < MAX_HEALTH || mem.health_show > 0 {
        vitals_row(
            screen,
            l.hearts_y,
            0,
            |i| {
                if i < health {
                    color::get4(-1, 200, 500, 533)
                } else {
                    color::get4(-1, 100, 0, 0)
                }
            },
            health * 10 <= MAX_HEALTH * 3,
            pulse_on,
        );
    }
    if stamina < MAX_STAMINA || mem.stamina_show > 0 || recharge_delay > 0 {
        vitals_row(
            screen,
            l.stamina_y,
            1,
            |i| {
                if recharge_delay > 0 {
                    // the white/gray blinking effect when you run out
                    if recharge_delay / 4 % 2 == 0 {
                        color::get4(-1, 555, 0, 0)
                    } else {
                        color::get4(-1, 110, 0, 0)
                    }
                } else if i < stamina {
                    color::get4(-1, 220, 550, 553)
                } else {
                    color::get4(-1, 110, 0, 0)
                }
            },
            stamina * 10 <= MAX_STAMINA * 3,
            pulse_on,
        );
    }
    if hunger < MAX_HUNGER || mem.hunger_show > 0 {
        vitals_row(
            screen,
            l.food_y,
            2,
            |i| {
                if i < hunger {
                    color::get4(-1, 100, 530, 211)
                } else {
                    color::get4(-1, 100, 0, 0)
                }
            },
            hunger * 10 <= MAX_HUNGER * 3,
            pulse_on,
        );
    }

    // THIRST droplets (gentle thirst, L6) in the reserved slot: same hidden-while-full
    // / linger / low-pulse rules as the rows above.
    // TODO(art): a dedicated droplet cell; until then the heart cell renders Y-mirrored
    // (point up, round bottom) in water blues.
    {
        use crate::entity::mob::player::MAX_THIRST;
        let thirst = v.thirst;
        if thirst < MAX_THIRST || mem.thirst_show > 0 {
            let n = crate::entity::mob::player::MAX_STAT;
            screen.darken_rect_screen(2, l.thirst_y - 1, n * 8 + 4, 10, 90);
            for i in 0..n {
                let col = if i < thirst {
                    color::get4(-1, 12, 235, 455)
                } else {
                    color::get4(-1, 1, 0, 0)
                };
                screen.render(
                    4 + i * 8,
                    l.thirst_y,
                    12 * 32,
                    col,
                    crate::gfx::screen::BIT_MIRROR_Y,
                );
            }
            if thirst <= 3 && pulse_on {
                screen.fill_rect(4, l.thirst_y + 8, n * 8, 1, 0xF0F0F0);
            }
        }
    }
}

/// Shield pip + hits-left count, docked against the right edge of the hearts strip and
/// drawn only while armor is worn. Flashes white while it soaks a hit. Detail lives on
/// the WEAR tab.
fn draw_armor_chip(screen: &mut Screen, l: HudLayout, v: &Vitals, flash: i32) {
    if v.armor <= 0 {
        return;
    }
    let chip_x = VITALS_X + VITALS_STRIP_W;
    let count = v.armor.to_string();
    let wtxt = font::text_width(&count);
    screen.darken_rect_screen(chip_x, l.hearts_y - 1, wtxt + 13, 10, 90);
    let flash_on = flash > 0 && (flash / 3) % 2 == 0;
    let col = if flash_on {
        color::get(-1, 555)
    } else {
        v.armor_color.unwrap_or(color::get4(-1, 111, 333, 444))
    };
    screen.render(chip_x + 2, l.hearts_y, 3 + 12 * 32, col, 0);
    draw_text_shadowed(screen, &count, chip_x + 11, l.hearts_y, color::get(-1, 555));
}

/// Band color for the temperature dot (core::temperature). The outermost bands alternate
/// with the pulse clock; `steps == 0` is the comfort band and never draws.
fn temp_band_rgb(steps: i32, pulse_on: bool) -> i32 {
    match steps {
        -1 => 0x5E8FD4,
        -2 => 0x3E6FE0,
        i32::MIN..=-3 => {
            if pulse_on {
                0x8FB4FF
            } else {
                0x2B4FF0
            }
        }
        1 => 0xD9A85A,
        2 => 0xE07E33,
        _ => {
            if pulse_on {
                0xFF9A66
            } else {
                0xE0491F
            }
        }
    }
}

/// The temperature dot in the badge tier, plus its one-word label from +-2 steps out.
/// Comfort band: absent.
fn draw_temperature_badge(screen: &mut Screen, l: HudLayout, steps: i32, pulse_on: bool) {
    if steps == 0 {
        return;
    }
    let rgb = temp_band_rgb(steps, pulse_on);
    badge_dot(screen, 4, l.badge_y, rgb);
    if steps.abs() >= 2 {
        let word = if steps < 0 { "COLD" } else { "HOT" };
        draw_text_rgb(screen, word, 12, l.badge_y, rgb);
    }
}

/// Potion pips share the badge tier right of the temp slot — the same dot silhouette in
/// each potion's display color, so the row reads as one instrument cluster. Names and
/// timers belong to the SELF tab.
fn draw_effect_pips(screen: &mut Screen, l: HudLayout, g: &Game) {
    let mut effects: Vec<crate::item::PotionType> =
        g.player().player().potioneffects.keys().copied().collect();
    effects.sort_by_key(|p| p.disp_color()); // HashMap order is unstable
    for (i, ptype) in effects.iter().enumerate() {
        let x = EFFECT_PIPS_X + i as i32 * 9;
        if x + 7 > l.w {
            break;
        }
        badge_dot(
            screen,
            x,
            l.badge_y,
            color::upgrade(color::get_byte(ptype.disp_color())),
        );
    }
}

/* ------------------------------ banners + messages ------------------------------ */

/// The centered sleep/bed banner's lines. A non-empty result also means the warning
/// band stands down for the frame.
fn perm_status_lines(g: &Game) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    if bed_behavior::sleeping(g) {
        lines.push("Sleeping...".to_string());
    } else if g.bed_state.players_awake > 0 && bed_behavior::in_bed(g, g.player_id) {
        let num_awake = g.bed_state.players_awake;
        lines.push(crate::core::my_utils::plural(num_awake, "player") + " still awake");
        lines.push(" ".to_string());
        lines.push(format!("Press {} to cancel", g.input.get_mapping("exit")));
    }
    lines
}

fn draw_perm_status(screen: &mut Screen, lines: &[String]) {
    let mut style = FontStyle::new(color::WHITE)
        .set_y_pos(screen.h / 2 - 25)
        .set_rel_text_pos(RelPos::Top)
        .set_shadow_type(color::DARK_GRAY, false);
    font::draw_paragraph(lines, screen, &mut style, 1);
}

/// Warning / event band (playtest #2) — centered and loud on purpose. Ages one line off
/// every 120 frames and never shows more than 3.
fn draw_warning_band(screen: &mut Screen, g: &mut Game) {
    if g.warnings.is_empty() {
        return;
    }
    g.note_tick += 1;
    if g.warnings.len() > 3 {
        // only show 3 warnings max at one time; erase old ones
        let start = g.warnings.len() - 3;
        g.warnings = g.warnings[start..].to_vec();
    }
    if g.note_tick > 120 {
        // display time per warning
        g.warnings.remove(0);
        g.note_tick = 0;
    }

    let mut style = FontStyle::new(color::WHITE)
        .set_shadow_type(color::DARK_GRAY, false)
        .set_y_pos(screen.h * 2 / 5)
        .set_rel_text_pos_both(RelPos::Top, false);
    let notes = g.warnings.clone();
    // smoked-glass backing band (same primitive as the menu panels) so the text reads
    // over any terrain; mirrors the geometry FontStyle computes above
    let size = Dimension::new(
        font::text_width_para(&notes),
        notes.len() as i32 * font::text_height(),
    );
    let band = RelPos::Top.position_rect(size, Point::new(screen.w / 2, screen.h * 2 / 5));
    screen.darken_rect_screen(
        band.left() - 4,
        band.top() - 3,
        band.width() + 8,
        band.height() + 5,
        185,
    );
    font::draw_paragraph(&notes, screen, &mut style, 0);
}

/// Ambient chatter ticker — flush to the top-left edge (the frame boxes it used to dock
/// under are gone), newest line on top, ~90 frames each, max 3 lines. The font is
/// caps-only, so quiet placement and a faint backing do the de-emphasis instead of
/// sentence case.
fn draw_ticker(screen: &mut Screen, g: &mut Game, menu_open: bool) {
    g.sync_note_ages();
    if menu_open || g.notifications.is_empty() {
        return;
    }
    for age in &mut g.note_ages {
        *age += 1;
    }
    let mut i = 0;
    while i < g.notifications.len() {
        if g.note_ages[i] > 90 {
            g.notifications.remove(i);
            g.note_ages.remove(i);
        } else {
            i += 1;
        }
    }
    while g.notifications.len() > 3 {
        g.notifications.remove(0);
        g.note_ages.remove(0);
    }

    const TICKER_X: i32 = 4;
    const TICKER_Y: i32 = 3; // top edge of the frame (mock_hud_calm)
    for (row, idx) in (0..g.notifications.len()).rev().enumerate() {
        let line = &g.notifications[idx];
        let y = TICKER_Y + row as i32 * 9;
        let w = font::text_width(line);
        screen.darken_rect_screen(TICKER_X - 2, y - 1, w + 4, 9, 150);
        // newest line bright, older lines receding
        let col = if row == 0 {
            color::get(-1, 555)
        } else {
            color::get(-1, 333)
        };
        font::draw(line, screen, TICKER_X, y, col);
    }
}

/// Bottom-right save toast: live progress while saving, then the "World Saved!" line the
/// save path pushes. Lifts clear of the held-item name label while that is up — both are
/// transient, so the collision is rare and the dodge is cheap.
fn draw_save_toast(screen: &mut Screen, g: &mut Game, label_up: bool) {
    if !g.saving && g.toast.is_some() {
        g.toast_tick += 1;
        if g.toast_tick > 90 {
            g.toast = None;
        }
    }
    let line = if g.saving {
        Some(format!("Saving... {}%", g.loading_percentage.round()))
    } else {
        g.toast.clone()
    };
    let Some(line) = line else {
        return;
    };
    let lift = if label_up { 20 } else { 0 };
    let w = font::text_width(&line);
    let (tx, ty) = (screen.w - w - 4, screen.h - 12 - lift);
    screen.darken_rect_screen(tx - 2, ty - 1, w + 4, 10, 185);
    font::draw(&line, screen, tx, ty, color::get(-1, 555));
}

/* --------------------------------- held-item plate --------------------------------- */

/// The plate's 18x18 shell: 1px border, smoked-glass interior.
fn draw_plate_frame(screen: &mut Screen, l: HudLayout) {
    screen.fill_rect(l.plate_x, l.plate_y, 18, 1, PLATE_BORDER_RGB);
    screen.fill_rect(l.plate_x, l.plate_y + 17, 18, 1, PLATE_BORDER_RGB);
    screen.fill_rect(l.plate_x, l.plate_y + 1, 1, 16, PLATE_BORDER_RGB);
    screen.fill_rect(l.plate_x + 17, l.plate_y + 1, 1, 16, PLATE_BORDER_RGB);
    screen.darken_rect_screen(l.plate_x + 1, l.plate_y + 1, 16, 16, 150);
}

/// The plate's icon: 1x1-cell art is doubled to fill the 16x16 well, anything bigger
/// (held furniture is 2x2 cells = 16x16px) already fits at 1x. Empty hands get the fist.
fn draw_plate_icon(screen: &mut Screen, sheet: &SpriteSheet, l: HudLayout, active: Option<&Item>) {
    let (x, y) = (l.plate_x + 1, l.plate_y + 1);
    match active {
        Some(item) => {
            let cells = (
                item.sprite.sprite_pixels.len(),
                item.sprite.sprite_pixels[0].len(),
            );
            if cells == (1, 1) {
                render_sprite_2x(screen, sheet, &item.sprite, x, y);
            } else {
                item.sprite.render(screen, x, y);
            }
        }
        None => render_fist(screen, x, y),
    }
}

/// Traffic-light durability shade: green -> amber at 50% -> red at 20% remaining.
fn durability_shade(frac: f32) -> i32 {
    if frac > 0.5 {
        140 // green
    } else if frac > 0.2 {
        540 // amber
    } else {
        500 // red
    }
}

/// The durability gauge under the plate (it replaced the numeric % readout). A tool with
/// any durability left keeps at least 1px of fill, so "one hit left" never renders as
/// the same empty lane as "broken".
fn draw_durability_bar(screen: &mut Screen, l: HudLayout, active: Option<&Item>) {
    let Some(ItemKind::Tool { ttype, level, dur }) = active.map(|i| &i.kind) else {
        return;
    };
    let max = (ttype.durability() * (level + 1)).max(1);
    let frac = (*dur).clamp(0, max) as f32 / max as f32;
    let fill = color::upgrade(color::get_byte(durability_shade(frac)));
    let empty = color::upgrade(color::get_byte(111));
    screen.fill_rect(l.plate_x, l.plate_y + 18, 18, 3, 0x000000);
    screen.fill_rect(l.plate_x + 1, l.plate_y + 19, 16, 1, empty);
    let min_fw = if *dur > 0 { 1 } else { 0 };
    let fw = ((16.0 * frac).round() as i32).clamp(min_fw, 16);
    screen.fill_rect(l.plate_x + 1, l.plate_y + 19, fw, 1, fill);
}

/// Count badge above the plate: stack sizes and ammo only. No bow, no counter — the
/// permanent `X0` arrow readout is gone. `^` means effectively infinite.
fn count_badge_text(g: &Game, active: Option<&Item>) -> Option<String> {
    let item = active?;
    match &item.kind {
        ItemKind::Tool { ttype, .. } => {
            let ammo = match ttype {
                ToolType::Bow | ToolType::Crossbow => Some(crate::item::registry::arrow_item(g)),
                ToolType::Slingshot => Some(crate::item::registry::get(g, "Stone")),
                _ => None,
            };
            ammo.map(|it| {
                let n = g.player().player().inventory.count(&it);
                if g.is_mode("creative") || n >= 10000 {
                    "^".to_string()
                } else {
                    n.min(9999).to_string()
                }
            })
        }
        _ if item.is_stackable() => Some(item.count().min(999).to_string()),
        _ => None,
    }
}

fn draw_count_badge(screen: &mut Screen, l: HudLayout, text: &str) {
    let w = font::text_width(text);
    let tx = l.w - 6 - w;
    screen.darken_rect_screen(tx - 1, l.plate_y - 9, w + 3, 10, 150);
    draw_text_shadowed(screen, text, tx, l.plate_y - 8, color::get(-1, 555));
}

/// The transient held-item name: up for ~90 frames after a switch, dimmed for its last
/// stretch, then gone. Because the persistent plate state is icon + gauge, the old
/// truncation ("1 LEATHER..") is impossible by construction.
fn draw_held_label(screen: &mut Screen, l: HudLayout, name: &str, ticks: i32) {
    let w = font::text_width(name);
    let tx = (l.w - 4 - w).max(2);
    let ty = l.badge_y + 1;
    screen.darken_rect_screen(tx - 2, ty - 1, w + 4, 10, 150);
    let col = if ticks < 20 {
        color::get(-1, 333) // fading out
    } else {
        color::get(-1, 555)
    };
    draw_text_shadowed(screen, name, tx, ty, col);
}

impl Renderer {
    pub fn new(sheet: Arc<SpriteSheet>) -> Renderer {
        Renderer {
            flyover: None,
            screen: Screen::new(sheet.clone()),
            light_screen: Screen::new(sheet.clone()),
            sheet,
            hud: HudMem::new(),
        }
    }

    /// Replace the two per-frame buffers without disturbing game or flyover state.
    pub fn resize(&mut self, w: i32, h: i32) {
        if self.screen.w == w && self.screen.h == h {
            return;
        }
        self.screen = Screen::with_size(w, h, self.sheet.clone());
        self.light_screen = Screen::with_size(w, h, self.sheet.clone());
    }

    /// Java `Renderer.render()` — called from the game loop, a bit after tick().
    pub fn render(&mut self, g: &mut Game) {
        g.screen_size = (self.screen.w, self.screen.h);
        if !g.has_gui {
            return; // no point in this if there's no gui
        }

        if g.ready_to_render_gameplay {
            // (isValidServer branch removed — always singleplayer)
            self.flyover = None;
            self.render_level(g);
            self.render_gui(g);
        } else if g.display.menu_active() {
            self.render_flyover(g);
        }

        // renders menu, if present (top display only, as in Java)
        if let Some(mut top) = g.display.stack.pop() {
            g.display.taken_out = true;
            top.render(&mut self.screen, g);
            g.display.taken_out = false;
            g.display.stack.push(top);
        }

        if !g.has_focus && !g.is_online() && !g.continous {
            self.render_focus_nagger(g);
        }
    }

    /// The title-screen backdrop: generate a throwaway infinite surface and drift a
    /// camera over it, like a drone flyover. Torn down as soon as gameplay renders.
    fn render_flyover(&mut self, g: &mut Game) {
        const LVL: usize = 3; // surface slot; reset_game/init_world reclaim it later

        if self.flyover.is_none() {
            let seed = g.random.next_long();
            // (heading doubles as the frame counter for the pan cadence)
            // a fresh menu-world level in the surface slot (only when no game is loaded)
            let mut level = crate::level::Level::empty(128, 128, 0, 1);
            level.chunks = Some(crate::level::chunk::ChunkMap::default());
            g.levels[LVL] = Some(level);
            g.world_seed = seed;
            // start over land so the shot opens on terrain, not open ocean
            let (sx, sy) = crate::level::infinite_gen::find_surface_spawn(seed, &g.tiles);
            self.flyover = Some(Flyover {
                seed,
                cam_x: (sx * 16) as f64,
                cam_y: (sy * 16) as f64,
                heading: 0.0,
            });
        }
        let Some(fly) = self.flyover.as_mut() else {
            return;
        };
        if g.levels[LVL].as_ref().is_none_or(|l| !l.is_infinite()) {
            // a real world took the slot (loading screen etc.) — stop flying
            self.flyover = None;
            return;
        }

        // Smooth pan: exactly one pixel every other frame (a regular cadence reads far
        // smoother than fractional speeds, which step at irregular intervals), plus a
        // very slow north/south wander.
        fly.heading += 1.0; // frame counter (repurposed field)
        if fly.heading as u64 % 2 == 0 {
            fly.cam_x += 1.0;
        }
        fly.cam_y += (fly.heading * 0.004).sin() * 0.12;
        let (cx, cy) = (fly.cam_x as i32, fly.cam_y as i32);
        let _ = fly.seed;

        // spawn_structures = false: the flyover world is a throwaway — structure chests
        // would dirty its chunks, and dirty chunks persist to the current save dir
        crate::level::ensure_chunks_at(g, LVL, cx >> 4, cy >> 4, false);

        let x_scroll = cx - self.screen.w / 2;
        let y_scroll = cy - (self.screen.h - 8) / 2;
        crate::level::render_background(g, &mut self.screen, LVL, x_scroll, y_scroll);

        // dusk dimming, deepening smoothly toward the menu area: per-row brightness
        // ramps from 50% (top, showcases the world) to ~22% (bottom, text contrast)
        for y in 0..self.screen.h {
            let k: i32 = 128 - ((y - 40).clamp(0, 100) * 72) / 100; // 128 -> 56
            let row = (y * self.screen.w) as usize;
            for p in self.screen.pixels[row..row + self.screen.w as usize].iter_mut() {
                let r = (((*p >> 16) & 0xFF) * k) >> 8;
                let g2 = (((*p >> 8) & 0xFF) * k) >> 8;
                let b = ((*p & 0xFF) * k) >> 8;
                *p = (r << 16) | (g2 << 8) | b;
            }
        }
    }

    /// Java `renderLevel()`.
    fn render_level(&mut self, g: &mut Game) {
        let lvl = g.current_level;
        if g.levels[lvl].is_none() {
            return;
        }

        let (player_x, player_y) = match g.try_player() {
            Some(p) => (p.c.x, p.c.y),
            None => return,
        };

        let (lw, lh) = {
            let level = g.level(lvl);
            (level.w, level.h)
        };

        let mut x_scroll = player_x - self.screen.w / 2; // scrolls the screen in the x axis
        let mut y_scroll = player_y - (self.screen.h - 8) / 2; // scrolls the screen in the y axis

        // stop scrolling at the borders (finite levels only; infinite layers have none)
        if !g.level(lvl).is_infinite() {
            if x_scroll < 0 {
                x_scroll = 0;
            }
            if y_scroll < 0 {
                y_scroll = 0;
            }
            if x_scroll > lw * 16 - self.screen.w {
                x_scroll = lw * 16 - self.screen.w;
            }
            if y_scroll > lh * 16 - self.screen.h {
                y_scroll = lh * 16 - self.screen.h;
            }
        }
        if lvl > 3 {
            // Parallax backdrop behind the dungeon, scrolling at quarter speed. The
            // 0..7px offset means the 8px grid needs one extra cell past each edge to
            // cover the live framebuffer — sizing it to the classic screen left the
            // right/bottom of a 640x400 window unpainted.
            let col = color::get4(20, 20, 121, 121);
            let (ox, oy) = ((x_scroll / 4) & 7, (y_scroll / 4) & 7);
            let cols = (self.screen.w + ox) / 8 + 1;
            let rows = (self.screen.h + oy) / 8 + 1;
            for y in 0..rows {
                for x in 0..cols {
                    self.screen.render(x * 8 - ox, y * 8 - oy, 0, col, 0);
                }
            }
        }

        crate::level::render_background(g, &mut self.screen, lvl, x_scroll, y_scroll);
        // ground tint/seam treatment before anything stands on the ground — sprites
        // must never pick up seam stipple (see lighting::ground_pass)
        crate::gfx::lighting::ground_pass(&mut self.screen, g, lvl, x_scroll, y_scroll);
        // contact shadows sit on the ground, under the y-sorted sprite pass
        crate::gfx::ambience::contact_shadows(&mut self.screen, g, lvl, x_scroll, y_scroll);
        crate::level::render_sprites(g, &mut self.screen, lvl, x_scroll, y_scroll);

        // The lighting/atmosphere pass runs here — after the world, before render_gui
        // and menus — so darkness and color grading never touch UI text.
        crate::gfx::lighting::render_pass(
            &mut self.screen,
            &mut self.light_screen,
            g,
            lvl,
            x_scroll,
            y_scroll,
        );
    }

    /// The corner HUD (docs/UI_REDESIGN.md §2): frameless, need-to-know meters
    /// bottom-left, held-item plate bottom-right, notifications on their own tiers.
    /// Guiding rule: a meter that needs nothing from you does not exist.
    ///
    /// Draw order is load-bearing: the debug paragraphs sit under everything, the
    /// sleep banner suppresses the warning band, and the save toast goes last so it
    /// reads over the held plate.
    fn render_gui(&mut self, g: &mut Game) {
        let l = HudLayout::for_screen(self.screen.w, self.screen.h);
        let vitals = Vitals::sample(g);
        let active = g.player().player().active_item.clone();
        self.hud.update(&vitals, held_label(g, active.as_ref()));

        if g.debug && g.dev_overlay {
            crate::screen::dev_console::render_overlay(&mut self.screen, g);
        }
        self.render_debug_info(g);

        let perm_status = perm_status_lines(g);
        if !perm_status.is_empty() {
            draw_perm_status(&mut self.screen, &perm_status);
        }

        // Notifications are held — neither drawn nor aged — while a menu Display is
        // open, so they never bleed through the smoked-glass panels, and resume after
        // it closes.
        let menu_open = g.display.menu_active();
        if perm_status.is_empty() && !menu_open {
            draw_warning_band(&mut self.screen, g);
        }
        draw_ticker(&mut self.screen, g, menu_open);

        let pulse_on = (g.tick_count / 15) % 2 == 0;
        if !g.is_mode("creative") {
            // Creative mode shows the held plate only.
            let armor_flash = self.hud.armor_flash;
            draw_vitals(&mut self.screen, l, &vitals, &self.hud, pulse_on);
            draw_armor_chip(&mut self.screen, l, &vitals, armor_flash);
            let steps = crate::core::temperature::band_for(g, g.player()).steps();
            draw_temperature_badge(&mut self.screen, l, steps, pulse_on);
            draw_effect_pips(&mut self.screen, l, g);
        }

        let label_up = self.hud.label_ticks > 0;
        self.render_held_plate(g, l, active.as_ref());
        draw_save_toast(&mut self.screen, g, label_up);
    }

    /// The bottom-right held-item plate: frame, icon, durability gauge, count badge,
    /// and the transient name label. Persistent state is icon + gauge; everything
    /// text-shaped here is short-lived.
    fn render_held_plate(&mut self, g: &Game, l: HudLayout, active: Option<&Item>) {
        draw_plate_frame(&mut self.screen, l);
        draw_plate_icon(&mut self.screen, &self.sheet, l, active);
        draw_durability_bar(&mut self.screen, l, active);
        if let Some(text) = count_badge_text(g, active) {
            draw_count_badge(&mut self.screen, l, &text);
        }
        if self.hud.label_ticks > 0 {
            if let Some(name) = &self.hud.held_label {
                draw_held_label(&mut self.screen, l, name, self.hud.label_ticks);
            }
        }
    }

    /// Java `renderDebugInfo()` — the F3 overlay.
    fn render_debug_info(&mut self, g: &mut Game) {
        let screen = &mut self.screen;
        let textcol = color::WHITE;
        if !g.show_info {
            return;
        }
        let mut info: Vec<String> = Vec::new();
        info.push(format!("VERSION {}", game::version()));
        info.push(format!("{} fps", g.fra));
        info.push(format!("day tiks {} ({})", g.tick_count, g.get_time()));
        info.push(format!(
            "{} tik/sec",
            updater::NORM_SPEED as f32 * g.gamespeed
        ));
        {
            let p = g.player();
            info.push(format!("walk spd {}", p.player().move_speed));
            info.push(format!("X {}-{}", p.c.x / 16, p.c.x % 16));
            info.push(format!("Y {}-{}", p.c.y / 16, p.c.y % 16));
        }
        if g.levels[g.current_level].is_some() {
            let (px, py) = {
                let p = g.player();
                (p.c.x >> 4, p.c.y >> 4)
            };
            info.push(format!("Tile {}", g.tile_at(g.current_level, px, py).name));
            if g.is_mode("score") {
                info.push(format!("Score {}", g.player().player().get_score()));
            }
            let level = g.level(g.current_level);
            info.push(format!(
                "Mob Cnt {}/{}",
                level.mob_count, level.max_mob_count
            ));
        }

        // Displays number of chests left, if on dungeon level
        if g.levels[g.current_level].is_some() && g.current_level == 5 {
            if let Some(dungeon) = &g.levels[5] {
                if dungeon.chest_count > 0 {
                    info.push(format!("Chests: {}", dungeon.chest_count));
                } else {
                    info.push("Chests: Complete!".to_string());
                }
            }
        }

        {
            let p = g.player();
            let pd = p.player();
            info.push(format!("Hunger stam: {}", pd.get_debug_hunger()));
            if pd.armor > 0 {
                info.push(format!("armor: {}", pd.armor));
                info.push(format!("dam buffer: {}", pd.armor_damage_buffer));
            }
        }

        let mut style = FontStyle::new(textcol)
            .set_shadow_type(color::BLACK, true)
            .set_x_pos(1)
            .set_y_pos(2);
        font::draw_paragraph(&info, screen, &mut style, 2);
    }

    /// Java `renderFocusNagger()` — the "Come Back!" box when the window loses focus.
    fn render_focus_nagger(&mut self, g: &mut Game) {
        let msg = "Come Back!"; // the message when you click off the screen
        g.paused = true; // perhaps paused is only used for this
        let xx = (self.screen.w - font::text_width(msg)) / 2; // the width of the box
        let yy = (self.screen.h - 8) / 2; // the height of the box
        let w = msg.len() as i32; // length of message in characters
        let h = 1;
        let txtcolor = color::get4(-1, 1, color::hex("#2c2c2c"), 445);

        let screen = &mut self.screen;

        // renders the four corners of the box
        screen.render(xx - 8, yy - 8, 13 * 32, txtcolor, 0);
        screen.render(xx + w * 8, yy - 8, 13 * 32, txtcolor, 1);
        screen.render(xx - 8, yy + 8, 13 * 32, txtcolor, 2);
        screen.render(xx + w * 8, yy + 8, 13 * 32, txtcolor, 3);

        // renders each part of the box...
        for x in 0..w {
            screen.render(xx + x * 8, yy - 8, 1 + 13 * 32, txtcolor, 0); // ...top part
            screen.render(xx + x * 8, yy + 8, 1 + 13 * 32, txtcolor, 2); // ...bottom part
        }
        for y in 0..h {
            screen.render(xx - 8, yy + y * 8, 2 + 13 * 32, txtcolor, 0); // ...left part
            screen.render(xx + w * 8, yy + y * 8, 2 + 13 * 32, txtcolor, 1); // ...right part
        }

        // renders the focus nagger text with a flash effect...
        if (g.tick_count / 20) % 2 == 0 {
            font::draw(msg, screen, xx, yy, color::get(color::hex("#2c2c2c"), 333));
        } else {
            font::draw(msg, screen, xx, yy, color::get(color::hex("#2c2c2c"), 555));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::mob::player::MAX_STAT;

    /// The HUD is corner-anchored, so every slot must track the live framebuffer size.
    /// A slot pinned to the classic 192 would float mid-screen at 400 tall (G10).
    #[test]
    fn hud_slots_follow_the_live_screen_size() {
        for (w, h) in [(288, 192), (320, 240), (400, 300), (640, 400)] {
            let l = HudLayout::for_screen(w, h);
            // vitals stack keeps its order and 8px pitch, above the bottom edge
            assert!(l.badge_y < l.hearts_y, "{w}x{h}");
            assert!(l.hearts_y < l.stamina_y, "{w}x{h}");
            assert!(l.stamina_y < l.food_y, "{w}x{h}");
            assert_eq!(l.stamina_y - l.hearts_y, 8, "{w}x{h}");
            assert_eq!(l.food_y - l.stamina_y, 8, "{w}x{h}");
            // every slot lands inside the framebuffer, measured from the far edges
            assert_eq!(h - l.food_y, 18, "{w}x{h}");
            assert_eq!(w - l.plate_x, 22, "{w}x{h}");
            assert_eq!(h - l.plate_y, 22, "{w}x{h}");
            assert!(l.badge_y > 0, "{w}x{h}");
            // the plate and its 3px durability lane stay on screen
            assert!(l.plate_x + 18 <= w, "{w}x{h}");
            assert!(l.plate_y + 21 <= h, "{w}x{h}");
        }
    }

    /// The classic 288x192 geometry the mocks were measured against, pinned exactly.
    #[test]
    fn hud_slots_match_the_classic_mock_geometry() {
        let l = HudLayout::for_screen(288, 192);
        assert_eq!(
            (l.hearts_y, l.stamina_y, l.food_y, l.badge_y),
            (158, 166, 174, 146)
        );
        assert_eq!((l.plate_x, l.plate_y), (266, 170));
    }

    /// The armor chip docks flush against the hearts strip; both derive from MAX_STAT
    /// rather than the hand-counted 86/88/97 the chip used to carry.
    #[test]
    fn armor_chip_docks_against_the_vitals_strip() {
        assert_eq!(VITALS_STRIP_W, MAX_STAT * 8 + 4);
        assert_eq!(VITALS_X + VITALS_STRIP_W, 86); // the old literal
    }

    /// Band colors are the visible contract of core::temperature; the outer bands
    /// alternate with the pulse clock and the inner ones hold steady.
    #[test]
    fn temperature_band_colors_are_pinned() {
        for pulse in [false, true] {
            assert_eq!(temp_band_rgb(-1, pulse), 0x5E8FD4);
            assert_eq!(temp_band_rgb(-2, pulse), 0x3E6FE0);
            assert_eq!(temp_band_rgb(1, pulse), 0xD9A85A);
            assert_eq!(temp_band_rgb(2, pulse), 0xE07E33);
        }
        assert_eq!(temp_band_rgb(-3, true), 0x8FB4FF);
        assert_eq!(temp_band_rgb(-3, false), 0x2B4FF0);
        assert_eq!(temp_band_rgb(-9, true), 0x8FB4FF);
        assert_eq!(temp_band_rgb(3, true), 0xFF9A66);
        assert_eq!(temp_band_rgb(3, false), 0xE0491F);
        assert_eq!(temp_band_rgb(9, false), 0xE0491F);
    }

    #[test]
    fn durability_shades_cross_at_50_and_20_percent() {
        assert_eq!(durability_shade(1.0), 140);
        assert_eq!(durability_shade(0.51), 140);
        assert_eq!(durability_shade(0.5), 540); // boundary belongs to amber
        assert_eq!(durability_shade(0.21), 540);
        assert_eq!(durability_shade(0.2), 500); // boundary belongs to red
        assert_eq!(durability_shade(0.0), 500);
    }
}
