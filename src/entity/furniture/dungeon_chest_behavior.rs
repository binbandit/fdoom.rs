//! Behavior of `fdoom.entity.furniture.DungeonChest`.

use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::{Entity, EntityKind, particle};
use crate::gfx::{Screen, color};
use crate::item::registry;

use super::dungeon_chest::{lock_col, open_col};

/// Java `DungeonChest.use(player)` — key/unlock logic.
pub fn use_furniture(g: &mut Game, e: &mut Entity, player: &mut Entity) -> bool {
    let is_locked = match &e.kind {
        EntityKind::DungeonChest(d) => d.is_locked,
        _ => return false,
    };

    if is_locked {
        let key = registry::get(g, "Key");
        let (active_key, inv_key) = {
            let pd = player.player();
            let active_key = pd
                .active_item
                .as_ref()
                .map(|a| a.item_equals(&key))
                .unwrap_or(false);
            let inv_key = pd.inventory.count(&key) > 0;
            (active_key, inv_key)
        };
        if active_key || inv_key {
            // if the player has a key...
            if !g.is_mode("creative") {
                // remove the key unless on creative mode.
                let pd = player.player_mut();
                if active_key {
                    // remove activeItem
                    if let Some(active) = pd.active_item.as_mut() {
                        if let Some(count) = active.count_mut() {
                            *count -= 1;
                        }
                    }
                } else {
                    // remove from inv
                    pd.inventory.remove_item(&key);
                }
            }

            if let EntityKind::DungeonChest(d) = &mut e.kind {
                d.is_locked = false;
            }
            e.c.col = open_col(); // set to the unlocked color

            if let Some(lvl) = e.c.level {
                // The *16 is a long-standing quirk: x/y are already pixel coordinates,
                // so the smash particle lands far off-screen. Kept as-is.
                g.play_sound(Sound::MonsterHurt);
                let smash = particle::new_smash_particle(e.c.x * 16, e.c.y * 16);
                g.level_mut(lvl).add(smash, lvl);
                let text =
                    particle::new_text_particle("-1 key", e.c.x, e.c.y, color::RED, &mut g.random);
                g.level_mut(lvl).add(text, lvl);

                g.level_mut(lvl).chest_count -= 1;
                if g.level(lvl).chest_count == 0 {
                    // the last chest on the level: bonus loot
                    let gold_apple = registry::get(g, "Gold Apple");
                    for _ in 0..5 {
                        crate::level::drop_item(g, lvl, e.c.x, e.c.y, gold_apple.clone());
                    }
                    g.notify_all_tick("The dungeon lies plundered!", -100);
                }
            }

            return super::chest_behavior::use_furniture(g, e, player); // the player unlocked the chest.
        }

        false // the chest is locked, and the player has no key.
    } else {
        super::chest_behavior::use_furniture(g, e, player) // the chest was already unlocked.
    }
}

/// Java `DungeonChest.render(screen)` — refreshes the color from the lock state first.
pub fn render(g: &mut Game, screen: &mut Screen, e: &mut Entity) {
    let col = match &e.kind {
        EntityKind::DungeonChest(d) => {
            if d.is_locked {
                lock_col()
            } else {
                open_col()
            }
        }
        _ => return,
    };
    if let EntityKind::DungeonChest(d) = &mut e.kind {
        d.chest.furniture.sprite.color = col;
    }
    e.c.col = col;
    super::behavior::render(g, screen, e);
}

/// Java `DungeonChest.touchedBy(entity)` — can only be pushed if unlocked.
pub fn touched_by(g: &mut Game, e: &mut Entity, by: &mut Entity) {
    let is_locked = matches!(&e.kind, EntityKind::DungeonChest(d) if d.is_locked);
    if !is_locked {
        if by.is_player() {
            super::behavior::try_push(g, e, by);
        }
    }
}

/// Java `DungeonChest.take(player)` — can only be taken if unlocked.
pub fn take(g: &mut Game, e: &mut Entity, player: &mut Entity) {
    let is_locked = matches!(&e.kind, EntityKind::DungeonChest(d) if d.is_locked);
    if !is_locked {
        super::behavior::take(g, e, player);
    }
}
