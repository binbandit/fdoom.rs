//! Player behavior — tick/attack/render/hurt/death. The data + constructor live in
//! `player.rs`. This build is singleplayer-only: the multiplayer client/server branches
//! of the original were never ported (see PORTING.md "Multiplayer").

use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::core::temperature;
use crate::core::updater::Time;
use crate::core::weather;
use crate::entity::behavior::{
    entity_interact, get_attack_dir, is_swimming, mob_do_hurt_base, mob_hurt_by_mob, mob_move,
    mob_tick_base, remove_entity,
};
use crate::entity::furniture::{bed_behavior, behavior as furniture_behavior, death_chest};
use crate::entity::mob::player::{
    ATTACK_DIST, CARRY_SPRITES, CARRY_SUIT_SPRITES, HUNGER_STEP_COUNT, HUNGER_TICK_COUNT,
    INTERACT_DIST, MAX_HEALTH, MAX_HUNGER, MAX_HUNGER_STAMS, MAX_HUNGER_TICKS, MAX_STAMINA,
    MAX_STAMINA_RECHARGE, MIN_STARVE_HEALTH, PLAYER_HURT_TIME, SPRITES, SUIT_SPRITES,
};
use crate::entity::particle::{
    new_bobber_particle, new_material_puff, new_smash_particle, new_text_particle,
};
use crate::entity::projectile::{ProjectileStyle, new_arrow, new_thrown};
use crate::entity::{Direction, Entity, EntityKind};
use crate::gfx::{MobAnims, Point, Rectangle, Screen, color};
use crate::item::{Item, ItemKind, PotionType, ToolType, interact as item_interact, registry};
use crate::level;
use crate::level::tile::TileKind;
use crate::level::tile::dispatch as tiles;
use crate::rng::Rng;

// ---- Post-port ranged/thrown weapon tuning (see docs/ITEMS_AND_CRAFTING.md) ----
/// Crossbow bolt damage — vs the Bow, whose bolt damage is its 0..=5 tool tier.
const CROSSBOW_DAMAGE: i32 = 7;
/// `attack_time` set on a crossbow shot; it doubles as the re-cock delay (a click while
/// `attack_time > 0` is a dry trigger pull).
const CROSSBOW_COOLDOWN: i32 = 30;
/// Slingshot pellets ride on the arrow-vs-mob +3/+1 bonus alone — the weak opener.
const SLINGSHOT_DAMAGE: i32 = 0;
const PELLET_RANGE_TICKS: i32 = 12;
const KNIFE_DAMAGE: i32 = 2;
const KNIFE_RANGE_TICKS: i32 = 15;
/// Thrown spear damage = base + 2 per tool tier.
const SPEAR_THROW_BASE: i32 = 2;
const SPEAR_RANGE_TICKS: i32 = 16;
/// Extra melee reach (px past `ATTACK_DIST`) for a held spear.
const SPEAR_REACH_BONUS: i32 = 8;
const THROWING_KNIFE: &str = "Throwing Knife";

// ---- Attack & interaction juice (playtest #1) ----
/// `swing_flash` start value. It ticks down in the same block as `attack_time`
/// (including on the attack tick itself), so renders see 3, 2, 1 — three frames of
/// sweep arc traveling into the facing tile.
const SWING_FLASH_START: i32 = 4;

/// Java `Player.tick()`.
pub fn tick(g: &mut Game, e: &mut Entity) {
    if e.c.level.is_none() || e.c.removed {
        return;
    }

    // Refresh the inventory's creative-mode flag (Java's anonymous Inventory subclass in
    // the Player constructor read Game.isMode("creative") live; see inventory.rs).
    e.player_mut().inventory.creative = g.is_mode("creative");

    // don't tick the player while a menu is open (the Updater still ticks the entity)
    if g.menu_open() {
        return;
    }

    // shared mob tick; returns false if the player was removed (died)
    if !mob_tick_base(g, e) {
        return;
    }

    {
        let paused = g.paused;
        e.player_mut().tick_multiplier(paused);
    }

    if !e.player().potioneffects.is_empty() && !bed_behavior::in_bed(g, e.c.eid) {
        // snapshot the keys: apply_potion mutates the map mid-iteration
        let keys: Vec<PotionType> = e.player().potioneffects.keys().copied().collect();
        for potion_type in keys {
            let time = e
                .player()
                .potioneffects
                .get(&potion_type)
                .copied()
                .unwrap_or(0);
            if time <= 1 {
                // if time is zero (going to be set to 0 in a moment)...
                // automatically removes this potion effect.
                item_interact::apply_potion(g, e, potion_type, false);
            } else {
                // otherwise, replace it with one less.
                e.player_mut().potioneffects.insert(potion_type, time - 1);
            }
        }
    }

    if e.player().cooldowninfo > 0 {
        e.player_mut().cooldowninfo -= 1;
    }

    if g.input.get_key("potionEffects").clicked && e.player().cooldowninfo == 0 {
        let pd = e.player_mut();
        pd.cooldowninfo = 10;
        pd.showpotioneffects = !pd.showpotioneffects;
    }

    // First-day thread, cue 1: brand-new worlds (world.rs arms the delay) point at
    // tall grass about a minute in — unless the player already found fibers.
    if e.player().grass_cue_delay > 0 {
        let due = {
            let pd = e.player_mut();
            pd.grass_cue_delay -= 1;
            pd.grass_cue_delay == 0 && !pd.fiber_cue_done
        };
        if due
            && e.player()
                .inventory
                .count(&crate::item::registry::get(g, "Grass Fibers"))
                == 0
        {
            g.push_cue("The tall grass holds fibers.");
        }
    }

    let lvl = e.c.level.expect("player must be on a level");
    // gets the current tile the player is on.
    let on_tile = g.tile_at(lvl, e.c.x >> 4, e.c.y >> 4);
    let stairs_down_id = g.tiles.get("Stairs Down").id;
    let stairs_up_id = g.tiles.get("Stairs Up").id;
    let quicksand_id = g.tiles.get("Quick Sand").id;
    // dug chasms drop you a layer; their ladders climb back up (multi-level terrain)
    let chasm_id = g.tiles.get("Chasm").id;
    let ladder_id = g.tiles.get("Ladder").id;
    if on_tile.id == stairs_down_id
        || on_tile.id == stairs_up_id
        || on_tile.id == quicksand_id
        || on_tile.id == chasm_id
        || on_tile.id == ladder_id
    {
        if e.player().on_stair_delay <= 0 {
            // when the delay time has passed...
            g.pending_level_change = if on_tile.id == stairs_up_id || on_tile.id == ladder_id {
                1
            } else {
                -1
            };
            e.player_mut().on_stair_delay = 10; // resets delay, since the level has now been changed.
            return; // SKIPS the rest of the tick() method.
        }
        // Keep re-arming the delay while standing on the tile: no second level change
        // can happen until the player has been off stairs for 10+ ticks.
        e.player_mut().on_stair_delay = 10;
    } else if e.player().on_stair_delay > 0 {
        e.player_mut().on_stair_delay -= 1;
    }

    if g.is_mode("creative") {
        // prevent stamina/hunger decay in creative mode.
        let pd = e.player_mut();
        pd.stamina = MAX_STAMINA;
        pd.hunger = MAX_HUNGER;
    }

    {
        let pd = e.player_mut();
        // staminaRechargeDelay is a penalty delay for when the player uses up all their stamina.
        if pd.stamina <= 0 && pd.stamina_recharge_delay == 0 && pd.stamina_recharge == 0 {
            pd.stamina_recharge_delay = 40; // delay before resuming adding to stamina.
        }

        if pd.stamina_recharge_delay > 0 && pd.stamina < MAX_STAMINA {
            pd.stamina_recharge_delay -= 1;
        }
    }

    if e.player().stamina_recharge_delay == 0 {
        // ticks since last recharge, accounting for the time potion effect.
        e.player_mut().stamina_recharge += 1;

        // fire wave: resting within 2 tiles of a lit campfire recharges at 2x
        if crate::entity::furniture::campfire_behavior::near_lit_campfire(g, e) {
            e.player_mut().stamina_recharge += 1;
        }

        if is_swimming(g, e) && !e.player().potioneffects.contains_key(&PotionType::Swim) {
            e.player_mut().stamina_recharge = 0; // don't recharge stamina while swimming.
        }

        // recharge a bolt for each multiple of maxStaminaRecharge.
        let pd = e.player_mut();
        while pd.stamina_recharge > MAX_STAMINA_RECHARGE {
            pd.stamina_recharge -= MAX_STAMINA_RECHARGE;
            if pd.stamina < MAX_STAMINA {
                pd.stamina += 1; // recharge one stamina bolt per "charge".
            }
        }
    }

    let diff_idx = g.settings.get_idx("diff");

    {
        let pd = e.player_mut();
        if pd.hunger < 0 {
            pd.hunger = 0; // error correction
        }

        if pd.stamina < MAX_STAMINA {
            // affect hunger if not at full stamina; this is 2 levels away from a hunger "burger".
            pd.stam_hunger_ticks -= diff_idx;
            if pd.stamina == 0 {
                pd.stam_hunger_ticks -= diff_idx; // double effect if no stamina at all.
            }
        }
    }

    // this if statement encapsulates the hunger system
    if !bed_behavior::in_bed(g, e.c.eid) {
        let tick_count = g.tick_count;
        {
            let pd = e.player_mut();
            if pd.hunger_charge_delay > 0 {
                // if the hunger is recharging health...
                pd.stam_hunger_ticks -= 2 + diff_idx; // penalize the hunger
                if pd.hunger == 0 {
                    pd.stam_hunger_ticks -= diff_idx; // starving: extra penalty
                }
            }

            if tick_count % HUNGER_TICK_COUNT[diff_idx as usize] == 0 {
                pd.stam_hunger_ticks -= 1; // hunger due to time.
            }

            if pd.step_count >= HUNGER_STEP_COUNT[diff_idx as usize] {
                pd.stam_hunger_ticks -= 1; // hunger due to exercise.
                pd.step_count = 0; // reset.
            }

            if pd.stam_hunger_ticks <= 0 {
                pd.stam_hunger_ticks += MAX_HUNGER_TICKS; // reset stamHungerTicks
                pd.hunger_stam_cnt -= 1; // enter 1 level away from burger.
            }

            while pd.hunger_stam_cnt <= 0 {
                pd.hunger -= 1; // reached burger level.
                pd.hunger_stam_cnt += MAX_HUNGER_STAMS[diff_idx as usize];
            }

            // system that heals you depending on your hunger
            if pd.mob.health < MAX_HEALTH && pd.hunger > MAX_HUNGER / 2 {
                pd.hunger_charge_delay += 1;
                if (pd.hunger_charge_delay as f64)
                    > 20.0 * ((MAX_HUNGER - pd.hunger + 2) as f64).powi(2)
                {
                    pd.mob.health += 1;
                    pd.hunger_charge_delay = 0;
                }
            } else {
                pd.hunger_charge_delay = 0;
            }

            if pd.hunger_starve_delay == 0 {
                pd.hunger_starve_delay = 120;
            }
        }

        if e.player().hunger == 0 && e.player().mob.health > MIN_STARVE_HEALTH[diff_idx as usize] {
            if e.player().hunger_starve_delay > 0 {
                e.player_mut().hunger_starve_delay -= 1;
            }
            if e.player().hunger_starve_delay == 0 {
                // self-inflicted starvation damage bypasses the hurt() entry point
                // (whose creative insta-kill check requires a distinct attacker)
                do_hurt(g, e, 1, Direction::None); // do 1 damage to the player
            }
        }
    }

    // regen health
    if e.player().potioneffects.contains_key(&PotionType::Regen) {
        let pd = e.player_mut();
        pd.regentick += 1;
        if pd.regentick > 60 {
            pd.regentick = 0;
            if pd.mob.health < 10 {
                pd.mob.health += 1;
            }
        }
    }

    // temperature wave: ambient heat/cold effects (see core::temperature)
    temperature_tick(g, e);

    if g.save_cooldown > 0 && !g.saving {
        g.save_cooldown -= 1;
    }

    if !g.menu_open() && !bed_behavior::in_bed(g, e.c.eid) {
        // this is where movement detection occurs.
        let mut xa = 0;
        let mut ya = 0;
        if g.input.get_key("up").down {
            ya -= 1;
        }
        if g.input.get_key("down").down {
            ya += 1;
        }
        if g.input.get_key("left").down {
            xa -= 1;
        }
        if g.input.get_key("right").down {
            xa += 1;
        }

        // executes if not saving; and... essentially halves speed if out of stamina.
        if (xa != 0 || ya != 0)
            && (e.player().stamina_recharge_delay % 2 == 0 || is_swimming(g, e))
            && !g.saving
        {
            let spd = e.player().move_speed
                * if e.player().potioneffects.contains_key(&PotionType::Speed) {
                    1.5
                } else {
                    1.0
                };
            let xd = (xa as f64 * spd) as i32;
            let yd = (ya as f64 * spd) as i32;
            let moved = mob_move(g, e, xd, yd, true);
            if moved {
                e.player_mut().step_count += 1;
            }
        }

        if is_swimming(g, e)
            && e.player().mob.tick_time % 60 == 0
            && !e.player().potioneffects.contains_key(&PotionType::Swim)
        {
            // if drowning... :P
            if e.player().stamina > 0 {
                e.player_mut().stamina -= 1; // take away stamina
            } else {
                do_hurt(g, e, 1, Direction::None); // out of breath: take damage
            }
        }

        let drop_one = g.input.get_key("drop-one").clicked;
        let drop_stack = g.input.get_key("drop-stack").clicked;
        if e.player().active_item.is_some() && (drop_one || drop_stack) {
            let creative = g.is_mode("creative");
            let drop = {
                let pd = e.player_mut();
                let mut drop = pd.active_item.clone().expect("checked is_some above");
                if drop_one && drop.is_stackable() && drop.count() > 1 {
                    // drop one from stack
                    if let Some(count) = pd.active_item.as_mut().and_then(|i| i.count_mut()) {
                        *count -= 1;
                    }
                    drop.set_count(1);
                } else if !creative {
                    pd.active_item = None; // remove it from the "inventory"
                }
                drop
            };
            level::drop_item(g, lvl, e.c.x, e.c.y, drop);
        }

        // this only allows attacks or pickups when such action is possible.
        let mut attack_clicked = g.input.get_key("attack").clicked;
        // Post-port: SHIFT-attack throws a held spear. The plain "attack" binding never
        // fires while SHIFT is held (modifier matching zeroes it), so check the shifted
        // chords explicitly — but only route them to attack() while a spear is in hand,
        // leaving every other SHIFT combo's behavior untouched.
        if !attack_clicked
            && matches!(
                e.player().active_item.as_ref().map(|i| &i.kind),
                Some(ItemKind::Tool {
                    ttype: ToolType::Spear,
                    ..
                })
            )
        {
            attack_clicked = g.input.get_key("shift-space|shift-c").clicked;
        }
        let pickup_clicked = g.input.get_key("pickup").clicked;
        if (attack_clicked || pickup_clicked) && e.player().stamina != 0 {
            if !e.player().potioneffects.contains_key(&PotionType::Energy) {
                e.player_mut().stamina -= 1;
            }
            e.player_mut().stamina_recharge = 0;

            if pickup_clicked {
                // if you are not already holding a power glove (aka in the middle of a
                // separate interaction)...
                let holding_glove = matches!(
                    e.player().active_item.as_ref().map(|i| &i.kind),
                    Some(ItemKind::PowerGlove)
                );
                if !holding_glove {
                    let pd = e.player_mut();
                    pd.prev_item = pd.active_item.take(); // then save the current item...
                    pd.active_item = Some(registry::new_power_glove()); // and replace it with a power glove.
                }
                attack(g, e); // attack (with the power glove)
                resolve_held_item(g, e);
            } else {
                attack(g, e);
            }
        } else if attack_clicked || pickup_clicked {
            // too winded to swing (0 stamina): the input used to die silently —
            // a soft back-beep and a small gray breath puff say "no" instead
            g.play_sound(Sound::Back);
            if let Some(lvl) = e.c.level {
                let jx = g.random.next_int_bound(5) - 2;
                // centered above the head so the player sprite (drawn later in the
                // y-sort) doesn't cover it
                let puff = new_material_puff(
                    e.c.x + jx,
                    e.c.y - 14,
                    color::get4(-1, -1, 333, 444),
                    &mut g.random,
                );
                g.level_mut(lvl).add(puff, lvl);
            }
        }

        if g.input.get_key("menu").clicked && e.player().active_item.is_some() {
            let pd = e.player_mut();
            if let Some(item) = pd.active_item.take() {
                pd.inventory.add_at(0, item);
            }
        }

        if g.input.get_key("map").clicked {
            g.set_menu(crate::screen::map_menu::MapMenu::new(g));
        }

        // !use() = no furniture in front of the player; this prevents the player inventory
        // from opening (will open furniture inventory instead)
        if g.input.get_key("inventory").clicked && !player_use(g, e) {
            g.set_menu(crate::screen::player_inv_display::PlayerInvDisplay::new(
                g, e,
            ));
        }
        if g.input.get_key("pause").clicked {
            g.set_menu(crate::screen::pause_display::PauseDisplay::new(g));
        }
        if g.input.get_key("craft").clicked && !player_use(g, e) {
            g.set_menu(
                crate::screen::crafting_display::CraftingDisplay::with_personal(
                    g,
                    g.recipes.craft.clone(),
                    "Crafting",
                    e,
                    true,
                ),
            );
        }

        if g.input.get_key("info").clicked {
            // InfoDisplay reads the player from the arena, but we're inside the player's
            // take-out tick — lend it a copy for the constructor.
            g.entities.put_back(e.clone());
            let display = crate::screen::info_display::InfoDisplay::new(g);
            g.entities.take(e.c.eid);
            g.set_menu(display);
        }

        if g.input.get_key("save").clicked && !g.saving {
            // Don't save right here: this code runs inside the player's own take-out
            // tick, and `write_player` needs the player present in the arena
            // (`g.player()`). Defer the save to `Game::tick` (same deferred shape the
            // pause-menu Save Game action gets for free by running from the display).
            g.saving = true;
            g.loading_percentage = 0.0;
            g.pending_save = true;
        }

        if g.input.get_key("night").clicked {
            g.change_time_of_day(Time::Night);
        }

        // debug feature: remove all potion effects
        if g.debug && g.input.get_key("shift-p").clicked {
            let keys: Vec<PotionType> = e.player().potioneffects.keys().copied().collect();
            for potion_type in keys {
                item_interact::apply_potion(g, e, potion_type, false);
            }
        }

        let pd = e.player_mut();
        if pd.attack_time > 0 {
            pd.attack_time -= 1;
            if pd.attack_time == 0 {
                pd.attack_item = None; // null the attackItem once we are done attacking.
            }
        }
        if pd.swing_flash > 0 {
            pd.swing_flash -= 1;
        }
    }
}

// ---- Temperature wave tuning (see core::temperature for the band model) ----
/// One heart per this many ticks while in an extreme band (~6s at normal speed):
/// full health to the 3-heart floor takes about 40 seconds of ignored warnings.
const TEMP_DAMAGE_PERIOD: i32 = 360;
/// Temperature damage stops here unless the score passes `temperature::DEADLY_SCORE`.
const TEMP_DAMAGE_FLOOR: i32 = 3;
/// Shiver/sweat puff cadence in the second-band-out states.
const TEMP_PUFF_PERIOD: i32 = 60;
/// Minimum spacing between temperature cues (band-boundary flicker guard).
const TEMP_CUE_COOLDOWN: i32 = 240;

/// Per-tick heat/cold effects. Everything is derived fresh from
/// `temperature::score_for`, so nothing here needs saving. Shape (taste rule:
/// survival pressure teaches, it doesn't punish):
/// - one band out (Chilly/Warm): nothing — the HUD dot alone carries it;
/// - two bands out (Cold/Hot): stamina recharges at ~2/3 speed, an ambient cue,
///   and a small shiver/sweat puff above the head;
/// - extreme (Freezing/Scorching): a loud warning plus slow damage that stops at
///   [`TEMP_DAMAGE_FLOOR`] hearts unless the score is truly extreme.
fn temperature_tick(g: &mut Game, e: &mut Entity) {
    if g.is_mode("creative") || bed_behavior::in_bed(g, e.c.eid) {
        return;
    }
    let score = temperature::score_for(g, e);
    apply_temperature_effects(g, e, score);
}

/// The effects mechanism, split from the world-derived score so tests can drive it
/// directly with pinned scores (and pinned `game_time`/cue fields) instead of
/// simulating thousands of real ticks.
pub fn apply_temperature_effects(g: &mut Game, e: &mut Entity, score: f64) {
    let steps = temperature::Band::from_score(score).steps();

    // Cues fire on band changes, spaced by a cooldown; the remembered band only
    // advances when the cooldown is open, so a crossing blocked mid-cooldown
    // retries (and still cues) once it expires.
    if e.player().temp_cue_cooldown > 0 {
        e.player_mut().temp_cue_cooldown -= 1;
    } else if steps != e.player().temp_prev_band {
        let prev = e.player().temp_prev_band;
        match steps {
            -3 => g.push_warning("The cold bites deep!"),
            3 => g.push_warning("The sun hammers down!"),
            -2 if prev > -2 => g.push_ambient("Your breath fogs."),
            2 if prev < 2 => g.push_ambient("The heat presses down."),
            -1..=1 if prev <= -2 => g.push_ambient("The chill eases."),
            -1..=1 if prev >= 2 => g.push_ambient("The heat eases."),
            _ => {}
        }
        e.player_mut().temp_prev_band = steps;
        e.player_mut().temp_cue_cooldown = TEMP_CUE_COOLDOWN;
    }

    // second band out: stamina recharges slower, and the body shows it
    if steps.abs() >= 2 {
        if g.game_time % 3 == 0 {
            let pd = e.player_mut();
            if pd.stamina_recharge > 0 {
                pd.stamina_recharge -= 1; // ~2/3 recharge speed
            }
        }
        if g.game_time % TEMP_PUFF_PERIOD == 0 {
            if let Some(lvl) = e.c.level {
                // shiver: an icy breath-fog puff; sweat: a watery one (tiny overlay
                // above the head — the sprite itself is never touched)
                let palette = if steps < 0 {
                    color::get4(-1, -1, 445, 555)
                } else {
                    color::get4(-1, -1, 115, 335)
                };
                let jx = g.random.next_int_bound(9) - 4;
                let puff = new_material_puff(e.c.x + jx, e.c.y - 12, palette, &mut g.random);
                g.level_mut(lvl).add(puff, lvl);
            }
        }
    }

    // extreme band: slow damage with the mercy floor
    if steps.abs() >= 3 && g.game_time % TEMP_DAMAGE_PERIOD == 0 {
        let deadly = score.abs() >= temperature::DEADLY_SCORE;
        if deadly || e.player().mob.health > TEMP_DAMAGE_FLOOR {
            do_hurt(g, e, 1, Direction::None);
        }
    }
}

/// Java `Player.resolveHeldItem()` — removes a held item and places it back into the
/// inventory. Looks complicated so it can handle the powerglove.
pub fn resolve_held_item(g: &mut Game, player: &mut Entity) {
    let creative = g.is_mode("creative");
    let pd = player.player_mut();
    let holding_glove = matches!(
        pd.active_item.as_ref().map(|i| &i.kind),
        Some(ItemKind::PowerGlove)
    );
    if !holding_glove {
        // if you are now holding something other than a power glove...
        if let Some(prev) = pd.prev_item.take() {
            // and you had a previous item that we should care about...
            if !creative {
                // add that previous item to your inventory so it isn't lost.
                pd.inventory.add_at(0, prev);
            }
        }
    } else {
        // if you're holding a power glove, then the held item didn't change, so we can
        // remove the power glove and make it what it was before.
        pd.active_item = pd.prev_item.take();
    }

    pd.prev_item = None; // this is no longer of use.

    // if, for some odd reason, you are still holding a power glove at this point, then
    // null it because it's useless and shouldn't remain in hand.
    if matches!(
        pd.active_item.as_ref().map(|i| &i.kind),
        Some(ItemKind::PowerGlove)
    ) {
        pd.active_item = None;
    }
}

/// Java `use()` — this actually ends up calling another use method down below.
fn player_use(g: &mut Game, e: &mut Entity) -> bool {
    let area = get_interaction_box(e, INTERACT_DIST);
    use_area(g, e, &area)
}

/// Java `Player.attack()` — called when we press the attack button.
pub fn attack(g: &mut Game, e: &mut Entity) {
    // walkDist is not synced, so this can happen for both the client and server.
    e.player_mut().mob.walk_dist += 8; // increase the walkDist (changes the sprite, like you moved your arm)

    let dir = e.player().mob.dir;
    let creative = g.is_mode("creative");
    let lvl = e.c.level.expect("player must be on a level");

    if e.player()
        .active_item
        .as_ref()
        .is_some_and(|i| !i.interacts_with_world())
    {
        {
            let pd = e.player_mut();
            pd.attack_dir = dir; // make the attack direction equal the current direction
            pd.attack_item = pd.active_item.clone(); // make attackItem equal activeItem
        }
        // self-targeted items (potions, food) get a dummy interaction at tile (0,0);
        // they ignore the tile entirely
        let mut item = e
            .player_mut()
            .active_item
            .take()
            .expect("checked is_some above");
        item_interact::item_interact_on_tile(g, &mut item, lvl, 0, 0, e, dir);
        e.player_mut().active_item = Some(item);
        if e.player()
            .active_item
            .as_ref()
            .is_some_and(|i| i.is_depleted())
            && !creative
        {
            e.player_mut().active_item = None;
        }
        return;
    }

    e.player_mut().attack_dir = dir; // make the attack direction equal the current direction
    let attack_dir = dir;
    {
        let pd = e.player_mut();
        pd.attack_item = pd.active_item.clone(); // make attackItem equal activeItem
    }

    // the player is holding a tool, and has stamina available.
    let tool = match e.player().active_item.as_ref().map(|i| &i.kind) {
        Some(&ItemKind::Tool { ttype, level, dur }) => Some((ttype, level, dur)),
        _ => None,
    };
    if let Some((ttype, tool_level, dur)) = tool {
        #[allow(clippy::int_plus_one)]
        if e.player().stamina - 1 >= 0 && dur > 0 {
            match ttype {
                ToolType::Bow => {
                    let arrow = registry::arrow_item(g);
                    if e.player().inventory.count(&arrow) > 0 {
                        // if the player is holding a bow, and has arrows...
                        if !creative {
                            e.player_mut().inventory.remove_item(&arrow);
                        }
                        let arrow_entity = new_arrow(e.c.eid, e.c.x, e.c.y, attack_dir, tool_level);
                        g.level_mut(lvl).add(arrow_entity, lvl);
                        e.player_mut().attack_time = 10;
                        pay_ranged_durability(e, creative);
                        return; // we have attacked!
                    }
                }
                ToolType::Crossbow => {
                    // still re-cocking: the click is a dry trigger pull.
                    if e.player().attack_time > 0 {
                        return;
                    }
                    let arrow = registry::arrow_item(g);
                    if e.player().inventory.count(&arrow) > 0 {
                        if !creative {
                            e.player_mut().inventory.remove_item(&arrow);
                        }
                        let bolt = new_arrow(e.c.eid, e.c.x, e.c.y, attack_dir, CROSSBOW_DAMAGE);
                        g.level_mut(lvl).add(bolt, lvl);
                        e.player_mut().attack_time = CROSSBOW_COOLDOWN;
                        pay_ranged_durability(e, creative);
                        return;
                    }
                }
                ToolType::Slingshot => {
                    let stone = registry::get(g, "Stone");
                    if e.player().inventory.count(&stone) > 0 {
                        if !creative {
                            e.player_mut().inventory.remove_items(&stone, 1);
                        }
                        let pellet = new_thrown(
                            e.c.eid,
                            e.c.x,
                            e.c.y,
                            attack_dir,
                            SLINGSHOT_DAMAGE,
                            ProjectileStyle::Pellet,
                            PELLET_RANGE_TICKS,
                            None,
                        );
                        g.level_mut(lvl).add(pellet, lvl);
                        e.player_mut().attack_time = 10;
                        pay_ranged_durability(e, creative);
                        return;
                    }
                }
                ToolType::Spear if g.input.get_key("shift").down => {
                    // SHIFT-attack: throw the spear itself; it lands as a pickup
                    // (durability preserved through the payload data string).
                    if let Some(item) = e.player_mut().active_item.take() {
                        let spear = new_thrown(
                            e.c.eid,
                            e.c.x,
                            e.c.y,
                            attack_dir,
                            SPEAR_THROW_BASE + tool_level * 2,
                            ProjectileStyle::Spear,
                            SPEAR_RANGE_TICKS,
                            Some(item.get_data()),
                        );
                        g.level_mut(lvl).add(spear, lvl);
                        e.player_mut().attack_time = 15;
                    }
                    return;
                }
                _ => {}
            }
        }
    }

    // Post-port: a held Throwing Knife is thrown by the attack key — it lands where it
    // stops (or in whatever it hits) as a pickup, so knives are recoverable.
    let holding_knife = e.player().active_item.as_ref().is_some_and(|i| {
        matches!(i.kind, ItemKind::Stackable { .. })
            && i.get_name().eq_ignore_ascii_case(THROWING_KNIFE)
    });
    #[allow(clippy::int_plus_one)]
    if holding_knife && e.player().stamina - 1 >= 0 {
        if !creative {
            if let Some(count) = e
                .player_mut()
                .active_item
                .as_mut()
                .and_then(|i| i.count_mut())
            {
                *count -= 1;
            }
            if e.player()
                .active_item
                .as_ref()
                .is_some_and(|i| i.is_depleted())
            {
                e.player_mut().active_item = None;
            }
        }
        let knife = new_thrown(
            e.c.eid,
            e.c.x,
            e.c.y,
            attack_dir,
            KNIFE_DAMAGE,
            ProjectileStyle::Knife,
            KNIFE_RANGE_TICKS,
            Some(format!("{THROWING_KNIFE}_1")),
        );
        g.level_mut(lvl).add(knife, lvl);
        e.player_mut().attack_time = 10;
        return;
    }

    let mut done = false; // we're not done yet (we just started!)

    // if we are simply holding an item...
    if e.player().active_item.is_some() {
        e.player_mut().attack_time = 10; // attack time will be set to 10.

        // if the interaction between you and an entity is successful, then return.
        let area = get_interaction_box(e, INTERACT_DIST);
        if interact_area(g, e, &area, attack_dir) {
            return;
        }

        // otherwise, attempt to interact with the tile.
        let t = get_interaction_tile(e);
        // Finite levels bound-check the target; infinite layers have no edges (this
        // guard silently ate every attack at negative coordinates — half the world).
        let in_bounds = g.level(lvl).is_infinite() || {
            let l = g.level(lvl);
            t.x >= 0 && t.y >= 0 && t.x < l.w && t.y < l.h
        };
        if in_bounds {
            // all entities on the target tile EXCEPT item entities
            let tile_entities: Vec<i32> = level::get_entities_in_tiles(g, lvl, t.x, t.y, t.x, t.y)
                .into_iter()
                .filter(|id| {
                    !matches!(
                        g.entities.get(*id).map(|o| &o.kind),
                        Some(EntityKind::ItemEntity(_))
                    )
                })
                .collect();
            if tile_entities.is_empty() || (tile_entities.len() == 1 && tile_entities[0] == e.c.eid)
            {
                let tile = g.tile_at(lvl, t.x, t.y);
                if let Some(mut item) = e.player_mut().active_item.take() {
                    // returns true if your held item successfully interacts with the target tile.
                    if item_interact::item_interact_on_tile(
                        g, &mut item, lvl, t.x, t.y, e, attack_dir,
                    ) {
                        done = true;
                    } else {
                        // item can't interact with tile; returns true if the target tile
                        // successfully interacts with the item.
                        if tiles::interact(g, &tile, lvl, t.x, t.y, e, &mut item, attack_dir) {
                            done = true;
                        }
                    }
                    // only restore the taken item if the interaction didn't already
                    // put a replacement in the player's hand
                    if e.player().active_item.is_none() {
                        e.player_mut().active_item = Some(item);
                    }
                }
            }

            if e.player()
                .active_item
                .as_ref()
                .is_some_and(|i| i.is_depleted())
                && !creative
            {
                // if the activeItem has 0 items left, then "destroy" it.
                e.player_mut().active_item = None;
            }
        }
    }

    if done {
        return; // skip the rest if interaction was handled.
    }

    if e.player().active_item.is_none()
        || e.player()
            .active_item
            .as_ref()
            .is_some_and(|i| i.can_attack())
    {
        // if there is no active item, OR if the item can be used to attack...
        e.player_mut().attack_time = 5;
        e.player_mut().swing_flash = SWING_FLASH_START;
        // attacks the enemy in the appropriate direction. (post-port: a spear reaches
        // further than any other melee swing)
        let attack_dist = if matches!(
            e.player().active_item.as_ref().map(|i| &i.kind),
            Some(ItemKind::Tool {
                ttype: ToolType::Spear,
                ..
            })
        ) {
            ATTACK_DIST + SPEAR_REACH_BONUS
        } else {
            ATTACK_DIST
        };
        let area = get_interaction_box(e, attack_dist);
        let mut used = hurt_area(g, e, &area, attack_dir);

        // attempts to hurt the tile in the appropriate direction.
        let t = get_interaction_tile(e);
        let in_bounds = g.level(lvl).is_infinite() || {
            let l = g.level(lvl);
            t.x >= 0 && t.y >= 0 && t.x < l.w && t.y < l.h
        };
        if in_bounds {
            let tile = g.tile_at(lvl, t.x, t.y);
            let dmg = g.random.next_int_bound(3) + 1;
            let tile_hit = tiles::hurt_by(g, &tile, lvl, t.x, t.y, e, dmg, attack_dir);
            if tile_hit {
                // impact feedback: chips of the struck material fly off the tile
                // (a whiff — nothing reacted — stays visually silent)
                spawn_impact_puffs(g, lvl, t.x, t.y, &tile.kind);
            }
            used = tile_hit || used;
        }

        if used
            && matches!(
                e.player().active_item.as_ref().map(|i| &i.kind),
                Some(ItemKind::Tool { .. })
            )
        {
            if let Some(item) = e.player_mut().active_item.as_mut() {
                item.pay_durability(creative);
            }
        }
    }
}

/// Two small puffs of the struck tile's material at the hit tile, jittered around
/// its center — the per-hit impact feedback (the smash burst still marks the break).
fn spawn_impact_puffs(g: &mut Game, lvl: usize, xt: i32, yt: i32, kind: &TileKind) {
    let palette = material_puff_palette(kind);
    for _ in 0..2 {
        let jx = g.random.next_int_bound(11) - 5;
        let jy = g.random.next_int_bound(7) - 3;
        let p = new_material_puff(xt * 16 + 8 + jx, yt * 16 + 8 + jy, palette, &mut g.random);
        g.level_mut(lvl).add(p, lvl);
    }
}

/// What color flies off a struck tile: leaf flecks off flora, gray chips off rock,
/// sand/snow/earth dust in their own hue, neutral dust for everything else.
fn material_puff_palette(kind: &TileKind) -> i32 {
    use TileKind as K;
    match kind {
        K::Tree
        | K::TreeSpecies { .. }
        | K::SnowTree
        | K::Sapling { .. }
        | K::TallGrass { .. }
        | K::Grass
        | K::Flower
        | K::Wheat
        | K::BerryBush
        | K::Seaweed
        | K::Cactus
        | K::FruitingCactus
        | K::Mushroom => color::get4(-1, -1, 131, 253),
        K::Rock
        | K::HardRock
        | K::Ore { .. }
        | K::Wall { .. }
        | K::GraveStone { .. }
        | K::Coral
        | K::CloudCactus => color::get4(-1, -1, 333, 444),
        K::Sand | K::QuickSand | K::DryBush => color::get4(-1, -1, 442, 553),
        K::Snow => color::get4(-1, -1, 445, 555),
        K::Dirt | K::Mud | K::Farm | K::Hole | K::DugPit | K::TidalFlat | K::Heath => {
            color::get4(-1, -1, 321, 432)
        }
        K::Floor { .. } | K::Door { .. } | K::Fence | K::TimberProp => {
            color::get4(-1, -1, 210, 431)
        }
        _ => color::get4(-1, -1, 322, 433),
    }
}

/// The detached sweep arc of a melee swing: the same slash cells as the base arc,
/// pushed `off` px further into the facing tile (see `SWING_FLASH_START`).
fn render_swing_sweep(screen: &mut Screen, xo: i32, yo: i32, dir: Direction, off: i32) {
    match dir {
        Direction::Up => {
            screen.render(xo, yo - 4 - off, 6 + 13 * 32, color::WHITE, 0);
            screen.render(xo + 8, yo - 4 - off, 6 + 13 * 32, color::WHITE, 1);
        }
        Direction::Down => {
            screen.render(xo, yo + 8 + 4 + off, 6 + 13 * 32, color::WHITE, 2);
            screen.render(xo + 8, yo + 8 + 4 + off, 6 + 13 * 32, color::WHITE, 3);
        }
        Direction::Left => {
            screen.render(xo - 4 - off, yo, 7 + 13 * 32, color::WHITE, 1);
            screen.render(xo - 4 - off, yo + 8, 7 + 13 * 32, color::WHITE, 3);
        }
        Direction::Right => {
            screen.render(xo + 8 + 4 + off, yo, 7 + 13 * 32, color::WHITE, 0);
            screen.render(xo + 8 + 4 + off, yo + 8, 7 + 13 * 32, color::WHITE, 2);
        }
        Direction::None => {}
    }
}

/// One durability point for firing a ranged tool (bow/crossbow/slingshot).
fn pay_ranged_durability(e: &mut Entity, creative: bool) {
    if !creative {
        if let Some(item) = e.player_mut().active_item.as_mut() {
            if let ItemKind::Tool { dur, .. } = &mut item.kind {
                *dur -= 1;
            }
        }
    }
}

/// Java `getInteractionBox(range)`.
fn get_interaction_box(e: &Entity, range: i32) -> Rectangle {
    let x = e.c.x;
    let y = e.c.y - 2;
    let dir = e.player().mob.dir;

    let para_close = 4;
    let para_far = range;
    let perp_close = 0;
    let perp_far = 8;

    let x_close = x + dir.x() * para_close + dir.y() * perp_close;
    let y_close = y + dir.y() * para_close + dir.x() * perp_close;
    let x_far = x + dir.x() * para_far + dir.y() * perp_far;
    let y_far = y + dir.y() * para_far + dir.x() * perp_far;

    Rectangle::new(
        x_close.min(x_far),
        y_close.min(y_far),
        x_close.max(x_far),
        y_close.max(y_far),
        Rectangle::CORNERS,
    )
}

/// Java `getInteractionTile()`.
fn get_interaction_tile(e: &Entity) -> Point {
    let mut x = e.c.x;
    let mut y = e.c.y - 2;
    let dir = e.player().mob.dir;

    x += dir.x() * INTERACT_DIST;
    y += dir.y() * INTERACT_DIST;

    Point::new(x >> 4, y >> 4)
}

// ---- Fishing wave tuning (invisible fish: the water tells you where they are) ----
/// Base per-cast catch chance on average open water — tuned to the classic feel
/// (the Java table landed ~16/90 ≈ 0.18 per cast).
const FISHING_BASE_CHANCE: f64 = 0.18;
/// Catch multiplier on a bubbling hotspot (`weather::fish_presence` at or above
/// `FISH_PRESENCE_THRESHOLD` — the same edge that draws the bubbles).
const FISHING_HOTSPOT_MULT: f64 = 3.0;
/// Fish bite in the rain (`weather::is_raining` at the player).
const FISHING_RAIN_MULT: f64 = 1.3;

/// Which catch table a cast draws from, decided by where the line lands.
enum CastWater {
    /// Open `water` — and a submerged Tidal Flat, which fishes like regular water.
    Regular,
    /// `Deep Water`, cast from a raft or the shore edge: bigger fish, rare treasure.
    Deep,
    /// Underground pools (any `depth < 0` layer): pale things live down there.
    Cave,
}

/// Per-cast catch chance from the local fish-presence field + rain. Pure, so tests
/// can pin the multipliers exactly:
/// - at/above [`weather::FISH_PRESENCE_THRESHOLD`] (the bubble edge) → 3x base;
/// - below it the odds scale down linearly — dead water bottoms out around 0.25x;
/// - rain adds a further 1.3x everywhere.
pub fn fishing_catch_chance(presence: f64, raining: bool) -> f64 {
    let presence_mult = if presence >= weather::FISH_PRESENCE_THRESHOLD {
        FISHING_HOTSPOT_MULT
    } else {
        // 0.25x in dead water up to ~1x just under the bubble edge
        0.25 + 1.2 * presence
    };
    let rain_mult = if raining { FISHING_RAIN_MULT } else { 1.0 };
    (FISHING_BASE_CHANCE * presence_mult * rain_mult).min(0.95)
}

/// Java `Player.goFishing(x, y)`, reworked for the fishing wave: `(x, y)` is where a
/// catch drops (pixels, by the player), `(xt, yt)` the tile the line landed on. There
/// are no fish entities — the `weather::fish_presence` field (rendered as bubbles on
/// open water) sets the odds, and the kind of water sets the catch table.
pub fn go_fishing(g: &mut Game, player: &mut Entity, x: i32, y: i32, xt: i32, yt: i32) {
    let Some(lvl) = player.c.level else { return };

    // Classify the cast: mine pools -> cave table; Deep Water -> deep table;
    // everything else (open water, submerged Tidal Flat) -> the regular table.
    let cast = if g.level(lvl).depth < 0 {
        CastWater::Cave
    } else if g.tile_at(lvl, xt, yt).id == g.tiles.get("Deep Water").id {
        CastWater::Deep
    } else {
        CastWater::Regular
    };

    let presence = weather::fish_presence(g.world_seed, xt, yt);
    let raining = weather::is_raining(g);

    // Cast feedback at the landed tile: a one-frame splash ring where the line
    // hits, and a little red bobber that sits on the water a moment.
    let (bx, by) = (xt * 16 + 8, yt * 16 + 8);
    let splash = new_smash_particle(xt * 16, yt * 16);
    g.level_mut(lvl).add(splash, lvl);
    let bobber = new_bobber_particle(bx, by, &mut g.random);
    g.level_mut(lvl).add(bobber, lvl);

    // Flavor cues (ambient tier, deduped so repeat casts don't stack the same note).
    let cue = if presence >= weather::FISH_PRESENCE_THRESHOLD {
        Some("Something stirs here...")
    } else if raining {
        Some("The rain has the fish biting...")
    } else {
        None
    };
    if let Some(msg) = cue {
        if g.notifications.last().map(String::as_str) != Some(msg) {
            g.push_ambient(msg);
        }
    }

    if g.random.next_double() >= fishing_catch_chance(presence, raining) {
        if g.random.next_int_bound(370) == 42 {
            // long-standing easter-egg console message
            println!(
                "FISHNORRIS got away... just kidding, FISHNORRIS din't get away from you, you got away from FISHNORRIS..."
            );
        }
        return;
    }

    let roll = g.random.next_int_bound(100);
    let name = match cast {
        // mostly fish, the odd slime, rarely someone's lost armor (the classic trio)
        CastWater::Regular => match roll {
            0..=64 => "Raw Fish",
            65..=93 => "Slime",
            _ => "Leather Armor",
        },
        // bigger fish out deep, and a very rare snagged treasure
        CastWater::Deep => match roll {
            0..=1 => "gem",
            2..=4 => "Iron",
            5..=21 => "Big Fish",
            _ => "Raw Fish",
        },
        // underground pools: pale eels (and the slime that was always going to be there)
        CastWater::Cave => match roll {
            0..=84 => "Cave Eel",
            _ => "Slime",
        },
    };
    let item = registry::get(g, name);
    level::drop_item(g, lvl, x, y, item);
}

/// Java `use(Rectangle area)` — called by the other use method; this serves as a buffer in
/// case there is no entity in front of the player.
fn use_area(g: &mut Game, e: &mut Entity, area: &Rectangle) -> bool {
    let Some(lvl) = e.c.level else { return false };
    let entities = level::get_entities_in_rect(g, lvl, area); // gets the entities within the 4 points
    for id in entities {
        // only some furniture classes use this.
        let is_furniture = g
            .entities
            .get(id)
            .map(|o| o.is_furniture())
            .unwrap_or(false);
        if is_furniture {
            let used = g
                .with_entity(id, |other, g| {
                    furniture_behavior::use_furniture(g, other, e)
                })
                .unwrap_or(false);
            if used {
                return true;
            }
        }
    }
    false
}

/// Java `interact(Rectangle area)` — same, but for interaction.
fn interact_area(g: &mut Game, e: &mut Entity, area: &Rectangle, attack_dir: Direction) -> bool {
    let Some(lvl) = e.c.level else { return false };
    let entities = level::get_entities_in_rect(g, lvl, area);
    let mut item = e.player_mut().active_item.take();
    let mut handled = false;
    for id in entities {
        if id == e.c.eid {
            continue;
        }
        // this is the ONLY place that the Entity.interact method is actually called.
        let hit = g
            .with_entity(id, |other, g| {
                entity_interact(g, other, e, &mut item, attack_dir)
            })
            .unwrap_or(false);
        if hit {
            handled = true;
            break;
        }
    }
    // a successful furniture take() has already put the furniture item in the player's
    // hand; only restore the taken item if nothing claimed the slot
    if e.player().active_item.is_none() {
        e.player_mut().active_item = item;
    }
    handled
}

/// Java `hurt(Rectangle area)` — same, but for attacking.
fn hurt_area(g: &mut Game, e: &mut Entity, area: &Rectangle, attack_dir: Direction) -> bool {
    let Some(lvl) = e.c.level else { return false };
    let entities = level::get_entities_in_rect(g, lvl, area);
    let mut max_dmg = 0;
    for id in entities {
        if id == e.c.eid {
            continue;
        }
        let (is_mob, is_furniture) = match g.entities.get(id) {
            Some(other) => (other.is_mob(), other.is_furniture()),
            None => continue,
        };
        if is_mob {
            let dmg = get_attack_damage(g, e);
            max_dmg = dmg.max(max_dmg);
            // note: this really only does something for mobs.
            g.with_entity(id, |other, g| mob_hurt_by_mob(g, e, other, dmg, attack_dir));
        }
        if is_furniture {
            // an itemless interact is effectively a no-op for plain furniture, but
            // Spawner/Tnt route it to their `use` handling
            let mut no_item: Option<Item> = None;
            g.with_entity(id, |other, g| {
                entity_interact(g, other, e, &mut no_item, attack_dir)
            });
        }
    }
    max_dmg > 0
}

/// How much damage the player's swing does (the target is always a mob).
fn get_attack_damage(g: &mut Game, e: &mut Entity) -> i32 {
    let mut dmg = g.random.next_int_bound(2) + 1;
    let creative = g.is_mode("creative");
    if let Some(item) = e.player_mut().active_item.as_mut() {
        if matches!(item.kind, ItemKind::Tool { .. }) {
            // sword/axe are more effective at dealing damage.
            dmg += get_attack_damage_bonus(g, item, creative);
        }
    }
    dmg
}

/// The held tool's damage bonus against mobs. Pays one durability up front — a broken
/// tool contributes nothing.
fn get_attack_damage_bonus(g: &mut Game, item: &mut Item, creative: bool) -> i32 {
    if !item.pay_durability(creative) {
        return 0;
    }

    let ItemKind::Tool { ttype, level, .. } = item.kind else {
        return 0;
    };
    // Tiers run Crude(0)..Gem(5); the bonus ranges below are for those endpoints.
    match ttype {
        // crude axe bonus: 2-5; gem axe bonus: 12-15.
        ToolType::Axe => (level + 1) * 2 + g.random.next_int_bound(4),
        // crude: 3-4 bonus; gem: 18-44 bonus.
        ToolType::Sword => (level + 1) * 3 + g.random.next_int_bound(2 + level * level),
        // crude: 3-6 bonus; gem: 18-96 bonus.
        ToolType::Claymore => (level + 1) * 3 + g.random.next_int_bound(4 + level * level * 3),
        // post-port spear bonus: crude 2-3; gem 12-18 — reach over raw power.
        ToolType::Spear => (level + 1) * 2 + g.random.next_int_bound(2 + level),
        // all other tools do very little damage to mobs.
        _ => 1,
    }
}

/// Java `Player.render(screen)`.
pub fn render(g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    let col = color::get4(-1, 100, e.player().shirt_color, 532);
    e.c.col = col;

    // the default, walking sprites.
    let carrying = matches!(
        e.player().active_item.as_ref().map(|i| &i.kind),
        Some(ItemKind::Furniture { .. })
    );
    let skinon = e.player().skinon;
    let sprite_set: &MobAnims = if carrying {
        if skinon {
            &CARRY_SUIT_SPRITES
        } else {
            &CARRY_SPRITES
        }
    } else if skinon {
        &SUIT_SPRITES
    } else {
        &SPRITES
    };

    /* offset locations to start drawing the sprite relative to our position */
    let xo = e.c.x - 8; // horizontal
    let mut yo = e.c.y - 11; // vertical

    // standing on deep water = riding the raft: draw it under the player
    if let Some(lvl) = e.c.level {
        let deep_water_id = g.tiles.get("Deep Water").id;
        if g.tile_at(lvl, e.c.x >> 4, e.c.y >> 4).id == deep_water_id {
            let raft_col = color::get4(-1, 210, 431, 321);
            for i in 0..2 {
                screen.render(xo + i * 8, e.c.y - 4, 28 + 4 * 32, raft_col, 0);
            }
        }
    }

    let swimming = is_swimming(g, e);
    if swimming {
        yo += 4; // y offset is moved up by 4
        let lvl = e.c.level.expect("swimming player must be on a level");
        let tick_time = e.player().mob.tick_time;
        let mut liquid_color = 0; // color of water / lava circle
        let standing = g.tile_at(lvl, e.c.x >> 4, e.c.y >> 4);
        if standing.id == g.tiles.get("water").id {
            liquid_color = color::get4(-1, -1, 115, 335);
            if tick_time / 8 % 2 == 0 {
                liquid_color = color::get4(-1, 335, 5, 115);
            }
        } else if standing.id == g.tiles.get("lava").id {
            liquid_color = color::get4(-1, -1, 500, 300);
            if tick_time / 8 % 2 == 0 {
                liquid_color = color::get4(-1, 300, 400, 500);
            }
        }

        screen.render(xo, yo + 3, 5 + 13 * 32, liquid_color, 0); // render the water graphic
        screen.render(xo + 8, yo + 3, 5 + 13 * 32, liquid_color, 1); // render the mirrored water graphic to the right.
    }

    let attack_time = e.player().attack_time;
    let attack_dir = e.player().attack_dir;

    if attack_time > 0 && attack_dir == Direction::Up {
        // if currently attacking upwards...
        screen.render(xo, yo - 4, 6 + 13 * 32, color::WHITE, 0); // render left half-slash
        screen.render(xo + 8, yo - 4, 6 + 13 * 32, color::WHITE, 1); // render right half-slash (mirror of left).
        if let Some(attack_item) = &e.player().attack_item {
            // if the player had an item when they last attacked...
            attack_item.sprite.render(screen, xo + 4, yo - 4); // then render the icon of the item.
        }
    }

    if e.player().mob.hurt_time > PLAYER_HURT_TIME - 10 {
        // if the player has just gotten hurt, make the sprite white.
        e.c.col = color::WHITE;
    }

    // gets the correct sprite to render (the player's dir is never NONE, so
    // get_dir() is always a valid index)
    let dir_idx = e.player().mob.dir.get_dir() as usize;
    let cur_sprite = &sprite_set[dir_idx][((e.player().mob.walk_dist >> 3) & 1) as usize];

    // render each corner of the sprite
    if !swimming {
        cur_sprite.render_color(screen, xo, yo, e.c.col);
    } else {
        // don't render the bottom half if swimming.
        cur_sprite.render_row_color(0, screen, xo, yo, e.c.col);
    }

    // renders slashes:

    if attack_time > 0 && attack_dir == Direction::Left {
        // if attacking to the left.... (same as above)
        screen.render(xo - 4, yo, 7 + 13 * 32, color::WHITE, 1);
        screen.render(xo - 4, yo + 8, 7 + 13 * 32, color::WHITE, 3);
        if let Some(attack_item) = &e.player().attack_item {
            attack_item.sprite.render(screen, xo - 4, yo + 4);
        }
    }
    if attack_time > 0 && attack_dir == Direction::Right {
        // attacking to the right
        screen.render(xo + 8 + 4, yo, 7 + 13 * 32, color::WHITE, 0);
        screen.render(xo + 8 + 4, yo + 8, 7 + 13 * 32, color::WHITE, 2);
        if let Some(attack_item) = &e.player().attack_item {
            attack_item.sprite.render(screen, xo + 8 + 4, yo + 4);
        }
    }
    if attack_time > 0 && attack_dir == Direction::Down {
        // attacking downwards
        screen.render(xo, yo + 8 + 4, 6 + 13 * 32, color::WHITE, 2);
        screen.render(xo + 8, yo + 8 + 4, 6 + 13 * 32, color::WHITE, 3);
        if let Some(attack_item) = &e.player().attack_item {
            attack_item.sprite.render(screen, xo + 4, yo + 8 + 4);
        }
    }

    // melee-swing sweep: for the first frames of a swing an extra arc detaches from
    // the base slash and travels ~6px into the facing tile — the motion smear that
    // makes the swing read (whiffs included, so a drained bolt is never a mystery)
    let swing_flash = e.player().swing_flash;
    if attack_time > 0 && swing_flash > 0 {
        let off = 2 * (SWING_FLASH_START - swing_flash);
        render_swing_sweep(screen, xo, yo, attack_dir, off);
    }

    // renders the furniture if the player is holding one.
    let ex = e.c.x;
    if let Some(item) = e.player_mut().active_item.as_mut() {
        if let ItemKind::Furniture { furniture, .. } = &mut item.kind {
            furniture.c.x = ex;
            furniture.c.y = yo - 4;
            crate::entity::behavior::entity_render(g, screen, furniture);
        }
    }
}

/// Java `Player.pickupItem(itemEntity)` — what happens when the player interacts with an
/// ItemEntity.
pub fn pickup_item(g: &mut Game, player: &mut Entity, item_entity: &mut Entity) {
    g.play_sound(Sound::Pickup);
    remove_entity(g, item_entity);
    let score_mode = g.is_mode("score");
    player.player_mut().add_score(1, score_mode);
    if g.is_mode("creative") {
        return; // we shall not bother the inventory on creative mode.
    }

    let EntityKind::ItemEntity(data) = &item_entity.kind else {
        return;
    };
    let item = data.item.clone();
    let is_fibers = item.get_name() == "Grass Fibers";
    let pd = player.player_mut();
    let stacks_with_held = item.is_stackable()
        && pd
            .active_item
            .as_ref()
            .map(|a| item.stacks_with(a))
            .unwrap_or(false);
    if stacks_with_held {
        // picked up item equals the one in your hand
        if let Some(active) = pd.active_item.as_mut() {
            let count = active.count();
            active.set_count(count + item.count());
        }
    } else {
        pd.inventory.add(item); // add item to inventory
    }

    // First-day thread, cue 2: the first fibers point at the craft key.
    if is_fibers && !pd.fiber_cue_done {
        pd.fiber_cue_done = true;
        pd.grass_cue_delay = 0; // the tall-grass hint is moot now
        let mapping = g.input.get_mapping("craft"); // e.g. "Z/SHIFT-E"
        let key = mapping.split('/').next().unwrap_or("Z").to_string();
        g.push_cue(&format!("Enough fibers could twist into cord [{key}]."));
    }
}

/// Finds a starting position for the player. With `spawn_seed` the pick is
/// deterministic (a fresh local `Rng` seeded from it); without, it draws from the
/// incidental `g.random`.
pub fn find_start_pos(g: &mut Game, player: &mut Entity, lvl: usize, spawn_seed: Option<i64>) {
    if g.level(lvl).is_infinite() {
        let (sx, sy) = crate::level::infinite_gen::find_surface_spawn(g.world_seed, &g.tiles);
        player.c.x = sx * 16 + 8;
        player.c.y = sy * 16 + 8;
        return;
    }
    let mut seeded = spawn_seed.map(Rng::new);

    let grass_id = g.tiles.get("grass").id;
    let mut spawn_tile_positions =
        level::get_matching_tiles(g, lvl, |_g, t, _x, _y| t.id == grass_id);

    if spawn_tile_positions.is_empty() {
        spawn_tile_positions.extend(level::get_matching_tiles(g, lvl, |_g, t, _x, _y| {
            tiles::may_spawn(t)
        }));
    }

    if spawn_tile_positions.is_empty() {
        let p: &Entity = player;
        spawn_tile_positions.extend(level::get_matching_tiles(g, lvl, |g2, t, x, y| {
            tiles::may_pass(g2, t, lvl, x, y, p)
        }));
    }

    let spawn_pos = if spawn_tile_positions.is_empty() {
        // there are no tiles in the entire map which the player is allowed to stand on.
        // Not likely.
        let (w, h) = {
            let l = g.level(lvl);
            (l.w, l.h)
        };
        let pos = {
            let rng = seeded.as_mut().unwrap_or(&mut g.random);
            Point::new(
                rng.next_int_bound(w / 4) + w * 3 / 8,
                rng.next_int_bound(h / 4) + h * 3 / 8,
            )
        };
        let grass = g.tiles.get("grass");
        g.set_tile_default(lvl, pos.x, pos.y, &grass);
        pos
    } else {
        // gets random valid spawn tile position.
        let idx = {
            let rng = seeded.as_mut().unwrap_or(&mut g.random);
            rng.next_int_bound(spawn_tile_positions.len() as i32)
        };
        spawn_tile_positions[idx as usize]
    };

    // used to save (tile) coordinates of spawnpoint outside of this method.
    {
        let pd = player.player_mut();
        pd.spawnx = spawn_pos.x;
        pd.spawny = spawn_pos.y;
    }
    // set (entity) coordinates of player to the center of the tile.
    player.c.x = player.player().spawnx * 16 + 8; // conversion from tile coords to entity coords.
    player.c.y = player.player().spawny * 16 + 8;
}

/// Java `Player.respawn(level)` — finds a location where the player can respawn.
pub fn respawn(g: &mut Game, player: &mut Entity, lvl: usize) {
    let (spawnx, spawny) = {
        let pd = player.player();
        (pd.spawnx, pd.spawny)
    };
    let tile = g.tile_at(lvl, spawnx, spawny);
    if !tiles::may_spawn(&tile) {
        // if there's no bed to spawn from, and the stored coordinates don't point to a
        // grass tile, then find a new point.
        find_start_pos(g, player, lvl, None);
    }

    // move the player to the spawnpoint
    player.c.x = player.player().spawnx * 16 + 8;
    player.c.y = player.player().spawny * 16 + 8;
}

/// Java `Player.payStamina(cost)` — uses an amount of stamina to do an action.
pub fn pay_stamina(player: &mut Entity, cost: i32) -> bool {
    let pd = player.player_mut();
    if pd.potioneffects.contains_key(&PotionType::Energy) {
        // the potion effect for infinite stamina; return true without subtracting cost.
        return true;
    }
    if pd.stamina <= 0 {
        return false; // the player doesn't have enough stamina; failure.
    }

    let cost = cost.max(0); // error correction
    pd.stamina -= pd.stamina.min(cost); // subtract the cost from the current stamina
    true // success
}

/// Java `Player.getLightRadius()` — the player's light radius underground.
pub fn get_light_radius(e: &Entity) -> i32 {
    let mut r = 5; // the radius of the light.

    if let Some(item) = &e.player().active_item {
        if let ItemKind::Furniture { furniture, .. } = &item.kind {
            // if player is holding furniture: brings player light up to furniture light,
            // if less, since the furniture is not yet part of the level and so doesn't
            // emit light even if it should.
            let rr = crate::entity::behavior::get_light_radius(furniture);
            if rr > r {
                r = rr;
            }
        }
    }

    r // return light radius
}

/// Java `Player.die()` — what happens when the player dies.
pub fn die(g: &mut Game, e: &mut Entity) {
    {
        let pd = e.player_mut();
        let score = pd.get_score();
        pd.set_score(score - score / 3); // subtracts score penalty (minus 1/3 of the original score)
        pd.reset_multiplier();
    }

    // make a death chest holding everything the player carried
    let mut dc = death_chest::new(g);
    dc.c.x = e.c.x;
    dc.c.y = e.c.y;
    dc.chest_mut()
        .expect("death chest")
        .inventory
        .add_all(&e.player().inventory);

    if let Some(active) = e.player().active_item.clone() {
        dc.chest_mut().expect("death chest").inventory.add(active);
    }
    if let Some(armor) = e.player().cur_armor.clone() {
        dc.chest_mut().expect("death chest").inventory.add(armor);
    }

    g.play_sound(Sound::PlayerDeath);

    let cur = g.current_level;
    g.level_mut(cur).add(dc, cur);

    remove_entity(g, e);
}

/// Java `Player.hurt(Tnt tnt, int dmg)` — TNT damage also drains stamina.
pub fn hurt_by_tnt(g: &mut Game, player: &mut Entity, tnt: &Entity, dmg: i32) {
    let attack_dir = get_attack_dir(tnt, player);
    do_hurt(g, player, dmg, attack_dir);
    pay_stamina(player, dmg * 2);
}

/// Java `Player.hurt(damage, attackDir)` via a mob attacker (`Mob.hurt(mob, dmg, dir)`
/// lands here through the dispatch in behavior.rs).
pub fn hurt_by_mob(
    g: &mut Game,
    player: &mut Entity,
    attacker: &mut Entity,
    damage: i32,
    attack_dir: Direction,
) {
    let _ = attacker;
    do_hurt(g, player, damage, attack_dir);
}

/// Java `Player.doHurt(damage, attackDir)` — what happens when the player is hurt.
pub fn do_hurt(g: &mut Game, player: &mut Entity, damage: i32, attack_dir: Direction) {
    // can't get hurt in creative, hurt cooldown, or while someone is in bed
    if g.is_mode("creative")
        || player.player().mob.hurt_time > 0
        || bed_behavior::in_bed(g, player.c.eid)
    {
        return;
    }

    let mut health_dam = 0;
    let mut armor_dam = 0;
    g.play_sound(Sound::PlayerHurt);
    {
        let pd = player.player_mut();
        match &pd.cur_armor {
            None => {
                // no armor
                health_dam = damage; // subtract that amount
            }
            Some(cur_armor) => {
                // has armor
                let armor_level = match cur_armor.kind {
                    ItemKind::Armor { level, .. } => level,
                    _ => 0,
                };
                pd.armor_damage_buffer += damage;
                armor_dam += damage;

                // every (level + 1) buffered points of damage, one leaks through
                #[allow(clippy::int_plus_one)]
                while pd.armor_damage_buffer >= armor_level + 1 {
                    pd.armor_damage_buffer -= armor_level + 1;
                    health_dam += 1;
                }
            }
        }
    }

    // adds a text particle telling how much damage was done to the player, and the armor.
    if armor_dam > 0 {
        if let Some(lvl) = player.c.level {
            let p = new_text_particle(
                &damage.to_string(),
                player.c.x,
                player.c.y,
                color::GRAY,
                &mut g.random,
            );
            g.level_mut(lvl).add(p, lvl);
        }
        let pd = player.player_mut();
        pd.armor -= armor_dam;
        if pd.armor <= 0 {
            health_dam -= pd.armor; // adds armor damage overflow to health damage (minus b/c armor would be negative)
            pd.armor = 0;
            pd.armor_damage_buffer = 0; // ensures that new armor doesn't inherit partial breaking from this armor.
            pd.cur_armor = None; // removes armor
        }
    }

    if health_dam > 0 {
        if let Some(lvl) = player.c.level {
            let p = new_text_particle(
                &damage.to_string(),
                player.c.x,
                player.c.y,
                color::get(-1, 504),
                &mut g.random,
            );
            g.level_mut(lvl).add(p, lvl);
        }
        mob_do_hurt_base(g, player, health_dam, attack_dir); // sets knockback, and takes away health.
    }

    player.player_mut().mob.hurt_time = PLAYER_HURT_TIME;
}

/// Fill the creative inventory. The player constructor (player.rs) can't reach the item
/// registry, so world-init code calls this right after creating the player.
pub fn maybe_fill_creative_inv(g: &mut Game) {
    if !g.is_mode("creative") {
        return;
    }
    let mut inv = std::mem::take(&mut g.player_mut().player_mut().inventory);
    inv.creative = true;
    registry::fill_creative_inv(g, &mut inv, true);
    g.player_mut().player_mut().inventory = inv;
}
