//! Port of `fdoom.item.Recipe` and `fdoom.item.Recipes`.

use crate::core::game::Game;
use crate::item::{registry, Inventory, Item};

#[derive(Debug, Clone)]
pub struct Recipe {
    /// cost item name (uppercase) -> amount
    costs: Vec<(String, i32)>,
    product: String,
    amount: i32,
    can_craft: bool,
}

impl Recipe {
    /// Java `new Recipe(createdItem, reqItems...)` — "Name_amount" strings.
    pub fn new(created_item: &str, req_items: &[&str]) -> Recipe {
        let sep: Vec<&str> = created_item.split('_').collect();
        let product = sep[0].to_uppercase();
        let amount: i32 = sep[1].parse().unwrap();

        let mut costs: Vec<(String, i32)> = Vec::new();
        for req in req_items {
            let cur_sep: Vec<&str> = req.split('_').collect();
            let cur_item = cur_sep[0].to_uppercase();
            let amt: i32 = cur_sep[1].parse().unwrap();
            if let Some(existing) = costs.iter_mut().find(|(name, _)| *name == cur_item) {
                existing.1 += amt;
            } else {
                costs.push((cur_item, amt));
            }
        }

        Recipe { costs, product, amount, can_craft: false }
    }

    pub fn get_product(&self, g: &Game) -> Item {
        registry::get(g, &self.product)
    }

    pub fn product_name(&self) -> &str {
        &self.product
    }

    pub fn get_costs(&self) -> &[(String, i32)] {
        &self.costs
    }

    pub fn get_amount(&self) -> i32 {
        self.amount
    }

    pub fn get_can_craft(&self) -> bool {
        self.can_craft
    }

    /// Java `checkCanCraft(player)` — updates and returns the cached flag.
    pub fn check_can_craft(&mut self, g: &Game, inventory: &Inventory) -> bool {
        self.can_craft = self.can_craft_with(g, inventory);
        self.can_craft
    }

    fn can_craft_with(&self, g: &Game, inventory: &Inventory) -> bool {
        if g.is_mode("creative") {
            return true;
        }
        for (cost, amt) in &self.costs {
            if inventory.count(&registry::get(g, cost)) < *amt {
                return false;
            }
        }
        true
    }

    /// Java `craft(player)`.
    pub fn craft(&self, g: &Game, inventory: &mut Inventory) -> bool {
        if !self.can_craft_with(g, inventory) {
            return false;
        }

        if !g.is_mode("creative") {
            // remove the cost items from the inventory
            for (cost, amt) in &self.costs {
                inventory.remove_items(&registry::get(g, cost), *amt);
            }
        }

        // add the crafted items
        for _ in 0..self.amount {
            inventory.add(self.get_product(g));
        }

        true
    }
}

/// Java `Recipes` — the static recipe lists.
pub struct Recipes {
    pub anvil: Vec<Recipe>,
    pub oven: Vec<Recipe>,
    pub furnace: Vec<Recipe>,
    pub workbench: Vec<Recipe>,
    pub enchant: Vec<Recipe>,
    pub craft: Vec<Recipe>,
    pub loom: Vec<Recipe>,
}

impl Default for Recipes {
    fn default() -> Self {
        Self::new()
    }
}

impl Recipes {
    pub fn new() -> Recipes {
        let mut craft = Vec::new();
        let mut workbench = Vec::new();
        let mut loom = Vec::new();
        let mut anvil = Vec::new();
        let mut furnace = Vec::new();
        let mut oven = Vec::new();
        let mut enchant = Vec::new();

        craft.push(Recipe::new("Workbench_1", &["Wood_10"]));
        craft.push(Recipe::new("Torch_2", &["Wood_1", "coal_1"]));
        craft.push(Recipe::new("Grass Seeds_1", &["seeds_1", "Flower_2"]));
        craft.push(Recipe::new("plank_2", &["Wood_1"]));
        craft.push(Recipe::new("Plank Wall_1", &["plank_3"]));
        craft.push(Recipe::new("Wood Door_1", &["plank_5"]));

        workbench.push(Recipe::new("Torch_2", &["Wood_1", "coal_1"]));
        workbench.push(Recipe::new("Lantern_1", &["Wood_8", "slime_4", "glass_3"]));
        workbench.push(Recipe::new("Stone Brick_2", &["Stone_2"]));
        workbench.push(Recipe::new("Stone Wall_1", &["Stone Brick_3"]));
        workbench.push(Recipe::new("Stone Door_1", &["Stone Brick_5"]));
        workbench.push(Recipe::new("Oven_1", &["Stone_15"]));
        workbench.push(Recipe::new("Furnace_1", &["Stone_20"]));
        workbench.push(Recipe::new("Enchanter_1", &["Wood_5", "String_2", "Lapis_10"]));
        workbench.push(Recipe::new("Chest_1", &["Wood_20"]));
        workbench.push(Recipe::new("Anvil_1", &["iron_5"]));
        workbench.push(Recipe::new("Tnt_1", &["Gunpowder_10", "Sand_8"]));
        workbench.push(Recipe::new("Loom_1", &["Wood_10", "Wool_5"]));
        workbench.push(Recipe::new("Fishing Rod_1", &["Wood_5", "String_3"]));

        loom.push(Recipe::new("String_2", &["Wool_1"]));
        loom.push(Recipe::new("red wool_1", &["Wool_1", "rose_1"]));
        loom.push(Recipe::new("blue wool_1", &["Wool_1", "Lapis_1"]));
        loom.push(Recipe::new("green wool_1", &["Wool_1", "Cactus_1"]));
        loom.push(Recipe::new("yellow wool_1", &["Wool_1", "Flower_1"]));
        loom.push(Recipe::new("black wool_1", &["Wool_1", "coal_1"]));
        loom.push(Recipe::new("Bed_1", &["Wood_5", "Wool_3"]));

        loom.push(Recipe::new("blue clothes_1", &["cloth_5", "Lapis_1"]));
        loom.push(Recipe::new("green clothes_1", &["cloth_5", "Cactus_1"]));
        loom.push(Recipe::new("yellow clothes_1", &["cloth_5", "Flower_1"]));
        loom.push(Recipe::new("black clothes_1", &["cloth_5", "coal_1"]));
        loom.push(Recipe::new("orange clothes_1", &["cloth_5", "rose_1", "Flower_1"]));
        loom.push(Recipe::new("purple clothes_1", &["cloth_5", "Lapis_1", "rose_1"]));
        loom.push(Recipe::new("cyan clothes_1", &["cloth_5", "Lapis_1", "Cactus_1"]));
        loom.push(Recipe::new("reg clothes_1", &["cloth_5"]));

        workbench.push(Recipe::new("Wood Sword_1", &["Wood_5"]));
        workbench.push(Recipe::new("Wood Axe_1", &["Wood_5"]));
        workbench.push(Recipe::new("Wood Hoe_1", &["Wood_5"]));
        workbench.push(Recipe::new("Wood Pickaxe_1", &["Wood_5"]));
        workbench.push(Recipe::new("Wood Shovel_1", &["Wood_5"]));
        workbench.push(Recipe::new("Wood Bow_1", &["Wood_5", "string_2"]));
        workbench.push(Recipe::new("Rock Sword_1", &["Wood_5", "Stone_5"]));
        workbench.push(Recipe::new("Rock Axe_1", &["Wood_5", "Stone_5"]));
        workbench.push(Recipe::new("Rock Hoe_1", &["Wood_5", "Stone_5"]));
        workbench.push(Recipe::new("Rock Pickaxe_1", &["Wood_5", "Stone_5"]));
        workbench.push(Recipe::new("Rock Shovel_1", &["Wood_5", "Stone_5"]));
        workbench.push(Recipe::new("Rock Bow_1", &["Wood_5", "Stone_5", "string_2"]));

        workbench.push(Recipe::new("arrow_3", &["Wood_2", "Stone_2"]));
        workbench.push(Recipe::new("Leather Armor_1", &["leather_10"]));
        workbench.push(Recipe::new("Snake Armor_1", &["scale_15"]));

        loom.push(Recipe::new("Leather Armor_1", &["leather_10"]));

        anvil.push(Recipe::new("Iron Armor_1", &["iron_10"]));
        anvil.push(Recipe::new("Gold Armor_1", &["gold_10"]));
        anvil.push(Recipe::new("Gem Armor_1", &["gem_65"]));
        anvil.push(Recipe::new("Empty Bucket_1", &["iron_5"]));
        anvil.push(Recipe::new("Iron Lantern_1", &["iron_8", "slime_5", "glass_4"]));
        anvil.push(Recipe::new("Gold Lantern_1", &["gold_10", "slime_5", "glass_4"]));
        anvil.push(Recipe::new("Iron Sword_1", &["Wood_5", "iron_5"]));
        anvil.push(Recipe::new("Iron Claymore_1", &["Iron Sword_1", "shard_15"]));
        anvil.push(Recipe::new("Iron Axe_1", &["Wood_5", "iron_5"]));
        anvil.push(Recipe::new("Iron Hoe_1", &["Wood_5", "iron_5"]));
        anvil.push(Recipe::new("Iron Pickaxe_1", &["Wood_5", "iron_5"]));
        anvil.push(Recipe::new("Iron Shovel_1", &["Wood_5", "iron_5"]));
        anvil.push(Recipe::new("Iron Bow_1", &["Wood_5", "iron_5", "string_2"]));
        anvil.push(Recipe::new("Gold Sword_1", &["Wood_5", "gold_5"]));
        anvil.push(Recipe::new("Gold Claymore_1", &["Gold Sword_1", "shard_15"]));
        anvil.push(Recipe::new("Gold Axe_1", &["Wood_5", "gold_5"]));
        anvil.push(Recipe::new("Gold Hoe_1", &["Wood_5", "gold_5"]));
        anvil.push(Recipe::new("Gold Pickaxe_1", &["Wood_5", "gold_5"]));
        anvil.push(Recipe::new("Gold Shovel_1", &["Wood_5", "gold_5"]));
        anvil.push(Recipe::new("Gold Bow_1", &["Wood_5", "gold_5", "string_2"]));
        anvil.push(Recipe::new("Gem Sword_1", &["Wood_5", "gem_50"]));
        anvil.push(Recipe::new("Gem Claymore_1", &["Gem Sword_1", "shard_15"]));
        anvil.push(Recipe::new("Gem Axe_1", &["Wood_5", "gem_50"]));
        anvil.push(Recipe::new("Gem Hoe_1", &["Wood_5", "gem_50"]));
        anvil.push(Recipe::new("Gem Pickaxe_1", &["Wood_5", "gem_50"]));
        anvil.push(Recipe::new("Gem Shovel_1", &["Wood_5", "gem_50"]));
        anvil.push(Recipe::new("Gem Bow_1", &["Wood_5", "gem_50", "string_2"]));

        furnace.push(Recipe::new("iron_1", &["iron Ore_4", "coal_1"]));
        furnace.push(Recipe::new("gold_1", &["gold Ore_4", "coal_1"]));
        furnace.push(Recipe::new("glass_1", &["sand_4", "coal_1"]));

        oven.push(Recipe::new("cooked pork_1", &["raw pork_1", "coal_1"]));
        oven.push(Recipe::new("steak_1", &["raw beef_1", "coal_1"]));
        oven.push(Recipe::new("cooked fish_1", &["raw fish_1", "coal_1"]));
        oven.push(Recipe::new("bread_1", &["wheat_4"]));

        enchant.push(Recipe::new("Gold Apple_1", &["apple_1", "gold_8"]));
        enchant.push(Recipe::new("potion_1", &["glass_1", "Lapis_3"]));
        enchant.push(Recipe::new("speed potion_1", &["potion_1", "Cactus_5"]));
        enchant.push(Recipe::new("light potion_1", &["potion_1", "slime_5"]));
        enchant.push(Recipe::new("swim potion_1", &["potion_1", "raw fish_5"]));
        enchant.push(Recipe::new("haste potion_1", &["potion_1", "Wood_5", "Stone_5"]));
        enchant.push(Recipe::new("lava potion_1", &["potion_1", "Lava Bucket_1"]));
        enchant.push(Recipe::new("energy potion_1", &["potion_1", "gem_25"]));
        enchant.push(Recipe::new("regen potion_1", &["potion_1", "Gold Apple_1"]));
        enchant.push(Recipe::new("Health Potion_1", &["potion_1", "GunPowder_2", "Leather Armor_1"]));

        Recipes { anvil, oven, furnace, workbench, enchant, craft, loom }
    }
}
