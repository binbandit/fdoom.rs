//! Port of `fdoom.core.Renderer` — draws each frame into the 288x192 software screen.
//! The platform layer scales/blits `self.screen.pixels` to the window.

use std::sync::Arc;

use crate::core::game::{self, Game};
use crate::core::updater;
use crate::entity::furniture::bed_behavior;
use crate::gfx::ellipsis::Ellipsis;
use crate::gfx::screen::{self, Screen};
use crate::gfx::sprite_sheet::SpriteSheet;
use crate::gfx::{FontStyle, color, font};
use crate::item::ItemKind;
use crate::screen::RelPos;

pub const HEIGHT: i32 = screen::H;
pub const WIDTH: i32 = screen::W;

pub struct Renderer {
    pub screen: Screen,
    /// The darkness/fog-of-war overlay screen (JAVA: the overlay call is commented out in
    /// this fork, but the screen is still constructed).
    pub light_screen: Screen,
    #[allow(dead_code)]
    ellipsis: Ellipsis,
}

impl Renderer {
    pub fn new(sheet: Arc<SpriteSheet>) -> Renderer {
        Renderer {
            screen: Screen::new(sheet.clone()),
            light_screen: Screen::new(sheet),
            ellipsis: Ellipsis::smooth_tick(),
        }
    }

    /// Java `Renderer.render()` — called from the game loop, a bit after tick().
    pub fn render(&mut self, g: &mut Game) {
        if !g.has_gui {
            return; // no point in this if there's no gui
        }

        if g.ready_to_render_gameplay {
            // (isValidServer branch removed — always singleplayer)
            self.render_level(g);
            self.render_gui(g);
        }

        // renders menu, if present (top display only, as in Java)
        if let Some(mut top) = g.display.stack.pop() {
            top.render(&mut self.screen, g);
            g.display.stack.push(top);
        }

        if !g.has_focus && !g.is_online() && !g.continous {
            self.render_focus_nagger(g);
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

        // stop scrolling at the borders
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
        crate::level::render_sprites(g, &mut self.screen, lvl, x_scroll, y_scroll);

        // JAVA: the cave-darkness light overlay is commented out in this fork; preserved
        // as disabled (see Renderer.java renderLevel).
    }

    /// Java `renderGui()` — hearts, stamina, hunger, item bar, notifications...
    fn render_gui(&mut self, g: &mut Game) {
        let screen = &mut self.screen;

        // This is the box for the arrows and durability
        font::render_frame(screen, "", 26, 0, 35, 2);

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
        if g.saving {
            perm_status.push(format!("Saving... {}%", g.loading_percentage.round()));
        }
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

        // NOTIFICATIONS
        if perm_status.is_empty() && !g.notifications.is_empty() {
            g.note_tick += 1;
            if g.notifications.len() > 3 {
                // only show 3 notifs max at one time; erase old notifs
                let start = g.notifications.len() - 3;
                g.notifications = g.notifications[start..].to_vec();
            }

            if g.note_tick > 120 {
                // display time per notification
                g.notifications.remove(0);
                g.note_tick = 0;
            }

            // draw each current notification, with shadow text effect
            let mut style = FontStyle::new(color::WHITE)
                .set_shadow_type(color::DARK_GRAY, false)
                .set_y_pos(screen::H * 2 / 5)
                .set_rel_text_pos_both(RelPos::Top, false);
            let notes = g.notifications.clone();
            font::draw_paragraph(&notes, screen, &mut style, 0);
        }

        // SCORE MODE ONLY:
        if g.is_mode("score") {
            let seconds = (g.score_time as f64 / updater::NORM_SPEED as f64).ceil() as i32;
            let minutes = seconds / 60;
            let hours = minutes / 60;
            let minutes = minutes % 60;
            let seconds = seconds % 60;

            let time_col = if g.score_time >= 18000 {
                color::get(0, 555)
            } else if g.score_time >= 3600 {
                color::get(330, 555)
            } else {
                color::get(400, 555)
            };

            font::draw(
                &format!(
                    "Time left {}{}m {}s",
                    if hours > 0 {
                        format!("{hours}h ")
                    } else {
                        String::new()
                    },
                    minutes,
                    seconds
                ),
                screen,
                screen::W / 2 - 9 * 8,
                2,
                time_col,
            );

            let score_mode = g.is_mode("score");
            let score_string = format!("Current score: {}", g.player().player().get_score());
            font::draw(
                &score_string,
                screen,
                screen::W - font::text_width(&score_string) - 2,
                3 + 8,
                color::WHITE,
            );

            let mult = g.player().player().get_multiplier(score_mode);
            if mult > 1 {
                let mult_color = if mult < crate::entity::mob::player::MAX_MULTIPLIER {
                    color::get(-1, 540)
                } else {
                    color::RED
                };
                let mult_str = format!("X{mult}");
                font::draw(
                    &mult_str,
                    screen,
                    screen::W - font::text_width(&mult_str) - 2,
                    4 + 2 * 8,
                    mult_color,
                );
            }
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
        }

        // CURRENT ITEM
        let active = g.player().player().active_item.clone();
        if let Some(item) = active {
            // shows active item sprite and name in bottom toolbar
            item.render_inventory(&mut self.screen, g, 12 * 7 + 10, 8, false);
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

    /// Access for gameplay screens that need the ellipsis (server waiting screen).
    pub fn ellipsis_update(&mut self, tick_count: i32) -> String {
        self.ellipsis.update_and_get(tick_count)
    }
}
