//! Port of `fdoom.item.Recipe` and `fdoom.item.Recipes`.
//!
//! Adding a recipe is one line in the right station list in [`Recipes::new`]:
//!
//! ```text
//! r("Ruby Ring", "Ruby*2 + gold"),
//! ```
//!
//! See [`Recipe::parse`] for the spec syntax; docs/ADDING_CONTENT.md for the
//! progression rules on which station a recipe belongs at.

use crate::core::game::Game;
use crate::item::{Inventory, Item, registry};

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
    /// This is the wire format (saves/registry lookups); recipe *declarations*
    /// read better through [`Recipe::parse`].
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

        Recipe {
            costs,
            product,
            amount,
            can_craft: false,
        }
    }

    /// A recipe from a compact spec: costs are `+`-separated, `*N` sets an amount
    /// (default 1). Sugar over [`Recipe::new`] — identical matching and the same
    /// `"Name_count"` wire format underneath. Names are case-insensitive.
    ///
    /// ```
    /// # use fdoom::item::Recipe;
    /// Recipe::parse("Crude Spear", "Stick*2 + Cord + Sharp Stone");
    /// Recipe::parse("Stick*2", "Wood"); // one Wood yields two Sticks
    /// ```
    pub fn parse(product: &str, costs: &str) -> Recipe {
        fn part(s: &str) -> (&str, i32) {
            match s.split_once('*') {
                Some((name, n)) => (
                    name.trim(),
                    n.trim()
                        .parse()
                        .unwrap_or_else(|_| panic!("bad amount in recipe part {s:?}")),
                ),
                None => (s.trim(), 1),
            }
        }
        let (pname, pamt) = part(product);
        let costs: Vec<String> = costs
            .split('+')
            .map(|c| {
                let (name, n) = part(c);
                format!("{name}_{n}")
            })
            .collect();
        let cost_refs: Vec<&str> = costs.iter().map(String::as_str).collect();
        Recipe::new(&format!("{pname}_{pamt}"), &cost_refs)
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

/// Java `Recipes` — the static recipe lists, one per crafting station.
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

/// Shorthand for [`Recipe::parse`] inside the tables below.
fn r(product: &str, costs: &str) -> Recipe {
    Recipe::parse(product, costs)
}

impl Recipes {
    pub fn new() -> Recipes {
        // Personal crafting (no station): the bare-handed survival chain.
        // Punch tall grass for fibers (and the odd loose stone), punch trees for
        // sticks, then: fibers -> cord, knap stone -> sharp stone, lash the three
        // together into crude tools. Everything past crude needs the workbench.
        let craft = vec![
            r("Cord", "Grass Fibers*3"),
            r("Sharp Stone", "Stone*2"),
            r("Stick*2", "Wood"),
            r("Crude Axe", "Stick + Cord + Sharp Stone"),
            r("Crude Pickaxe", "Stick + Cord + Sharp Stone"),
            r("Crude Spear", "Stick*2 + Cord + Sharp Stone"),
            r("Throwing Knife", "Sharp Stone + Stick + Cord"),
            r("Slingshot", "Stick*2 + Cord*2"),
            r("Bandage", "Cord*2 + Grass Fibers*2"),
            r("Fishing Rod", "Stick + Cord*2"),
            // fossicking: pan the creeks, shore up the tunnels
            r("Prospector's Pan", "Stick*3 + Cord + Stone"),
            r("Timber Prop", "Wood*2 + Stick*2"),
            r("Workbench", "Wood*10 + Stone*2"),
            // carve a pumpkin around a torch — a placeable ember-light
            r("Jack-O-Lantern", "Pumpkin + Torch"),
            // no-cook trail food: mashed forage
            r("Fruit Medley", "Berry*2 + Apple"),
            r("Torch*2", "Wood + coal"),
            r("Grass Seeds", "seeds + Flower*2"),
            r("plank*2", "Wood"),
            r("Plank Wall", "plank*3"),
            r("Wood Door", "plank*5"),
        ];

        // Verbose assembly: wood/rock tools are hafted (sticks) and lashed (cord),
        // not conjured from raw logs. Bows take an extra cord for the string.
        // (Fishing Rod moved to personal crafting: Stick + Cord.)
        let workbench = vec![
            r("Torch*2", "Wood + coal"),
            r("Lantern", "Wood*8 + slime*4 + glass*3"),
            r("Stone Brick*2", "Stone*2"),
            r("Stone Wall", "Stone Brick*3"),
            r("Stone Door", "Stone Brick*5"),
            // a paned wall segment: two panes leaded into a wooden frame
            r("Window", "glass*2 + Wood*2"),
            r("Oven", "Stone*15"),
            r("Furnace", "Stone*20"),
            r("Enchanter", "Wood*5 + String*2 + Lapis*10"),
            r("Chest", "Wood*20"),
            r("Anvil", "iron*5"),
            r("Tnt", "Gunpowder*10 + Sand*8"),
            r("Loom", "Wood*10 + Wool*5"),
            r("Wood Sword", "Wood*5 + Stick*2 + Cord"),
            r("Wood Axe", "Wood*5 + Stick*2 + Cord"),
            // crossing deep water (multi-level terrain)
            r("Raft", "Wood*10 + Cord*2"),
            r("Wood Hoe", "Wood*5 + Stick*2 + Cord"),
            r("Wood Pickaxe", "Wood*5 + Stick*2 + Cord"),
            r("Wood Shovel", "Wood*5 + Stick*2 + Cord"),
            r("Wood Bow", "Wood*5 + Stick*2 + Cord*2"),
            r("Rock Sword", "Stone*5 + Stick*2 + Cord"),
            r("Rock Axe", "Stone*5 + Stick*2 + Cord"),
            r("Rock Hoe", "Stone*5 + Stick*2 + Cord"),
            r("Rock Pickaxe", "Stone*5 + Stick*2 + Cord"),
            r("Rock Shovel", "Stone*5 + Stick*2 + Cord"),
            r("Rock Bow", "Wood*5 + Stone*5 + Cord*2"),
            r("Wood Spear", "Wood*5 + Stick*2 + Cord"),
            r("Rock Spear", "Stone*5 + Stick*2 + Cord"),
            // assembled weapon: wooden stock + lashings around an anvil-forged mechanism
            r("Crossbow", "Wood*5 + Stick*2 + Cord*2 + Crossbow Mechanism"),
            r("arrow*3", "Wood*2 + Stone*2"),
            r("Leather Armor", "leather*10"),
            r("Snake Armor", "scale*15"),
        ];

        let loom = vec![
            r("String*2", "Wool"),
            r("red wool", "Wool + rose"),
            r("blue wool", "Wool + Lapis"),
            r("green wool", "Wool + Cactus"),
            r("yellow wool", "Wool + Flower"),
            r("black wool", "Wool + coal"),
            r("Bed", "Wood*5 + Wool*3"),
            r("blue clothes", "cloth*5 + Lapis"),
            r("green clothes", "cloth*5 + Cactus"),
            r("yellow clothes", "cloth*5 + Flower"),
            r("black clothes", "cloth*5 + coal"),
            r("orange clothes", "cloth*5 + rose + Flower"),
            r("purple clothes", "cloth*5 + Lapis + rose"),
            r("cyan clothes", "cloth*5 + Lapis + Cactus"),
            r("reg clothes", "cloth*5"),
            r("Leather Armor", "leather*10"),
        ];

        let anvil = vec![
            r("Iron Armor", "iron*10"),
            r("Gold Armor", "gold*10"),
            r("Gem Armor", "gem*65"),
            r("Empty Bucket", "iron*5"),
            r("Iron Lantern", "iron*8 + slime*5 + glass*4"),
            r("Gold Lantern", "gold*10 + slime*5 + glass*4"),
            r("Iron Sword", "Wood*5 + iron*5"),
            r("Iron Claymore", "Iron Sword + shard*15"),
            r("Iron Axe", "Wood*5 + iron*5"),
            r("Iron Hoe", "Wood*5 + iron*5"),
            r("Iron Pickaxe", "Wood*5 + iron*5"),
            r("Iron Shovel", "Wood*5 + iron*5"),
            r("Iron Bow", "Wood*5 + iron*5 + string*2"),
            r("Iron Spear", "Wood*5 + iron*5"),
            // the crossbow's forged trigger/gear half; assembled at the workbench
            r("Crossbow Mechanism", "iron*3"),
            r("Gold Sword", "Wood*5 + gold*5"),
            r("Gold Claymore", "Gold Sword + shard*15"),
            r("Gold Axe", "Wood*5 + gold*5"),
            r("Gold Hoe", "Wood*5 + gold*5"),
            r("Gold Pickaxe", "Wood*5 + gold*5"),
            r("Gold Shovel", "Wood*5 + gold*5"),
            r("Gold Bow", "Wood*5 + gold*5 + string*2"),
            r("Gold Spear", "Wood*5 + gold*5"),
            r("Gem Sword", "Wood*5 + gem*50"),
            r("Gem Claymore", "Gem Sword + shard*15"),
            r("Gem Axe", "Wood*5 + gem*50"),
            r("Gem Hoe", "Wood*5 + gem*50"),
            r("Gem Pickaxe", "Wood*5 + gem*50"),
            r("Gem Shovel", "Wood*5 + gem*50"),
            r("Gem Bow", "Wood*5 + gem*50 + string*2"),
            r("Gem Spear", "Wood*5 + gem*50"),
        ];

        let furnace = vec![
            r("iron", "iron Ore*4 + coal"),
            r("gold", "gold Ore*4 + coal"),
            // light & shelter: cheapened from sand*4 so windows (glass*2 each) are
            // an early-house build, not a late-game luxury; coal is the fuel, as in
            // the ore smelts above
            r("glass", "sand*2 + coal"),
            // roast forage — available at either heat station (also in the oven list)
            r("Cooked Mushroom", "Mushroom + coal"),
        ];

        let oven = vec![
            r("cooked pork", "raw pork + coal"),
            r("steak", "raw beef + coal"),
            r("cooked fish", "raw fish + coal"),
            r("bread", "wheat*4"),
            r("Cooked Mushroom", "Mushroom + coal"),
        ];

        let enchant = vec![
            r("Gold Apple", "apple + gold*8"),
            r("potion", "glass + Lapis*3"),
            r("speed potion", "potion + Cactus*5"),
            r("light potion", "potion + slime*5"),
            r("swim potion", "potion + raw fish*5"),
            r("haste potion", "potion + Wood*5 + Stone*5"),
            r("lava potion", "potion + Lava Bucket"),
            r("energy potion", "potion + gem*25"),
            r("regen potion", "potion + Gold Apple"),
            r("Health Potion", "potion + GunPowder*2 + Leather Armor"),
        ];

        Recipes {
            anvil,
            oven,
            furnace,
            workbench,
            enchant,
            craft,
            loom,
        }
    }
}
