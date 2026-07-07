//! Port of `fdoom.item.Inventory`.
//!
//! The player's inventory in Java was an anonymous subclass with creative-mode add/remove
//! overrides; here that's the `player_inv` flag plus a `creative` flag refreshed by the
//! player tick (Java read `Game.isMode("creative")` live; see PORTING.md).

use crate::item::{Item, ItemKind};
use crate::rng::Rng;

#[derive(Debug, Clone, Default)]
pub struct Inventory {
    items: Vec<Item>,
    player_inv: bool,
    pub creative: bool,
}

impl Inventory {
    pub fn new() -> Inventory {
        Inventory::default()
    }

    /// The player's inventory (Java's anonymous `Inventory` subclass in Player).
    pub fn new_player() -> Inventory {
        Inventory {
            player_inv: true,
            ..Inventory::default()
        }
    }

    /// Java `getItems()` (borrowing instead of copying; use `.to_vec()` when a copy is
    /// really needed).
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn clear_inv(&mut self) {
        self.items.clear();
    }

    /// Java `invSize()`.
    pub fn inv_size(&self) -> i32 {
        self.items.len() as i32
    }

    pub fn get(&self, idx: i32) -> &Item {
        &self.items[idx as usize]
    }

    pub fn get_mut(&mut self, idx: i32) -> &mut Item {
        &mut self.items[idx as usize]
    }

    /// Java `remove(idx)` — includes the Player-subclass creative behavior.
    pub fn remove(&mut self, idx: i32) -> Item {
        if self.player_inv && self.creative {
            // creative slots are bottomless: hand out a single item and keep the slot
            if self.items[idx as usize].is_stackable() {
                self.items[idx as usize].set_count(1);
            }
            let cur = self.items[idx as usize].clone();
            if self.count(&cur) == 1 {
                let cur = self.items.remove(idx as usize);
                self.add_base(0, cur.clone());
                return cur;
            }
        }
        self.items.remove(idx as usize)
    }

    /// Java `addAll(other)`.
    pub fn add_all(&mut self, other: &Inventory) {
        for i in other.items.iter() {
            self.add(i.clone());
        }
    }

    /// Java `add(item)`.
    pub fn add(&mut self, item: Item) {
        self.add_at(self.items.len() as i32, item);
    }

    /// Java `add(item, num)`.
    pub fn add_num(&mut self, item: Item, num: i32) {
        for _ in 0..num {
            self.add(item.clone());
        }
    }

    /// Java `add(slot, item)` — includes the Player-subclass creative behavior.
    pub fn add_at(&mut self, slot: i32, item: Item) {
        if self.player_inv && self.creative {
            let mut item = item;
            if self.count(&item) > 0 {
                return;
            }
            if item.is_stackable() {
                item.set_count(1);
            }
            self.add_base(slot, item);
            return;
        }
        self.add_base(slot, item);
    }

    fn add_base(&mut self, slot: i32, item: Item) {
        if matches!(item.kind, ItemKind::PowerGlove) {
            println!("WARNING: tried to add power glove to inventory.");
            return; // do NOT add to inventory
        }

        if item.is_stackable() {
            let to_take = item;
            for existing in self.items.iter_mut() {
                if to_take.stacks_with(existing) {
                    let add = to_take.count();
                    existing.set_count(existing.count() + add);
                    return;
                }
            }
            self.items.insert(slot as usize, to_take);
        } else {
            self.items.insert(slot as usize, item);
        }
    }

    /// Java `removeFromStack(given, count)` — returns amount removed.
    fn remove_from_stack(&mut self, given: &Item, count: i32) -> i32 {
        let mut removed = 0;
        let mut i = 0;
        while i < self.items.len() {
            if !self.items[i].is_stackable() || !self.items[i].stacks_with(given) {
                i += 1;
                continue;
            }
            let cur_count = self.items[i].count();
            let amount_removing = (count - removed).min(cur_count);
            self.items[i].set_count(cur_count - amount_removing);
            if self.items[i].count() == 0 {
                self.items.remove(i);
            } else {
                i += 1;
            }
            removed += amount_removing;
            if removed == count {
                break;
            }
            if removed > count {
                println!(
                    "SCREW UP while removing items from stack: {} too many.",
                    removed - count
                );
                break;
            }
        }
        if removed < count {
            println!(
                "Inventory: could not remove all items; {} left.",
                count - removed
            );
        }
        removed
    }

    /// Java `removeItem(i)` — removes entirely, stack or lone item.
    pub fn remove_item(&mut self, i: &Item) {
        if i.is_stackable() {
            self.remove_items(i, i.count());
        } else {
            self.remove_items(i, 1);
        }
    }

    /// Java `removeItems(given, count)`.
    pub fn remove_items(&mut self, given: &Item, mut count: i32) {
        if given.is_stackable() {
            count -= self.remove_from_stack(given, count);
        } else {
            let mut i = 0;
            while i < self.items.len() {
                if self.items[i].item_equals(given) {
                    self.items.remove(i);
                    count -= 1;
                    if count == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        }
        if count > 0 {
            println!(
                "WARNING: could not remove {count} {given}{} from inventory",
                if count > 1 { "s" } else { "" }
            );
        }
    }

    /// Java `count(given)`.
    pub fn count(&self, given: &Item) -> i32 {
        let mut found = 0;
        for cur_item in self.items.iter() {
            if cur_item.is_stackable() && cur_item.stacks_with(given) {
                found += cur_item.count();
            } else if cur_item.item_equals(given) {
                found += 1;
            }
        }
        found
    }

    /// Java `getItemData()` — save/network string.
    pub fn get_item_data(&self) -> String {
        self.items
            .iter()
            .map(|i| i.get_data())
            .collect::<Vec<_>>()
            .join(":")
    }

    /// Java `tryAdd(chance, item, num, allOrNothing)`.
    pub fn try_add_all_or_nothing(
        &mut self,
        random: &mut Rng,
        chance: i32,
        item: &Item,
        num: i32,
        all_or_nothing: bool,
    ) {
        if !all_or_nothing || random.next_int_bound(chance) == 0 {
            for _ in 0..num {
                if all_or_nothing || random.next_int_bound(chance) == 0 {
                    self.add(item.clone());
                }
            }
        }
    }

    /// Java `tryAdd(chance, item, num)`.
    pub fn try_add_num(&mut self, random: &mut Rng, chance: i32, item: Option<Item>, num: i32) {
        let Some(mut item) = item else { return };
        if item.is_stackable() {
            item.set_count(item.count() * num);
            self.try_add_all_or_nothing(random, chance, &item, 1, true);
        } else {
            self.try_add_all_or_nothing(random, chance, &item, num, false);
        }
    }

    /// Java `tryAdd(chance, item)`.
    pub fn try_add(&mut self, random: &mut Rng, chance: i32, item: Option<Item>) {
        self.try_add_num(random, chance, item, 1);
    }
}
