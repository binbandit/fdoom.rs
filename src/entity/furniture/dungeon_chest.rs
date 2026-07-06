//! Port of `fdoom.entity.furniture.DungeonChest`.

use crate::core::game::Game;
use crate::entity::{Entity, EntityKind};
use crate::gfx::color;

use super::chest::ChestData;
use super::furniture_common;

pub fn open_col() -> i32 {
    color::get4(-1, 2, 115, 225)
}
pub fn lock_col() -> i32 {
    color::get4(-1, 222, 333, 555)
}

#[derive(Debug, Clone)]
pub struct DungeonChestData {
    pub chest: ChestData,
    pub is_locked: bool,
}

/// Java `new DungeonChest()` — populates its inventory with random loot.
pub fn new(g: &mut Game) -> Entity {
    let mut chest = ChestData::with_name("Dungeon Chest", lock_col());
    populate_inv(g, &mut chest);
    let c = furniture_common(chest.furniture.sprite.color, 3, 3);
    Entity::new(
        c,
        EntityKind::DungeonChest(DungeonChestData {
            chest,
            is_locked: true,
        }),
    )
}

/// Java `populateInv()` — populate the inventory of the DungeonChest, psudo-randomly.
pub fn populate_inv(g: &mut Game, chest: &mut ChestData) {
    use crate::item::registry::get;

    let inv = &mut chest.inventory; // Yes, I'm that lazy. ;P
    inv.clear_inv(); // clear the inventory.
    let mut rnd = g.random.clone();
    inv.try_add_num(&mut rnd, 5, Some(get(g, "steak")), 6);
    inv.try_add_num(&mut rnd, 5, Some(get(g, "cooked pork")), 6);
    inv.try_add_num(&mut rnd, 4, Some(get(g, "Wood")), 20);
    inv.try_add_num(&mut rnd, 4, Some(get(g, "Wool")), 12);
    inv.try_add_num(&mut rnd, 2, Some(get(g, "coal")), 4);
    inv.try_add_num(&mut rnd, 5, Some(get(g, "gem")), 7);
    inv.try_add_num(&mut rnd, 5, Some(get(g, "gem")), 8);
    inv.try_add(&mut rnd, 8, Some(get(g, "Gem Armor")));
    inv.try_add(&mut rnd, 6, Some(get(g, "Gold Armor")));
    inv.try_add_num(&mut rnd, 5, Some(get(g, "Iron Armor")), 2);
    inv.try_add_num(&mut rnd, 3, Some(get(g, "potion")), 10);
    inv.try_add_num(&mut rnd, 4, Some(get(g, "speed potion")), 2);
    inv.try_add_num(&mut rnd, 6, Some(get(g, "speed potion")), 5);
    inv.try_add_num(&mut rnd, 3, Some(get(g, "light potion")), 2);
    inv.try_add_num(&mut rnd, 4, Some(get(g, "light potion")), 3);
    inv.try_add(&mut rnd, 7, Some(get(g, "regen potion")));
    inv.try_add(&mut rnd, 7, Some(get(g, "energy potion")));
    inv.try_add(&mut rnd, 14, Some(get(g, "time potion")));
    inv.try_add(&mut rnd, 14, Some(get(g, "shield potion")));
    inv.try_add(&mut rnd, 7, Some(get(g, "lava potion")));
    inv.try_add_num(&mut rnd, 5, Some(get(g, "haste potion")), 3);

    inv.try_add(&mut rnd, 6, Some(get(g, "Gold Bow")));
    inv.try_add(&mut rnd, 7, Some(get(g, "Gem Bow")));
    inv.try_add(&mut rnd, 4, Some(get(g, "Gold Sword")));
    inv.try_add(&mut rnd, 7, Some(get(g, "Gem Sword")));
    inv.try_add(&mut rnd, 4, Some(get(g, "Rock Claymore")));
    inv.try_add(&mut rnd, 6, Some(get(g, "Iron Claymore")));
    g.random = rnd;

    if inv.inv_size() < 1 {
        // add this if none of the above was added.
        inv.add_num(get(g, "steak"), 6);
        inv.add(get(g, "Time Potion"));
        inv.add(get(g, "Gem Axe"));
    }
}
