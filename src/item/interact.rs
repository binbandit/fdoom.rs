//! Item use behaviors: Java `Item.interact` / `interactOn` and the subclass overrides
//! (Tool/Tile/Torch/Bucket/Food/Armor/Clothing/Potion/Book/Furniture/PowerGlove items).

use crate::core::game::Game;
use crate::entity::furniture;
use crate::entity::mob::player::MAX_HUNGER;
use crate::entity::mob::player_behavior::pay_stamina;
use crate::entity::{Direction, Entity, EntityKind};
use crate::item::{Fill, Item, ItemKind, PotionType, ToolType, registry};
use crate::level::tile::dispatch as tiles;

/// Java `item.interact(player, entity, attackDir)` (PowerGlove picks up furniture).
pub fn item_interact_entity(
    g: &mut Game,
    item: &mut Item,
    player: &mut Entity,
    entity: &mut Entity,
    attack_dir: Direction,
) -> bool {
    let _ = attack_dir;
    if matches!(item.kind, ItemKind::PowerGlove) {
        // the power glove picks up furniture; nothing else responds to it
        if entity.is_furniture() {
            furniture_take(g, entity, player);
            return true;
        }
        return false; // we were not given a furniture entity.
    }
    false // no other item interacts with entities directly
}

/// The virtual `Furniture.take(player)` dispatch (DeathChest/DungeonChest override it).
fn furniture_take(g: &mut Game, entity: &mut Entity, player: &mut Entity) {
    match &entity.kind {
        EntityKind::DeathChest(_) => {
            furniture::death_chest_behavior::take(g, entity, player) // can't grab a death chest
        }
        EntityKind::DungeonChest(_) => furniture::dungeon_chest_behavior::take(g, entity, player),
        _ => furniture::behavior::take(g, entity, player),
    }
}

/// Ambient-ticker cue for a player-initiated placement that failed. The audit rule
/// (UI_REDESIGN §1.3): a place attempt never dies silently — every gate answers
/// with its specific reason. Deduped against the newest ticker line so mashing the
/// key doesn't stack the note (the fishing-line pattern).
pub(crate) fn place_note(g: &mut Game, msg: &str) {
    if g.notifications.last().map(String::as_str) != Some(msg) {
        g.push_ambient(msg);
    }
}

/// Java `StackableItem.interactOn(subClassSuccess)` — the standardized count decrement.
fn stackable_interact_on(g: &Game, item: &mut Item, sub_class_success: bool) -> bool {
    if sub_class_success && !g.is_mode("creative") {
        if let Some(count) = item.count_mut() {
            *count -= 1;
        }
    }
    sub_class_success
}

/// Java `item.interactOn(tile, level, xt, yt, player, attackDir)`.
#[allow(clippy::too_many_arguments)]
pub fn item_interact_on_tile(
    g: &mut Game,
    item: &mut Item,
    lvl: usize,
    xt: i32,
    yt: i32,
    player: &mut Entity,
    attack_dir: Direction,
) -> bool {
    let _ = attack_dir;
    match &item.kind {
        // ToolItem.interactOn: fishing rod on fishable water — open `water`,
        // `Deep Water` (cast from a raft or the shore edge), or a Tidal Flat while
        // the tide has it submerged. The kind of water picks the catch table
        // (`go_fishing`).
        ItemKind::Tool { ttype, .. } => {
            let ttype = *ttype;
            let tile = g.tile_at(lvl, xt, yt);
            let fishable = tile.id == g.tiles.get("water").id
                || tile.id == g.tiles.get("Deep Water").id
                || (matches!(tile.kind, crate::level::tile::TileKind::TidalFlat)
                    && crate::level::tile::tidal::is_submerged(g, xt, yt));
            if ttype == ToolType::FishingRod && fishable {
                let creative = g.is_mode("creative");
                if item.pay_durability(creative) {
                    // the catch drops slightly off-center by the player
                    let (x, y) = (player.c.x - 5, player.c.y - 5);
                    crate::entity::mob::player_behavior::go_fishing(g, player, x, y, xt, yt);
                    return true;
                }
            } else if ttype == ToolType::FishingRod {
                // A cast that lands on dry ground still swings like any tool (the
                // fall-through attack pays durability) — say where the line went,
                // deduped so repeat casts don't stack the note.
                let msg = "The line lands in the dirt.";
                if g.notifications.last().map(String::as_str) != Some(msg) {
                    g.push_ambient(msg);
                }
            }

            false
        }

        // TileItem.interactOn: place the model tile on a matching valid tile.
        ItemKind::TileItem {
            model, valid_tiles, ..
        } => {
            let model = model.clone();
            let valid_tiles = valid_tiles.clone();
            let tile = g.tile_at(lvl, xt, yt);
            let data = g.level(lvl).get_data(xt, yt);
            for tilename in &valid_tiles {
                if tiles::matches(&tile, data, tilename) {
                    g.set_tile_named(lvl, xt, yt, &model); // TODO maybe data should be part of the saved tile..?
                    return stackable_interact_on(g, item, true);
                }
            }

            if g.debug {
                println!("{} cannot be placed on {}", model, tile.name);
            }

            // wrong ground always says why (walls want their floor, floors want a
            // hole, everything else names the ground it grows or sits on)
            let note = if model.contains("WALL") || model.contains("DOOR") {
                format!(
                    "Can only be placed on {}!",
                    g.tiles.get_name(&valid_tiles[0])
                )
            } else if model.contains("BRICK") || model.contains("PLANK") {
                "Dig a hole first!".to_string()
            } else {
                format!("Needs {} to go on.", g.tiles.get_name(&valid_tiles[0]))
            };
            place_note(g, &note);

            stackable_interact_on(g, item, false)
        }

        // TorchItem.interactOn: place the torch version of the tile.
        ItemKind::Torch { valid_tiles, .. } => {
            let valid_tiles = valid_tiles.clone();
            let tile = g.tile_at(lvl, xt, yt);
            // a held torch SMOKES a beehive rather than looking for footing — hand
            // the whole interaction to the tile (bees & honey wave)
            if matches!(tile.kind, crate::level::tile::TileKind::Beehive) {
                return false;
            }
            if valid_tiles.contains(&tile.name) {
                let torch = g.tiles.get_torch_tile(tile);
                g.set_tile_default(lvl, xt, yt, &torch);
                return stackable_interact_on(g, item, true);
            }
            place_note(g, "No footing for a torch here.");
            stackable_interact_on(g, item, false)
        }

        // BucketItem.interactOn: fill from / empty onto the tile.
        ItemKind::Bucket { filling, .. } => {
            let filling = *filling;
            let tile = g.tile_at(lvl, xt, yt);
            let fill = Fill::VALUES
                .iter()
                .copied()
                .find(|f| g.tiles.get(f.contained_tile()).id == tile.id);
            let Some(fill) = fill else { return false };
            if fill == Fill::Empty && filling != Fill::Empty {
                let t = g.tiles.get(filling.contained_tile());
                g.set_tile_default(lvl, xt, yt, &t);
                if !g.is_mode("creative") {
                    edit_bucket(item, player, Fill::Empty);
                }
                true
            } else if filling == Fill::Empty {
                let t = g.tiles.get("hole");
                g.set_tile_default(lvl, xt, yt, &t);
                if !g.is_mode("creative") {
                    edit_bucket(item, player, fill);
                }
                true
            } else {
                false
            }
        }

        // FoodItem.interactOn: eat.
        ItemKind::Food {
            count,
            heal,
            stamina_cost,
        } => {
            let (count, heal, stamina_cost) = (*count, *heal, *stamina_cost);
            let mut success = false;
            // if the player has hunger to fill, and stamina to pay...
            if count > 0 && player.player().hunger < MAX_HUNGER && pay_stamina(player, stamina_cost)
            {
                let pd = player.player_mut();
                pd.hunger = (pd.hunger + heal).min(MAX_HUNGER); // restore the hunger
                success = true;

                // cooking wave (item/cooking.rs): raw flesh gambles on a Queasy
                // spell; a composed hot dish refills stamina and grants a short
                // Regen — the pot cookery payoff.
                let name = item.get_name().to_string();
                if crate::item::cooking::queasy_risk(&name) && g.random.next_int_bound(3) == 0 {
                    apply_potion_time(g, player, PotionType::Queasy, PotionType::Queasy.duration());
                    g.notifications.push("Your stomach turns...".to_string());
                } else if crate::item::cooking::is_hearty(&name) {
                    let pd = player.player_mut();
                    pd.stamina = crate::entity::mob::player::MAX_STAMINA;
                    apply_potion_time(g, player, PotionType::Regen, 600);
                    g.notifications
                        .push("The hot meal warms you through".to_string());
                }
            }

            // bees & honey: the jar's sugar rush — the heal above plus a brief
            // Energy spell (faster stamina regen), the "brief stamina regen" half
            // of the honey payoff
            if success && item.get_name().eq_ignore_ascii_case("Honey Jar") {
                apply_potion_time(g, player, PotionType::Energy, 300);
                g.notifications
                    .push("Sweet warmth spreads through you.".to_string());
            }

            // scavenge wave: aged tin rations always leave the can behind, and
            // roughly one can in four has turned — a mild stamina knock and a
            // notification, never damage (pressure teaches, never punishes)
            if success && item.get_name().eq_ignore_ascii_case("Old Food Can") {
                player
                    .player_mut()
                    .inventory
                    .add(registry::get(g, "Empty Can"));
                if g.random.next_int_bound(4) == 0 {
                    let pd = player.player_mut();
                    pd.stamina = (pd.stamina - 4).max(0);
                    g.notifications.push("Your stomach churns...".to_string());
                }
            }

            stackable_interact_on(g, item, success)
        }

        // Post-port Medical.interactOn: patch up (restores health, not hunger).
        ItemKind::Medical { count, heal } => {
            let (count, heal) = (*count, *heal);
            let mut success = false;
            // if the player is hurt, and has the stamina to apply it...
            if count > 0
                && player.player().mob.health < crate::entity::mob::player::MAX_HEALTH
                && pay_stamina(player, 5)
            {
                crate::entity::behavior::heal(g, player, heal);
                success = true;
            }

            stackable_interact_on(g, item, success)
        }

        // ArmorItem.interactOn — the legacy use-to-wear path (attack with the armor
        // held). It keeps working for muscle memory, but now routes through the same
        // slot model as the WEAR pane (`PlayerData::equip`), swaps instead of
        // requiring an empty slot, and answers both ways (the audit's silent
        // failures, UI_REDESIGN §1.3). The WEAR pane's instant equip is the primary
        // flow and skips the stamina toll; this ritual keeps its classic cost.
        ItemKind::Armor { stamina_cost, .. } => {
            let stamina_cost = *stamina_cost;
            if !pay_stamina(player, stamina_cost) {
                g.notifications
                    .push("Too tired to put that on.".to_string());
                return stackable_interact_on(g, item, false);
            }
            // wear one unit off the held stack; the count decrement below pays it
            let mut worn = item.clone();
            if worn.count() > 1 {
                worn.set_count(1);
            }
            let name = worn.get_name().to_string();
            let pd = player.player_mut();
            if let Some(prev) = pd.equip(worn) {
                pd.inventory.add_at(0, prev); // displaced gear returns to the pack
            }
            g.notifications.push(format!("Worn - {name}."));
            stackable_interact_on(g, item, true)
        }

        // ClothingItem.interactOn: put on clothes.
        ItemKind::Clothing { player_col, .. } => {
            let player_col = *player_col;
            if player.player().shirt_color == player_col {
                false
            } else {
                player.player_mut().shirt_color = player_col;
                stackable_interact_on(g, item, true)
            }
        }

        // PotionItem.interactOn: drink.
        ItemKind::Potion { ptype, .. } => {
            let ptype = *ptype;
            let applied = apply_potion(g, player, ptype, true);
            stackable_interact_on(g, item, applied)
        }

        // BookItem.interactOn: read.
        ItemKind::Book {
            book,
            has_title_page,
        } => {
            // A None book shows the default blank book. The registry stores the raw
            // asset text, so split it into pages here.
            let text = book.map(|b| {
                b.lines()
                    .collect::<Vec<_>>()
                    .join("\n")
                    .replace("\\0", "\0")
            });
            g.set_menu(crate::screen::book_display::BookDisplay::with_title(
                g,
                text.as_deref(),
                *has_title_page,
            ));
            true
        }

        // FurnitureItem.interactOn: place the furniture.
        ItemKind::Furniture { .. } => {
            let may_pass = {
                let ItemKind::Furniture { furniture, .. } = &item.kind else {
                    return false;
                };
                let tile = g.tile_at(lvl, xt, yt);
                tiles::may_pass(g, &tile, lvl, xt, yt, furniture)
            };
            if may_pass {
                // If the furniture can go on the tile
                let creative = g.is_mode("creative");
                let ItemKind::Furniture { furniture, placed } = &mut item.kind else {
                    return false;
                };
                // Placed furniture's X and Y positions
                furniture.c.x = xt * 16 + 8;
                furniture.c.y = yt * 16 + 8;
                let to_place = (**furniture).clone();
                g.level_mut(lvl).add(to_place, lvl); // adds the furniture to the world
                if creative {
                    // in creative the held item is refreshed with a brand-new instance
                    let fresh = furniture_clone(g, furniture);
                    **furniture = fresh;
                } else {
                    *placed = true; // the value becomes true, which removes it from the player's active item
                }

                return true;
            }
            // the tile refused it (water, rock, a tree...) — name the ground
            let tile_name = g.tile_at(lvl, xt, yt).name.clone();
            place_note(g, &format!("Won't sit on {}.", tile_name.to_lowercase()));
            false
        }

        // Scavenge-wave bottles: plain stackables with bespoke uses, name-matched
        // like the Throwing Knife in attack(). A full bottle is a modest drink...
        ItemKind::Stackable { .. } if item.get_name().eq_ignore_ascii_case("Water Bottle") => {
            use crate::entity::mob::player::MAX_STAMINA;
            if player.player().stamina >= MAX_STAMINA {
                return false; // already fresh — don't waste the water
            }
            let pd = player.player_mut();
            pd.stamina = (pd.stamina + 4).min(MAX_STAMINA);
            if !g.is_mode("creative") {
                swap_named_stack(g, item, player, "Empty Bottle");
            }
            g.notifications.push("Refreshing.".to_string());
            true
        }
        // ...and the empty refills at any open water (a hot spring counts — warm,
        // mineral, still water).
        ItemKind::Stackable { .. } if item.get_name().eq_ignore_ascii_case("Empty Bottle") => {
            let tile = g.tile_at(lvl, xt, yt);
            if tile.id != g.tiles.get("water").id
                && tile.id != g.tiles.get("Deep Water").id
                && !matches!(tile.kind, crate::level::tile::TileKind::SpringWater)
            {
                return false;
            }
            if !g.is_mode("creative") {
                swap_named_stack(g, item, player, "Water Bottle");
            }
            true
        }

        // Item.interactOn default (PowerGlove, plain stackables, unknown items).
        _ => false,
    }
}

/// Swap one unit of the active stack for a different registry item (the bottle
/// fill/drain pair) — the same split-one-off-the-stack mechanics as [`edit_bucket`].
fn swap_named_stack(g: &Game, item: &mut Item, player: &mut Entity, to: &str) {
    let count = item.count();
    if count == 0 {
        return; // shouldn't happen; a count of 0 already marks the item depleted
    }
    if count == 1 {
        *item = registry::get(g, to);
        return;
    }
    item.set_count(count - 1);
    player.player_mut().inventory.add(registry::get(g, to));
}

/// Java `Furniture.clone()` (`getClass().newInstance()`) and its Crafter/Spawner
/// overrides — builds a *fresh* furniture of the same kind, exactly like Java (e.g. a
/// cloned chest is empty, a cloned dungeon chest re-rolls its loot).
fn furniture_clone(g: &mut Game, f: &Entity) -> Entity {
    match &f.kind {
        EntityKind::Chest(_) => furniture::chest::new(),
        EntityKind::DeathChest(_) => furniture::death_chest::new(g),
        EntityKind::DungeonChest(_) => furniture::dungeon_chest::new(g),
        // a fresh scavenge container is shut but holds nothing (worldgen fills them)
        EntityKind::ScavContainer(sc) => furniture::scav_container::new(sc.kind),
        EntityKind::Bed(_) => furniture::bed::new(),
        // fire wave: a fresh clone is fully fueled and lit
        EntityKind::Campfire(_) => furniture::campfire::new(),
        EntityKind::Crafter(c) => furniture::crafter::new(c.crafter_type),
        EntityKind::Lantern(l) => furniture::lantern::new(l.lantern_type),
        EntityKind::Tnt(_) => furniture::tnt::new(),
        EntityKind::Spawner(s) => {
            let mob = (*s.mob).clone();
            let mut rnd = g.random.clone();
            let spawner = furniture::spawner::new(mob, &mut rnd);
            g.random = rnd;
            spawner
        }
        // plain furniture has no per-instance state; a clone is a fresh instance
        _ => f.clone(),
    }
}

/// Change one bucket's fill. Buckets are stackable, but only one should be changed at
/// a time; `item` is the player's active item and is mutated in place.
fn edit_bucket(item: &mut Item, player: &mut Entity, new_fill: Fill) {
    let count = item.count();
    if count == 0 {
        // shouldn't happen; a count of 0 already marks the item depleted
        return;
    }
    if count == 1 {
        *item = registry::new_bucket_item(new_fill);
        return;
    }

    // this item object is a stack of buckets.
    item.set_count(count - 1);
    player
        .player_mut()
        .inventory
        .add(registry::new_bucket_item(new_fill));
}

/// Java `PotionItem.applyPotion(player, type, time)`.
pub fn apply_potion_time(g: &mut Game, player: &mut Entity, ptype: PotionType, time: i32) -> bool {
    let result = apply_potion(g, player, ptype, time > 0);
    if result {
        player.player_mut().potioneffects.insert(ptype, time);
    }
    result
}

/// Java `PotionItem.applyPotion(player, type, addEffect)` — this method is seperate from
/// the above method b/c this is called sepeately by Load.java.
pub fn apply_potion(
    g: &mut Game,
    player: &mut Entity,
    ptype: PotionType,
    add_effect: bool,
) -> bool {
    if ptype == PotionType::None {
        return false; // regular potions don't do anything.
    }

    // if hasEffect, and is disabling, or doesn't have effect, and is enabling...
    if player.player().potioneffects.contains_key(&ptype) != add_effect {
        toggle_effect(g, player, ptype, add_effect);
    }

    if add_effect && ptype.duration() > 0 {
        player
            .player_mut()
            .potioneffects
            .insert(ptype, ptype.duration()); // add it
    } else {
        player.player_mut().potioneffects.remove(&ptype);
    }

    true
}

/// Java `PotionType.toggleEffect(player, addEffect)` — the per-type overrides.
fn toggle_effect(g: &mut Game, player: &mut Entity, ptype: PotionType, add_effect: bool) -> bool {
    match ptype {
        PotionType::Speed => {
            let pd = player.player_mut();
            // never drop below the base speed of 1 when the effect wears off
            pd.move_speed += if add_effect {
                1.0
            } else if pd.move_speed > 1.0 {
                -1.0
            } else {
                0.0
            };
            true
        }
        PotionType::Health => {
            if add_effect {
                crate::entity::behavior::heal(g, player, 5);
            }
            true
        }
        _ => true,
    }
}
