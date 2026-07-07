//! Behavior of `fdoom.entity.mob.Player` — tick/attack/render/hurt/death.
//!
//! The data + constructor live in `player.rs`. Every `isValidClient()`/`isValidServer()`/
//! `ISONLINE` branch from Java is dead in this build (see PORTING.md "Multiplayer"); the
//! singleplayer paths are ported and the dead branches noted with `// JAVA:` comments.

use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::core::updater::Time;
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
use crate::entity::particle::new_text_particle;
use crate::entity::projectile::new_arrow;
use crate::entity::{Direction, Entity, EntityKind};
use crate::gfx::{MobAnims, Point, Rectangle, Screen, color};
use crate::item::{Item, ItemKind, PotionType, ToolType, interact as item_interact, registry};
use crate::level;
use crate::level::tile::dispatch as tiles;
use crate::rng::Rng;

/// Java `Player.tick()`.
pub fn tick(g: &mut Game, e: &mut Entity) {
    if e.c.level.is_none() || e.c.removed {
        return;
    }

    // Refresh the inventory's creative-mode flag (Java's anonymous Inventory subclass in
    // the Player constructor read Game.isMode("creative") live; see inventory.rs).
    e.player_mut().inventory.creative = g.is_mode("creative");

    // JAVA: `if(Game.getMenu() != null && !Game.ISONLINE) return;` — don't tick the player
    // when a menu is open (Updater still ticks the player entity then).
    if g.menu_open() {
        return;
    }

    // JAVA: super.tick() — ticks Mob.java. Returns false if the player was removed (died).
    if !mob_tick_base(g, e) {
        return;
    }

    // JAVA: `if(!Game.isValidClient())` — always true offline.
    {
        let paused = g.paused;
        e.player_mut().tick_multiplier(paused);
    }

    if !e.player().potioneffects.is_empty() && !bed_behavior::in_bed(g, e.c.eid) {
        // JAVA: iterates over a snapshot array of the key set.
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
            // JAVA: World.scheduleLevelChange (its isValidServer guard is always false).
            g.pending_level_change = if on_tile.id == stairs_up_id || on_tile.id == ladder_id {
                1
            } else {
                -1
            };
            e.player_mut().on_stair_delay = 10; // resets delay, since the level has now been changed.
            return; // SKIPS the rest of the tick() method.
        }
        // JAVA: resets the delay if on a stairs tile but the delay is greater than 0;
        // prevents a level change until you get off the tile for 10+ ticks.
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
                    // JAVA: "further penalty if at full hunger" (the check is hunger == 0)
                    pd.stam_hunger_ticks -= diff_idx;
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
                // JAVA: hurt(this, 1, Direction.NONE) — Mob.hurt with a self source goes
                // straight to doHurt (the creative insta-kill check requires mob != this).
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
            // JAVA: the newDir + Game.client.move packet is connected-client-only; dead offline.
            // THIS is where the player moves; part of Mob.java
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
                // JAVA: hurt(this, 1, Direction.NONE) — if no stamina, take damage.
                do_hurt(g, e, 1, Direction::None);
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
            // JAVA: the isValidClient dropItem packet is dead — level.dropItem(x, y, drop).
            level::drop_item(g, lvl, e.c.x, e.c.y, drop);
        }

        // this only allows attacks or pickups when such action is possible.
        // JAVA: `(activeItem == null || !activeItem.used_pending)` — used_pending is a
        // network-sync flag, always false offline.
        let attack_clicked = g.input.get_key("attack").clicked;
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
                // JAVA: `if(!Game.ISONLINE)` — always true offline.
                resolve_held_item(g, e);
            } else {
                attack(g, e);
            }
            // JAVA: the ISONLINE used_pending marking is skipped — offline.
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
            // JAVA: new CraftingDisplay(Recipes.craftRecipes, "Crafting", this, true)
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

        // JAVA: `!(this instanceof RemotePlayer) && !Game.isValidClient()` — both always true.
        if g.input.get_key("save").clicked && !g.saving {
            // FIX: saving right here panicked — this code runs inside the player's own
            // take-out tick, and `write_player` needs the player present in the arena
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
                // JAVA: isConnectedClient sendPotionEffect skipped — offline.
            }
        }

        let pd = e.player_mut();
        if pd.attack_time > 0 {
            pd.attack_time -= 1;
            if pd.attack_time == 0 {
                pd.attack_item = None; // null the attackItem once we are done attacking.
            }
        }
    }
    // JAVA: isConnectedClient sendPlayerUpdate skipped — offline.
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
        // JAVA: activeItem.interactOn(Tiles.get("rock"), level, 0, 0, this, attackDir) —
        // a dummy rock tile at 0,0; reflexive items ignore the tile entirely.
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

    // JAVA: the isConnectedClient branch (the server executes the full method) is dead offline.

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
        // JAVA: `stamina - 1 >= 0` — preserved verbatim.
        #[allow(clippy::int_plus_one)]
        if e.player().stamina - 1 >= 0 && ttype == ToolType::Bow && dur > 0 {
            let arrow = registry::arrow_item(g);
            if e.player().inventory.count(&arrow) > 0 {
                // if the player is holding a bow, and has arrows...
                if !creative {
                    e.player_mut().inventory.remove_item(&arrow);
                }
                let arrow_entity = new_arrow(e.c.eid, e.c.x, e.c.y, attack_dir, tool_level);
                g.level_mut(lvl).add(arrow_entity, lvl);
                e.player_mut().attack_time = 10;
                if !creative {
                    if let Some(item) = e.player_mut().active_item.as_mut() {
                        if let ItemKind::Tool { dur, .. } = &mut item.kind {
                            *dur -= 1;
                        }
                    }
                }
                return; // we have attacked!
            }
        }
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
        let (w, h) = {
            let l = g.level(lvl);
            (l.w, l.h)
        };
        if t.x >= 0 && t.y >= 0 && t.x < w && t.y < h {
            // if the target coordinates are a valid tile...
            // JAVA: getEntitiesInTiles(t.x, t.y, t.x, t.y, false, ItemEntity.class) — all
            // entities on the target tile EXCEPT item entities.
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
                    // JAVA: activeItem aliasing — only restore ours if an interaction
                    // didn't replace the held item.
                    if e.player().active_item.is_none() {
                        e.player_mut().active_item = Some(item);
                    }
                }
            }

            // JAVA: the isValidServer RemotePlayer sendTileUpdate is skipped — offline.

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
        // attacks the enemy in the appropriate direction.
        let area = get_interaction_box(e, ATTACK_DIST);
        let mut used = hurt_area(g, e, &area, attack_dir);

        // attempts to hurt the tile in the appropriate direction.
        let t = get_interaction_tile(e);
        let (w, h) = {
            let l = g.level(lvl);
            (l.w, l.h)
        };
        if t.x >= 0 && t.y >= 0 && t.x < w && t.y < h {
            let tile = g.tile_at(lvl, t.x, t.y);
            let dmg = g.random.next_int_bound(3) + 1;
            used = tiles::hurt_by(g, &tile, lvl, t.x, t.y, e, dmg, attack_dir) || used;
        }

        if used
            && matches!(
                e.player().active_item.as_ref().map(|i| &i.kind),
                Some(ItemKind::Tool { .. })
            )
        {
            // ((ToolItem)activeItem).payDurability()
            if let Some(item) = e.player_mut().active_item.as_mut() {
                item.pay_durability(creative);
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

/// Java `Player.goFishing(x, y)`.
pub fn go_fishing(g: &mut Game, player: &mut Entity, x: i32, y: i32) {
    let Some(lvl) = player.c.level else { return };
    let fcatch = g.random.next_int_bound(90);

    if fcatch < 10 {
        let item = registry::get(g, "raw fish");
        level::drop_item(g, lvl, x, y, item);
    } else if fcatch < 15 {
        let item = registry::get(g, "slime");
        level::drop_item(g, lvl, x, y, item);
    } else if fcatch == 15 {
        let item = registry::get(g, "Leather Armor");
        level::drop_item(g, lvl, x, y, item);
    } else if fcatch == 42 && g.random.next_int_bound(5) == 0 {
        // JAVA: easter-egg console message, preserved verbatim.
        println!(
            "FISHNORRIS got away... just kidding, FISHNORRIS din't get away from you, you got away from FISHNORRIS..."
        );
    }
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
    // JAVA: activeItem aliasing — a successful furniture take() has already replaced the
    // player's held item; only restore ours if nothing claimed the slot.
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
            // JAVA: e.interact(this, null, attackDir) — with a null item this is a no-op
            // (Entity.interact only forwards to item.interact).
            let mut no_item: Option<Item> = None;
            g.with_entity(id, |other, g| {
                entity_interact(g, other, e, &mut no_item, attack_dir)
            });
        }
    }
    max_dmg > 0
}

/// Java `getAttackDamage(e)` — calculates how much damage the player will do.
///
/// JAVA: the Entity parameter was only used for the `instanceof Mob` check inside
/// `ToolItem.getAttackDamageBonus`, and this is only ever called with a Mob target.
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

/// Java `ToolItem.getAttackDamageBonus(e)` — implemented Player-side since it needs
/// `g.random`; pays durability first, exactly like Java. The target is always a Mob.
fn get_attack_damage_bonus(g: &mut Game, item: &mut Item, creative: bool) -> i32 {
    if !item.pay_durability(creative) {
        return 0;
    }

    let ItemKind::Tool { ttype, level, .. } = item.kind else {
        return 0;
    };
    // Tiers run Crude(0)..Gem(5) since the post-port Crude tier shifted the Java
    // Wood..Gem levels up by one; bonus ranges below are for those endpoints.
    match ttype {
        // crude axe bonus: 2-5; gem axe bonus: 12-15.
        ToolType::Axe => (level + 1) * 2 + g.random.next_int_bound(4),
        // crude: 3-4 bonus; gem: 18-44 bonus.
        ToolType::Sword => (level + 1) * 3 + g.random.next_int_bound(2 + level * level),
        // crude: 3-6 bonus; gem: 18-96 bonus.
        ToolType::Claymore => (level + 1) * 3 + g.random.next_int_bound(4 + level * level * 3),
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
        let standing = g.tile_at(lvl, e.c.x / 16, e.c.y / 16);
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

    // gets the correct sprite to render.
    // JAVA: spriteSet[dir.getDir()] — the player's dir is never NONE, so getDir() >= 0.
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

    // renders the furniture if the player is holding one.
    let ex = e.c.x;
    if let Some(item) = e.player_mut().active_item.as_mut() {
        if let ItemKind::Furniture { furniture, .. } = &mut item.kind {
            furniture.c.x = ex;
            furniture.c.y = yo - 4;
            // JAVA: furniture.render(screen) — virtual dispatch on the furniture kind.
            crate::entity::behavior::entity_render(g, screen, furniture);
        }
    }
}

/// Java `Player.pickupItem(itemEntity)` — what happens when the player interacts with an
/// ItemEntity.
pub fn pickup_item(g: &mut Game, player: &mut Entity, item_entity: &mut Entity) {
    g.play_sound(Sound::Pickup);
    remove_entity(g, item_entity);
    // JAVA: addScore(1) — its isValidClient guard is dead offline.
    let score_mode = g.is_mode("score");
    player.player_mut().add_score(1, score_mode);
    if g.is_mode("creative") {
        return; // we shall not bother the inventory on creative mode.
    }

    let EntityKind::ItemEntity(data) = &item_entity.kind else {
        return;
    };
    let item = data.item.clone();
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
}

/// Java `findStartPos(level)` / `findStartPos(level, spawnSeed)` — finds a starting
/// position for the player.
///
/// JAVA: the seeded variant did `random.setSeed(spawnSeed)` on the entity's own Random;
/// here a local `Rng` stands in for the freshly-reseeded instance.
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
/// (JAVA: always returned true.)
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
    // JAVA: the isValidServer sendStaminaChange is skipped — offline.
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

    // make death chest. JAVA: `new DeathChest(this)` — the constructor copies the
    // player's position and inventory.
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

    // JAVA: `if(!Game.ISONLINE)` — always true; World.levels[Game.currentLevel].add(dc).
    let cur = g.current_level;
    g.level_mut(cur).add(dc, cur);

    // JAVA: super.die() — Mob.die → Entity.die → remove().
    remove_entity(g, e);
}

/// Java `Player.hurt(Tnt tnt, int dmg)` — TNT damage also drains stamina.
pub fn hurt_by_tnt(g: &mut Game, player: &mut Entity, tnt: &Entity, dmg: i32) {
    // JAVA: super.hurt(tnt, dmg) — Mob.hurt(Tnt) → doHurt(dmg, getAttackDir(tnt, this)).
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

    // JAVA: the isValidServer RemotePlayer broadcast is dead, and
    // `fullPlayer = !(isValidClient && this != Game.player)` is always true offline
    // (the !fullPlayer branches played Sound.monsterHurt instead; skipped).

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

                // JAVA: `>= curArmor.level+1` — preserved verbatim.
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
        // JAVA: `if(healthDam > 0 || !fullPlayer)` — fullPlayer is always true offline.
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

/// JAVA: the Player constructor calls `Items.fillCreativeInv(inventory)` in creative mode;
/// the constructor lives in player.rs (which can't reach the registry mutably), so the
/// world-init code calls this right after creating the player instead.
pub fn maybe_fill_creative_inv(g: &mut Game) {
    if !g.is_mode("creative") {
        return;
    }
    let mut inv = std::mem::take(&mut g.player_mut().player_mut().inventory);
    inv.creative = true;
    registry::fill_creative_inv(g, &mut inv, true);
    g.player_mut().player_mut().inventory = inv;
}
