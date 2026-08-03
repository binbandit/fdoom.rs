//! Interned item names — the compile-time-checked way to name a registry item.
//!
//! `registry::get(g, "Iron Ore")` resolves a string at runtime, and a typo is invisible:
//! the registry prints a warning and hands back an `UnknownItem`, so a bad drop table
//! ships as a gameplay bug rather than a build failure. [`ItemName`] fixes the naming
//! half of that — `iname::IRON_ORE` is checked by the compiler, and
//! `tests/tile_ids.rs::every_item_constant_resolves` proves every constant here still
//! matches something the registry actually builds.
//!
//! Unlike tiles, items have no stable numeric id to intern to: the registry is a `Vec`
//! built at world start and items are identified by name everywhere, including on disk.
//! So these stay names — the win is compile-time checking, not a faster lookup. Reach for
//! [`registry::get_checked`](super::registry::get_checked) wherever a name genuinely
//! arrives at runtime.

/// The name of an item in the registry, as a checked constant.
///
/// Construction is deliberately limited to the constants below, so an `ItemName` in hand
/// is a name the test suite has proven resolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemName(&'static str);

impl ItemName {
    #[inline]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl std::fmt::Display for ItemName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// Declares the constants and the table the tests check the live registry against.
macro_rules! item_names {
    ($($konst:ident = $name:literal;)*) => {
        $(pub const $konst: ItemName = ItemName($name);)*

        /// Every constant above. Used by `tests/tile_ids.rs` to prove each one resolves.
        pub const ALL: &[ItemName] = &[$($konst,)*];
    };
}

item_names! {
    ACORN = "Acorn";
    ADDER_SPAWNER = "Adder Spawner";
    ANTIDIOUS = "Antidious";
    ANVIL = "Anvil";
    APPLE = "Apple";
    ARROW = "arrow";
    ASSAY_KIT = "Assay Kit";
    BAKED_POTATO = "Baked Potato";
    BANDAGE = "Bandage";
    BARREL = "Barrel";
    BED = "Bed";
    BENCH = "Bench";
    BERRY = "Berry";
    BIG_FISH = "Big Fish";
    BLACK_CLOTHES = "Black Clothes";
    BLACK_WOOL = "Black Wool";
    BLUE_CLOTHES = "Blue Clothes";
    BLUE_WOOL = "Blue Wool";
    BONE = "Bone";
    BOOK = "Book";
    BREAD = "Bread";
    CACTUS = "Cactus";
    CACTUS_FRUIT = "Cactus Fruit";
    CAMPFIRE = "Campfire";
    CARROT = "Carrot";
    CARROT_SEEDS = "Carrot Seeds";
    CAVE_EEL = "Cave Eel";
    CHEST = "Chest";
    CLOTH = "cloth";
    CLOUD = "Cloud";
    COAL = "Coal";
    COCONUT = "Coconut";
    COOKED_BIG_FISH = "Cooked Big Fish";
    COOKED_CAVE_EEL = "Cooked Cave Eel";
    COOKED_FISH = "Cooked Fish";
    COOKED_MUSHROOM = "Cooked Mushroom";
    COOKED_PORK = "Cooked Pork";
    COOKED_VENISON = "Cooked Venison";
    CORD = "Cord";
    CORN = "Corn";
    CORN_KERNELS = "Corn Kernels";
    COW_SPAWNER = "Cow Spawner";
    CROSSBOW = "Crossbow";
    CROSSBOW_MECHANISM = "Crossbow Mechanism";
    CRUDE_AXE = "Crude Axe";
    CRUDE_BOW = "Crude Bow";
    CRUDE_CLAYMORE = "Crude Claymore";
    CRUDE_HOE = "Crude Hoe";
    CRUDE_PICKAXE = "Crude Pickaxe";
    CRUDE_SHOVEL = "Crude Shovel";
    CRUDE_SPEAR = "Crude Spear";
    CRUDE_SWORD = "Crude Sword";
    CUPBOARD = "Cupboard";
    CYAN_CLOTHES = "Cyan Clothes";
    DIRT = "Dirt";
    EMPTY_BOTTLE = "Empty Bottle";
    EMPTY_BUCKET = "Empty Bucket";
    EMPTY_CAN = "Empty Can";
    ENCHANTER = "Enchanter";
    ENERGY_POTION = "Energy Potion";
    FERAL_HOUND_SPAWNER = "FeralHound Spawner";
    FISHING_ROD = "Fishing Rod";
    FISH_CHOWDER = "Fish Chowder";
    FLOWER = "Flower";
    FRUIT_MEDLEY = "Fruit Medley";
    FUR = "Fur";
    FURNACE = "Furnace";
    FUR_COAT = "Fur Coat";
    GEM = "gem";
    GEM_ARMOR = "Gem Armor";
    GEM_AXE = "Gem Axe";
    GEM_BOW = "Gem Bow";
    GEM_CLAYMORE = "Gem Claymore";
    GEM_HOE = "Gem Hoe";
    GEM_PICKAXE = "Gem Pickaxe";
    GEM_SHOVEL = "Gem Shovel";
    GEM_SPEAR = "Gem Spear";
    GEM_SWORD = "Gem Sword";
    GHOST_SPAWNER = "Ghost Spawner";
    GLASS = "glass";
    GLOW_WORM_SPAWNER = "GlowWorm Spawner";
    GOLD = "Gold";
    GOLD_APPLE = "Gold Apple";
    GOLD_ARMOR = "Gold Armor";
    GOLD_AXE = "Gold Axe";
    GOLD_BOW = "Gold Bow";
    GOLD_CLAYMORE = "Gold Claymore";
    GOLD_HOE = "Gold Hoe";
    GOLD_LANTERN = "Gold Lantern";
    GOLD_ORE = "Gold Ore";
    GOLD_PICKAXE = "Gold Pickaxe";
    GOLD_SHOVEL = "Gold Shovel";
    GOLD_SPEAR = "Gold Spear";
    GOLD_SWORD = "Gold Sword";
    GRASS_FIBERS = "Grass Fibers";
    GRASS_SEEDS = "Grass Seeds";
    GRASS_SNAKE_SPAWNER = "GrassSnake Spawner";
    GREEN_CLOTHES = "Green Clothes";
    GREEN_WOOL = "Green Wool";
    GUN_POWDER = "GunPowder";
    HASTE_POTION = "Haste Potion";
    HEALTH_POTION = "Health Potion";
    HEARTY_STEW = "Hearty Stew";
    HIDE = "Hide";
    HONEYCOMB = "Honeycomb";
    HONEY_GLAZED_FISH = "Honey-Glazed Fish";
    HONEY_JAR = "Honey Jar";
    IRON = "Iron";
    IRON_ARMOR = "Iron Armor";
    IRON_AXE = "Iron Axe";
    IRON_BOW = "Iron Bow";
    IRON_CLAYMORE = "Iron Claymore";
    IRON_HOE = "Iron Hoe";
    IRON_LANTERN = "Iron Lantern";
    IRON_ORE = "Iron Ore";
    IRON_PICKAXE = "Iron Pickaxe";
    IRON_SHOVEL = "Iron Shovel";
    IRON_SPEAR = "Iron Spear";
    IRON_SWORD = "Iron Sword";
    JACK_O_LANTERN = "Jack-O-Lantern";
    KEY = "Key";
    KNIGHT_SPAWNER = "Knight Spawner";
    LANTERN = "Lantern";
    LAPIS = "Lapis";
    LAVA_BUCKET = "Lava Bucket";
    LAVA_POTION = "Lava Potion";
    LEATHER = "Leather";
    LEATHER_ARMOR = "Leather Armor";
    LIGHT_POTION = "Light Potion";
    LOOM = "Loom";
    MARSH_LURKER_SPAWNER = "MarshLurker Spawner";
    MUSHROOM = "Mushroom";
    MUSHROOM_SKEWER = "Mushroom Skewer";
    NIGHT_WISP_SPAWNER = "NightWisp Spawner";
    OBSIDIAN_BRICK = "Obsidian Brick";
    OBSIDIAN_DOOR = "Obsidian Door";
    OBSIDIAN_WALL = "Obsidian Wall";
    OLD_COIN = "Old Coin";
    OLD_FOOD_CAN = "Old Food Can";
    ORANGE_CLOTHES = "Orange Clothes";
    OVEN = "Oven";
    PIG_SPAWNER = "Pig Spawner";
    PLANK = "Plank";
    PLANK_WALL = "Plank Wall";
    PORK_CHOP = "Pork Chop";
    POTATO = "Potato";
    POTION = "Potion";
    POWER_GLOVE = "Power Glove";
    FLETCHERS_DIARY = "Fletcher's Diary";
    PROSPECTORS_NOTE = "Prospector's Note";
    PROSPECTORS_PAN = "Prospector's Pan";
    PUMPKIN = "Pumpkin";
    PUMPKIN_SEEDS = "Pumpkin Seeds";
    PURPLE_CLOTHES = "Purple Clothes";
    RAFT = "Raft";
    RATTLER_SPAWNER = "Rattler Spawner";
    RAW_BEEF = "Raw Beef";
    RAW_FISH = "Raw Fish";
    RAW_PORK = "Raw Pork";
    RED_CLOTHES = "Red Clothes";
    RED_WOOL = "Red Wool";
    REGEN_POTION = "Regen Potion";
    REG_CLOTHES = "Reg Clothes";
    ROASTED_SKEWER = "Roasted Skewer";
    ROAST_CORN = "Roast Corn";
    ROAST_PUMPKIN = "Roast Pumpkin";
    ROCK_AXE = "Rock Axe";
    ROCK_BOW = "Rock Bow";
    ROCK_CLAYMORE = "Rock Claymore";
    ROCK_HOE = "Rock Hoe";
    ROCK_PICKAXE = "Rock Pickaxe";
    ROCK_SHOVEL = "Rock Shovel";
    ROCK_SPEAR = "Rock Spear";
    ROCK_SWORD = "Rock Sword";
    ROSE = "Rose";
    SAND = "Sand";
    SCALE = "Scale";
    SEEDS = "Seeds";
    SEED_POTATO = "Seed Potato";
    SHARD = "Shard";
    SHARP_STONE = "Sharp Stone";
    SHEEP_SPAWNER = "Sheep Spawner";
    SHIELD_POTION = "Shield Potion";
    SLIME = "Slime";
    SLINGSHOT = "Slingshot";
    SNAKE_ARMOR = "Snake Armor";
    SNAKE_SPAWNER = "Snake Spawner";
    SPEED_POTION = "Speed Potion";
    SPINDLE = "Spindle";
    STEAK = "Steak";
    STICK = "Stick";
    STONE = "Stone";
    STONE_BRICK = "Stone Brick";
    STONE_DOOR = "Stone Door";
    STONE_GOLEM_SPAWNER = "StoneGolem Spawner";
    STONE_WALL = "Stone Wall";
    STRAW_HAT = "Straw Hat";
    STRING = "string";
    SUPPLY_CRATE = "Supply Crate";
    SWIM_POTION = "Swim Potion";
    THROWING_KNIFE = "Throwing Knife";
    TIMBER_PROP = "Timber Prop";
    TIME_POTION = "Time Potion";
    TIN = "Tin";
    TNT = "Tnt";
    TORCH = "Torch";
    VENISON = "Venison";
    VICE = "Vice";
    WATER_BOTTLE = "Water Bottle";
    WATER_BUCKET = "Water Bucket";
    WHEAT = "Wheat";
    WINDOW = "Window";
    WOOD = "Wood";
    WOOD_AXE = "Wood Axe";
    WOOD_BOW = "Wood Bow";
    WOOD_CLAYMORE = "Wood Claymore";
    TANNERS_NOTES = "Tanner's Notes";
    TRAPPERS_FIELD_GUIDE = "Trapper's Field Guide";
    WICKMAKERS_PAGE = "Wickmaker's Page";
    WOOD_DOOR = "Wood Door";
    WOOD_HOE = "Wood Hoe";
    WOOD_PICKAXE = "Wood Pickaxe";
    WOOD_SHOVEL = "Wood Shovel";
    WOOD_SPEAR = "Wood Spear";
    WOOD_SWORD = "Wood Sword";
    WOOL = "Wool";
    WORKBENCH = "Workbench";
    YELLOW_CLOTHES = "Yellow Clothes";
    YELLOW_WOOL = "Yellow Wool";
    ZOMBIE_SPAWNER = "Zombie Spawner";
}
