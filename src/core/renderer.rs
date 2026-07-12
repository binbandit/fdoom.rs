//! Port of `fdoom.core.Renderer` — draws each frame into the 288x192 software screen.
//! The platform layer scales/blits `self.screen.pixels` to the window.

use std::sync::Arc;

use crate::core::game::{self, Game};
use crate::core::updater;
use crate::entity::furniture::bed_behavior;
use crate::gfx::screen::{self, Screen};
use crate::gfx::sprite_sheet::SpriteSheet;
use crate::gfx::{Dimension, FontStyle, Point, color, font};
use crate::item::ItemKind;
use crate::screen::RelPos;

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
}

/// Fill a screen-space rect with a literal RGB color (bounds-clipped). The HUD's
/// durability bar needs a flat fill, which no sprite-cell primitive provides.
fn fill_rect_screen(screen: &mut Screen, x: i32, y: i32, w: i32, h: i32, rgb: i32) {
    for yy in y.max(0)..(y + h).min(screen::H) {
        let row = (yy * screen::W) as usize;
        for xx in x.max(0)..(x + w).min(screen::W) {
            screen.pixels[row + xx as usize] = rgb;
        }
    }
}

impl Renderer {
    pub fn new(sheet: Arc<SpriteSheet>) -> Renderer {
        Renderer {
            flyover: None,
            screen: Screen::new(sheet.clone()),
            light_screen: Screen::new(sheet),
        }
    }

    /// Java `Renderer.render()` — called from the game loop, a bit after tick().
    pub fn render(&mut self, g: &mut Game) {
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

        let x_scroll = cx - screen::W / 2;
        let y_scroll = cy - (screen::H - 8) / 2;
        crate::level::render_background(g, &mut self.screen, LVL, x_scroll, y_scroll);

        // dusk dimming, deepening smoothly toward the menu area: per-row brightness
        // ramps from 50% (top, showcases the world) to ~22% (bottom, text contrast)
        for y in 0..screen::H {
            let k: i32 = 128 - ((y - 40).clamp(0, 100) * 72) / 100; // 128 -> 56
            let row = (y * screen::W) as usize;
            for p in self.screen.pixels[row..row + screen::W as usize].iter_mut() {
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

        let mut x_scroll = player_x - screen::W / 2; // scrolls the screen in the x axis
        let mut y_scroll = player_y - (screen::H - 8) / 2; // scrolls the screen in the y axis

        // stop scrolling at the borders (finite levels only; infinite layers have none)
        if !g.level(lvl).is_infinite() {
            if x_scroll < 0 {
                x_scroll = 0;
            }
            if y_scroll < 0 {
                y_scroll = 0;
            }
            if x_scroll > lw * 16 - screen::W {
                x_scroll = lw * 16 - screen::W;
            }
            if y_scroll > lh * 16 - screen::H {
                y_scroll = lh * 16 - screen::H;
            }
        }
        if lvl > 3 {
            // sky (and dungeon) background
            let col = color::get4(20, 20, 121, 121);
            for y in 0..28 {
                for x in 0..48 {
                    self.screen.render(
                        x * 8 - ((x_scroll / 4) & 7),
                        y * 8 - ((y_scroll / 4) & 7),
                        0,
                        col,
                        0,
                    );
                }
            }
        }

        crate::level::render_background(g, &mut self.screen, lvl, x_scroll, y_scroll);
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

    /// Java `renderGui()` — hearts, stamina, hunger, item bar, notifications...
    fn render_gui(&mut self, g: &mut Game) {
        let screen = &mut self.screen;

        // This is the box for the arrows and durability
        font::render_frame(screen, "", 26, 0, 35, 2);

        if g.debug && g.dev_overlay {
            crate::screen::dev_console::render_overlay(screen, g);
        }

        self.render_debug_info(g);
        let screen = &mut self.screen;

        // Arrow counter. ^ = infinite symbol.
        let arrow_item = crate::item::registry::arrow_item(g);
        let ac = g.player().player().inventory.count(&arrow_item);
        if g.is_mode("creative") || ac >= 10000 {
            font::draw(
                "\tx^",
                screen,
                WIDTH - 70,
                8,
                color::get4(-1, 333, 444, 555),
            );
        } else {
            font::draw(
                &format!("\tx{ac}"),
                screen,
                WIDTH - 70,
                8,
                color::get(-1, 555),
            );
        }
        // displays arrow icon
        screen.render(
            WIDTH - 72,
            7,
            13 + 5 * 32,
            color::get4(-1, 111, 222, 430),
            0,
        );

        let mut perm_status: Vec<String> = Vec::new();
        if bed_behavior::sleeping(g) {
            perm_status.push("Sleeping...".to_string());
        } else if g.bed_state.players_awake > 0 && bed_behavior::in_bed(g, g.player_id) {
            let num_awake = g.bed_state.players_awake;
            perm_status.push(crate::core::my_utils::plural(num_awake, "player") + " still awake");
            perm_status.push(" ".to_string());
            perm_status.push(format!("Press {} to cancel", g.input.get_mapping("exit")));
        }

        if !perm_status.is_empty() {
            let mut style = FontStyle::new(color::WHITE)
                .set_y_pos(screen::H / 2 - 25)
                .set_rel_text_pos(RelPos::Top)
                .set_shadow_type(color::DARK_GRAY, false);
            font::draw_paragraph(&perm_status, screen, &mut style, 1);
        }

        // NOTIFICATIONS (playtest #2). Two tiers: warnings/event cues keep the centered
        // band; ambient chatter docks top-left under the HUD as a compact ticker. Both
        // are held — neither drawn nor aged — while a menu Display is open, so they
        // never bleed through the smoked-glass panels and resume after close.
        let menu_open = g.display.menu_active();

        // WARNING / EVENT band — centered, loud on purpose.
        if perm_status.is_empty() && !menu_open && !g.warnings.is_empty() {
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

            // draw each current warning, with shadow text effect
            let mut style = FontStyle::new(color::WHITE)
                .set_shadow_type(color::DARK_GRAY, false)
                .set_y_pos(screen::H * 2 / 5)
                .set_rel_text_pos_both(RelPos::Top, false);
            let notes = g.warnings.clone();
            // smoked-glass backing band (same primitive as the menu panels) so the text
            // reads over any terrain; mirrors the geometry FontStyle computes above
            let size = Dimension::new(
                font::text_width_para(&notes),
                notes.len() as i32 * font::text_height(),
            );
            let band =
                RelPos::Top.position_rect(size, Point::new(screen::W / 2, screen::H * 2 / 5));
            screen.darken_rect_screen(
                band.left() - 4,
                band.top() - 3,
                band.width() + 8,
                band.height() + 5,
                185,
            );
            font::draw_paragraph(&notes, screen, &mut style, 0);
        }

        // AMBIENT ticker — top-left under the HUD frames, newest line on top, ~90
        // ticks each, max 3 lines. Small presence: the font is caps-only (the CHARS
        // lowercase range maps past the stitched glyphs), so quiet placement and a
        // faint backing do the de-emphasis instead of sentence case.
        g.sync_note_ages();
        if !menu_open && !g.notifications.is_empty() {
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
            const TICKER_Y: i32 = 42; // just under the health/item HUD frames
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

        // SAVE TOAST — bottom-right, small: live progress while saving, then the
        // "World Saved!" toast pushed by the save path.
        if !g.saving && g.toast.is_some() {
            g.toast_tick += 1;
            if g.toast_tick > 90 {
                g.toast = None;
            }
        }
        let toast_line = if g.saving {
            Some(format!("Saving... {}%", g.loading_percentage.round()))
        } else {
            g.toast.clone()
        };
        if let Some(line) = toast_line {
            let w = font::text_width(&line);
            let (tx, ty) = (screen::W - w - 4, screen::H - 12);
            screen.darken_rect_screen(tx - 2, ty - 1, w + 4, 10, 185);
            font::draw(&line, screen, tx, ty, color::get(-1, 555));
        }

        // TOOL DURABILITY STATUS
        if let Some(item) = &g.player().player().active_item {
            if let ItemKind::Tool { ttype, level, dur } = &item.kind {
                let dura = dur * 100 / (ttype.durability() * (level + 1));
                font::draw(
                    &format!("{dura}%"),
                    screen,
                    WIDTH - 38,
                    8,
                    color::get(-1, 30),
                );
            }
        }

        // This renders the potions overlay
        {
            let pd = g.player().player();
            if pd.showpotioneffects && !pd.potioneffects.is_empty() {
                let effects: Vec<(crate::item::PotionType, i32)> =
                    pd.potioneffects.iter().map(|(k, v)| (*k, *v)).collect();
                // the key is potion type, value is remaining potion duration
                for (i, (ptype, time)) in effects.iter().enumerate() {
                    let p_time = time / updater::NORM_SPEED;
                    let pcol = color::get(ptype.disp_color(), 555);
                    font::draw(
                        &format!("({} to hide!)", g.input.get_mapping("potionEffects")),
                        screen,
                        180,
                        9,
                        color::get(0, 555),
                    );
                    font::draw(
                        &format!("{} ({}:{})", ptype, p_time / 60, p_time % 60),
                        screen,
                        180,
                        17 + i as i32 * font::text_height(),
                        pcol,
                    );
                }
            }
        }

        // Status icons: health hearts, stamina bolts, and hunger "burgers".
        if !g.is_mode("creative") {
            // Health box + selected item box frames
            font::render_frame(screen, "", 0, 0, 10, 4);
            font::render_frame(screen, "", 11, 0, 25, 2);

            let (health, stamina, stamina_recharge_delay, hunger, armor, cur_armor_color) = {
                let p = g.player();
                let pd = p.player();
                (
                    pd.mob.health,
                    pd.stamina,
                    pd.stamina_recharge_delay,
                    pd.hunger,
                    pd.armor,
                    pd.cur_armor.as_ref().map(|a| a.sprite.color),
                )
            };

            for i in 0..crate::entity::mob::player::MAX_STAT {
                // renders armor
                let armor_amt = armor * crate::entity::mob::player::MAX_STAT
                    / crate::entity::mob::player::MAX_ARMOR;
                let col = match cur_armor_color {
                    Some(c) if i <= armor_amt => c,
                    _ => color::get(-1, -1),
                };
                screen.render(i * 8, screen::H - 24, 3 + 12 * 32, col, 0);

                // renders current red hearts, or black hearts for damaged health
                let col = if i < health {
                    color::get4(-1, 200, 500, 533)
                } else {
                    color::get4(-1, 100, 0, 0)
                };
                screen.render(i * 8 + 4, 4, 12 * 32, col, 0);

                if stamina_recharge_delay > 0 {
                    // the white/gray blinking effect when you run out of stamina
                    let col = if stamina_recharge_delay / 4 % 2 == 0 {
                        color::get4(-1, 555, 0, 0)
                    } else {
                        color::get4(-1, 110, 0, 0)
                    };
                    screen.render(i * 8 + 4, 8 + 5, 1 + 12 * 32, col, 0);
                } else {
                    // current stamina, and uncharged gray stamina
                    let col = if i < stamina {
                        color::get4(-1, 220, 550, 553)
                    } else {
                        color::get4(-1, 110, 0, 0)
                    };
                    screen.render(i * 8 + 4, 8 + 5, 1 + 12 * 32, col, 0);
                }

                // renders hunger
                let col = if i < hunger {
                    color::get4(-1, 100, 530, 211)
                } else {
                    color::get4(-1, 100, 0, 0)
                };
                screen.render(i * 8 + 4, 8 + 5 + 8, 2 + 12 * 32, col, 0);
            }

            // TEMPERATURE dot (temperature wave, core::temperature): one small
            // indicator pinned to the frame edge at the end of the stamina row —
            // blue grading into amber, pulsing in the extreme bands, and not drawn
            // at all inside the comfort band (no meter when everything's fine).
            let steps = crate::core::temperature::band_for(g, g.player()).steps();
            if steps != 0 {
                let pulse = (g.tick_count / 15) % 2 == 0;
                let rgb = match steps {
                    -1 => 0x5E8FD4,
                    -2 => 0x3E6FE0,
                    i32::MIN..=-3 => {
                        if pulse {
                            0x8FB4FF
                        } else {
                            0x2B4FF0
                        }
                    }
                    1 => 0xD9A85A,
                    2 => 0xE07E33,
                    _ => {
                        if pulse {
                            0xFF9A66
                        } else {
                            0xE0491F
                        }
                    }
                };
                // on the frame-border seam at the end of the stamina row (x 84..91
                // sits past the last bolt at x<=83 and clear of both frames' content)
                fill_rect_screen(screen, 84, 13, 7, 7, 0x000000);
                fill_rect_screen(screen, 85, 14, 5, 5, rgb);
            }
        }

        // CURRENT ITEM — name clipped to the held-item frame (tiles 11..=25, inner
        // pixels end at 200) so long names never bleed into the arrow/durability box
        // (playtest #9 / bug #3).
        const ITEM_SPRITE_X: i32 = 12 * 7 + 10; // 94, inside the frame's left border
        const ITEM_TEXT_X: i32 = ITEM_SPRITE_X + 8;
        const ITEM_NAME_CHARS: usize = ((200 - ITEM_TEXT_X) / 8) as usize; // 12
        let active = g.player().player().active_item.clone();
        if let Some(item) = active {
            // shows active item sprite and clipped name in the top toolbar
            item.sprite.render(&mut self.screen, ITEM_SPRITE_X, 8);
            let name = item.get_display_name(g);
            let name = if name.chars().count() > ITEM_NAME_CHARS {
                name.chars().take(ITEM_NAME_CHARS - 2).collect::<String>() + ".."
            } else {
                name
            };
            font::draw(&name, &mut self.screen, ITEM_TEXT_X, 8, color::get(-1, 555));

            // HELD-TOOL DURABILITY BAR: a thin gauge under the toolbar item display,
            // green -> yellow -> red as the remaining durability fraction drops.
            if let ItemKind::Tool { ttype, level, dur } = &item.kind {
                let screen = &mut self.screen;
                let max = (ttype.durability() * (level + 1)).max(1);
                let frac = (*dur).clamp(0, max) as f32 / max as f32;
                let readable = if frac > 0.5 {
                    140 // green
                } else if frac > 0.25 {
                    540 // yellow
                } else {
                    500 // red
                };
                let fill = color::upgrade(color::get_byte(readable));
                let empty = color::upgrade(color::get_byte(111));
                let (bx, by, bw) = (96, 17, 104); // inside the item frame's bottom border
                fill_rect_screen(screen, bx - 1, by - 1, bw + 2, 4, 0x000000);
                fill_rect_screen(screen, bx, by, bw, 2, empty);
                let fw = ((bw as f32 * frac).round() as i32).clamp(0, bw);
                fill_rect_screen(screen, bx, by, fw, 2, fill);
            }
        } else if !g.is_mode("creative") {
            // empty hands: a small dim dash centered in the box, so it reads
            // "nothing held" rather than a broken blank panel
            font::draw("-", &mut self.screen, 144, 8, color::get(-1, 222));
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
        let xx = (screen::W - font::text_width(msg)) / 2; // the width of the box
        let yy = (HEIGHT - 8) / 2; // the height of the box
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
