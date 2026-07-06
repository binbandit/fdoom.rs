//! Item use behaviors: Java `Item.interact` / `interactOn` and the subclass overrides
//! (Tool/Tile/Torch/Bucket/Food/Armor/Clothing/Potion/Book/Furniture/PowerGlove items).

use crate::core::game::Game;
use crate::entity::furniture;
use crate::entity::mob::player::{MAX_ARMOR, MAX_HUNGER};
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
        // JAVA PowerGloveItem.interact: if used on a piece of furniture, take it.
        if entity.is_furniture() {
            furniture_take(g, entity, player); // JAVA: f.take(player) (virtual)
            return true;
        }
        return false; // we were not given a furniture entity.
    }
    false // JAVA: Item.interact default
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
        // ToolItem.interactOn: fishing rod on water.
        ItemKind::Tool { ttype, .. } => {
            let ttype = *ttype;
            let tile = g.tile_at(lvl, xt, yt);
            if ttype == ToolType::FishingRod && tile.id == g.tiles.get("water").id {
                let creative = g.is_mode("creative");
                if item.pay_durability(creative) {
                    // JAVA: player.goFishing(player.x - 5, player.y - 5)
                    let (x, y) = (player.c.x - 5, player.c.y - 5);
                    crate::entity::mob::player_behavior::go_fishing(g, player, x, y);
                    return true;
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

            let mut note = String::new();
            // JAVA: the WALL and DOOR branches are identical
            if model.contains("WALL") || model.contains("DOOR") {
                note = format!(
                    "Can only be placed on {}!",
                    g.tiles.get_name(&valid_tiles[0])
                );
            } else if model.contains("BRICK") || model.contains("PLANK") {
                note = "Dig a hole first!".to_string();
            }

            if !note.is_empty() {
                g.notifications.push(note);
            }

            stackable_interact_on(g, item, false)
        }

        // TorchItem.interactOn: place the torch version of the tile.
        ItemKind::Torch { valid_tiles, .. } => {
            let valid_tiles = valid_tiles.clone();
            let tile = g.tile_at(lvl, xt, yt);
            if valid_tiles.contains(&tile.name) {
                let torch = g.tiles.get_torch_tile(tile);
                g.set_tile_default(lvl, xt, yt, &torch);
                return stackable_interact_on(g, item, true);
            }
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
            }

            stackable_interact_on(g, item, success)
        }

        // ArmorItem.interactOn: put on the armor.
        ItemKind::Armor {
            armor,
            stamina_cost,
            ..
        } => {
            let (armor, stamina_cost) = (*armor, *stamina_cost);
            let mut success = false;
            if player.player().cur_armor.is_none() && pay_stamina(player, stamina_cost) {
                let pd = player.player_mut();
                pd.cur_armor = Some(item.clone()); // set the current armor being worn to this.
                pd.armor = (armor * MAX_ARMOR as f32) as i32; // armor is how many hits are left
                success = true;
            }

            stackable_interact_on(g, item, success)
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
            // JAVA: Game.setMenu(new BookDisplay(book, hasTitlePage)) — book == null (None)
            // shows the default blank book. Java's `book` held the BookData.loadBook-
            // processed text; the registry stores the raw asset text, so process it here.
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
                    // JAVA: furniture = furniture.clone() — a fresh instance.
                    let fresh = furniture_clone(g, furniture);
                    **furniture = fresh;
                } else {
                    *placed = true; // the value becomes true, which removes it from the player's active item
                }

                return true;
            }
            false
        }

        // Item.interactOn default (PowerGlove, plain stackables, unknown items).
        _ => false,
    }
}

/// Java `Furniture.clone()` (`getClass().newInstance()`) and its Crafter/Spawner
/// overrides — builds a *fresh* furniture of the same kind, exactly like Java (e.g. a
/// cloned chest is empty, a cloned dungeon chest re-rolls its loot).
fn furniture_clone(g: &mut Game, f: &Entity) -> Entity {
    match &f.kind {
        EntityKind::Chest(_) => furniture::chest::new(),
        EntityKind::DeathChest(_) => furniture::death_chest::new(g),
        EntityKind::DungeonChest(_) => furniture::dungeon_chest::new(g),
        EntityKind::Bed(_) => furniture::bed::new(),
        EntityKind::Crafter(c) => furniture::crafter::new(c.crafter_type), // JAVA: Crafter.clone()
        EntityKind::Lantern(l) => furniture::lantern::new(l.lantern_type),
        EntityKind::Tnt(_) => furniture::tnt::new(),
        EntityKind::Spawner(s) => {
            // JAVA: Spawner.clone() → new Spawner(mob)
            let mob = (*s.mob).clone();
            let mut rnd = g.random.clone();
            let spawner = furniture::spawner::new(mob, &mut rnd);
            g.random = rnd;
            spawner
        }
        // JAVA: the Furniture fallback `new Furniture(name, sprite)`.
        _ => f.clone(),
    }
}

/// Java `BucketItem.editBucket(player, newFill)` — buckets are stackable, but only one
/// should be changed at a time. JAVA: it returned the item to assign to
/// `player.activeItem`; here `item` *is* the active item, so it is mutated in place.
fn edit_bucket(item: &mut Item, player: &mut Entity, new_fill: Fill) {
    let count = item.count();
    if count == 0 {
        // this honestly should never happen... (JAVA: returned null → active item cleared;
        // a count of 0 makes the item depleted, with the same effect)
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
        player.player_mut().potioneffects.insert(ptype, time); // JAVA: player.addPotionEffect(type, time)
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
            // JAVA: player.moveSpeed += addEffect ? 1 : (moveSpeed > 1 ? -1 : 0)
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
