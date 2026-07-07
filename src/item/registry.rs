//! Port of `fdoom.item.Items` and each item class's `getAllInstances()` prototype lists.
//!
//! Java built the list once in a static block; here `build_registry` runs once per game
//! (from `Game::new`) since the furniture prototypes read the difficulty setting, exactly
//! as Java's static init read `Settings` at class-load time.
//!
//! Adding an item is one line in `build_registry` (list order = creative-inventory
//! order): a family helper + the sheet cell `(x, y)` + a `get4` palette, e.g.
//! `stackable("Ruby", (10, 4), get4(-1, 400, 500, 511))`. See docs/ADDING_CONTENT.md.

use crate::core::game::Game;
use crate::entity::Entity;
use crate::entity::furniture::crafter::CrafterType;
use crate::entity::furniture::lantern::LanternType;
use crate::entity::{furniture, mob};
use crate::gfx::color::get4;
use crate::gfx::{Sprite, color};
use crate::item::{Fill, Inventory, Item, ItemKind, PotionType, ToolType};

/// Java `ToolItem.LEVEL_NAMES`, extended post-port with the "Crude" tier at level 0
/// (knapped stone-and-cord tools, hand-craftable with no station). Wood..Gem shifted
/// up one level; tool names are unchanged, so recipes and old saves keep working.
pub const TOOL_LEVEL_NAMES: [&str; 6] = ["Crude", "Wood", "Rock", "Iron", "Gold", "Gem"];

/// Java `ToolItem.LEVEL_COLORS` (+ crude tier).
pub const TOOL_LEVEL_COLORS: [i32; 6] = [
    get4(-1, 100, 221, 332), // crude (dull stone-on-stick)
    get4(-1, 100, 321, 431), // wood
    get4(-1, 100, 321, 111), // rock/stone
    get4(-1, 100, 321, 555), // iron
    get4(-1, 100, 321, 550), // gold
    get4(-1, 100, 321, 55),  // gem
];

/// Java `ToolItem.BOW_COLORS` (+ crude tier).
pub const TOOL_BOW_COLORS: [i32; 6] = [
    get4(-1, 100, 444, 332),
    get4(-1, 100, 444, 431),
    get4(-1, 100, 444, 111),
    get4(-1, 100, 444, 555),
    get4(-1, 100, 444, 550),
    get4(-1, 100, 444, 55),
];

fn tool_color(ttype: ToolType, level: i32) -> i32 {
    if ttype == ToolType::Bow {
        TOOL_BOW_COLORS[level as usize]
    } else if ttype == ToolType::FishingRod {
        get4(-1, 320, 320, 444)
    } else if ttype == ToolType::Crossbow {
        // dark stock + iron mechanism (distinguishes it from the bow-cell placeholder)
        get4(-1, 100, 210, 444)
    } else if ttype == ToolType::Slingshot {
        // plain whittled wood + cord
        get4(-1, 100, 321, 210)
    } else {
        TOOL_LEVEL_COLORS[level as usize]
    }
}

/// An item icon: 8x8 sheet cell `(x, y)` + a `get4` palette.
fn icon(cell: (i32, i32), colors: i32) -> Sprite {
    Sprite::new1x1(cell.0, cell.1, colors)
}

/// Java `new ToolItem(type, level)`.
pub fn new_tool_item(ttype: ToolType, level: i32) -> Item {
    let name = match ttype.flat_name() {
        Some(flat) => flat.to_string(),
        None => format!("{} {}", TOOL_LEVEL_NAMES[level as usize], ttype.name()),
    };
    Item::new(
        &name,
        icon((ttype.sprite(), 5), tool_color(ttype, level)),
        ItemKind::Tool {
            ttype,
            level,
            dur: ttype.durability() * (level + 1),
        },
    )
}

/// Java `new PotionItem(type)`.
pub fn new_potion_item(ptype: PotionType) -> Item {
    Item::new(
        &ptype.item_name(),
        icon((27, 4), get4(-1, 333, 310, ptype.disp_color())),
        ItemKind::Potion { count: 1, ptype },
    )
}

/// Java `new BucketItem(fill)`.
pub fn new_bucket_item(fill: Fill) -> Item {
    Item::new(
        &format!("{} Bucket", fill.name()),
        icon((21, 4), get4(-1, 222, fill.inner_color(), 555)),
        ItemKind::Bucket {
            count: 1,
            filling: fill,
        },
    )
}

/// Java `new FurnitureItem(furniture)`.
pub fn new_furniture_item(f: Entity) -> Item {
    let fpos = f
        .furniture()
        .expect("furniture item needs furniture")
        .sprite
        .get_pos();
    let fcol = f.furniture().unwrap().sprite.color;
    let x = fpos % 32;
    let y = fpos / 32;
    let sprite_pos = (x / 2) + (y + 2) * 32;
    let name = f.furniture().unwrap().name.clone();
    Item::new(
        &name,
        Sprite::from_pos(sprite_pos, fcol),
        ItemKind::Furniture {
            furniture: Box::new(f),
            placed: false,
        },
    )
}

/// Java `new PowerGloveItem()`.
pub fn new_power_glove() -> Item {
    Item::new(
        "Power Glove",
        icon((7, 4), get4(-1, 100, 320, 430)),
        ItemKind::PowerGlove,
    )
}

/// Java `new UnknownItem(reqName)`.
pub fn new_unknown_item(req_name: &str) -> Item {
    Item::new(
        req_name,
        Sprite::missing_texture(1, 1),
        ItemKind::Unknown { count: 1 },
    )
}

/* ---------- family helpers: one line per item in build_registry ---------- */

fn stackable(name: &str, cell: (i32, i32), colors: i32) -> Item {
    Item::new(name, icon(cell, colors), ItemKind::Stackable { count: 1 })
}

/// Restores `heal` hunger points when eaten.
fn food(name: &str, cell: (i32, i32), colors: i32, heal: i32) -> Item {
    Item::new(
        name,
        icon(cell, colors),
        ItemKind::Food {
            count: 1,
            heal,
            stamina_cost: 5,
        },
    )
}

/// Post-port: first-aid items (Bandage) — restore health directly, not hunger.
fn medical(name: &str, cell: (i32, i32), colors: i32, heal: i32) -> Item {
    Item::new(
        name,
        icon(cell, colors),
        ItemKind::Medical { count: 1, heal },
    )
}

/// All armor shares the chestplate cell (3, 12); only the palette differs.
fn armor(name: &str, colors: i32, armor: f32, level: i32) -> Item {
    Item::new(
        name,
        icon((3, 12), colors),
        ItemKind::Armor {
            count: 1,
            armor,
            level,
            stamina_cost: 9,
        },
    )
}

fn clothing(name: &str, colors: i32, pcol: i32) -> Item {
    Item::new(
        name,
        icon((6, 12), colors),
        ItemKind::Clothing {
            count: 1,
            player_col: pcol,
        },
    )
}

/// Placeable: `model` = tile name to place, `valid_tiles` = names it can be placed on.
fn tile_item(name: &str, cell: (i32, i32), colors: i32, model: &str, valid_tiles: &[&str]) -> Item {
    Item::new(
        name,
        icon(cell, colors),
        ItemKind::TileItem {
            count: 1,
            model: model.to_uppercase(),
            valid_tiles: valid_tiles.iter().map(|t| t.to_uppercase()).collect(),
        },
    )
}

fn book(name: &str, colors: i32, text: Option<&'static str>, has_title_page: bool) -> Item {
    Item::new(
        name,
        icon((14, 4), colors),
        ItemKind::Book {
            book: text,
            has_title_page,
        },
    )
}

/// Builds the full item prototype list, in Java's registration order.
pub fn build_registry(g: &Game) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();

    items.push(new_power_glove());

    // FurnitureItem.getAllInstances()
    {
        let mut rand = g.random.clone();
        let rnd = &mut rand; // spawner constructors draw a spawn interval
        let mut spawner = |m: Entity| new_furniture_item(furniture::spawner::new(m, rnd));
        items.push(spawner(mob::cow::new(g)));
        items.push(spawner(mob::pig::new(g)));
        items.push(spawner(mob::sheep::new(g)));
        items.push(spawner(mob::zombie::new(g, 1)));
        items.push(spawner(mob::snake::new(g, 1)));
        items.push(spawner(mob::knight::new(g, 1)));
        items.push(spawner(mob::marsh_lurker::new(g, 1)));
        items.push(spawner(mob::feral_hound::new(g, 1)));
        items.push(spawner(mob::stone_golem::new(g, 1)));
        items.push(spawner(mob::night_wisp::new(g, 1)));
        items.push(spawner(mob::glow_worm::new(g)));

        items.push(new_furniture_item(furniture::chest::new()));
        for ctype in CrafterType::VALUES {
            items.push(new_furniture_item(furniture::crafter::new(ctype)));
        }
        for ltype in LanternType::VALUES {
            items.push(new_furniture_item(furniture::lantern::new(ltype)));
        }
        items.push(new_furniture_item(furniture::tnt::new()));
        items.push(new_furniture_item(furniture::bed::new()));
    }

    // TorchItem.getAllInstances()
    items.push(Item::new(
        "Torch",
        icon((18, 4), get4(-1, 500, 520, 320)),
        ItemKind::Torch {
            count: 1,
            valid_tiles: [
                "dirt",
                "Wood Planks",
                "Stone Bricks",
                "Obsidian",
                "Wool",
                "grass",
                "sand",
            ]
            .iter()
            .map(|t| t.to_uppercase())
            .collect(),
        },
    ));

    // BucketItem.getAllInstances()
    for fill in Fill::VALUES {
        items.push(new_bucket_item(fill));
    }

    // BookItem.getAllInstances()
    items.push(book("Book", get4(-1, 200, 531, 430), None, false));
    items.push(book(
        "Antidious",
        get4(-1, 100, 300, 500),
        Some(crate::assets::ANTIDOUS_TXT),
        true,
    ));

    // TileItem.getAllInstances()
    items.push(tile_item(
        "Flower",
        (0, 4),
        get4(-1, 10, 444, 330),
        "flower",
        &["grass"],
    ));
    items.push(tile_item(
        "Acorn",
        (3, 4),
        get4(-1, 100, 531, 320),
        "tree Sapling",
        &["grass"],
    ));
    items.push(tile_item(
        "Dirt",
        (2, 4),
        get4(-1, 100, 322, 432),
        "dirt",
        &["hole", "water", "lava"],
    ));
    items.push(tile_item(
        "Plank",
        (1, 4),
        get4(-1, 200, 531, 530),
        "Wood Planks",
        &["hole", "water"],
    ));
    items.push(tile_item(
        "Plank Wall",
        (16, 4),
        get4(-1, 200, 531, 530),
        "Wood Wall",
        &["Wood Planks"],
    ));
    items.push(tile_item(
        "Wood Door",
        (17, 4),
        get4(-1, 200, 531, 530),
        "Wood Door",
        &["Wood Planks"],
    ));
    items.push(tile_item(
        "Stone Brick",
        (1, 4),
        get4(-1, 333, 444, 444),
        "Stone Bricks",
        &["hole", "water", "lava"],
    ));
    items.push(tile_item(
        "Stone Wall",
        (16, 4),
        get4(-1, 100, 333, 444),
        "Stone Wall",
        &["Stone Bricks"],
    ));
    items.push(tile_item(
        "Stone Door",
        (17, 4),
        get4(-1, 111, 333, 444),
        "Stone Door",
        &["Stone Bricks"],
    ));
    items.push(tile_item(
        "Obsidian Brick",
        (1, 4),
        get4(-1, 159, 59, 59),
        "Obsidian",
        &["hole", "water", "lava"],
    ));
    items.push(tile_item(
        "Obsidian Wall",
        (16, 4),
        get4(-1, 159, 59, 59),
        "Obsidian Wall",
        &["Obsidian"],
    ));
    items.push(tile_item(
        "Obsidian Door",
        (17, 4),
        get4(-1, 159, 59, 59),
        "Obsidian Door",
        &["Obsidian"],
    ));
    items.push(tile_item(
        "Wool",
        (2, 4),
        color::WHITE,
        "wool",
        &["hole", "water"],
    ));
    items.push(tile_item(
        "Red Wool",
        (2, 4),
        get4(-1, 100, 300, 500),
        "Wool_RED",
        &["hole", "water"],
    ));
    items.push(tile_item(
        "Blue Wool",
        (2, 4),
        get4(-1, 5, 115, 115),
        "Wool_BLUE",
        &["hole", "water"],
    ));
    items.push(tile_item(
        "Green Wool",
        (2, 4),
        get4(-1, 10, 40, 50),
        "Wool_GREEN",
        &["hole", "water"],
    ));
    items.push(tile_item(
        "Yellow Wool",
        (2, 4),
        get4(-1, 110, 440, 552),
        "Wool_YELLOW",
        &["hole", "water"],
    ));
    items.push(tile_item(
        "Black Wool",
        (2, 4),
        get4(-1, 0, 111, 111),
        "Wool_BLACK",
        &["hole", "water"],
    ));
    items.push(tile_item(
        "Sand",
        (2, 4),
        get4(-1, 110, 440, 550),
        "sand",
        &["dirt"],
    ));
    items.push(tile_item(
        "Cactus",
        (4, 4),
        get4(-1, 10, 40, 50),
        "cactus Sapling",
        &["sand"],
    ));
    items.push(tile_item(
        "Seeds",
        (5, 4),
        get4(-1, 10, 40, 50),
        "wheat",
        &["farmland"],
    ));
    items.push(tile_item(
        "Grass Seeds",
        (5, 4),
        get4(-1, 10, 30, 50),
        "grass",
        &["dirt"],
    ));
    items.push(tile_item(
        "Bone",
        (15, 4),
        get4(-1, 222, 555, 555),
        "tree",
        &["tree Sapling"],
    ));
    items.push(tile_item(
        "Cloud",
        (2, 4),
        get4(-1, 222, 555, 444),
        "cloud",
        &["Infinite Fall"],
    ));
    // Fossicking: mine-ceiling support; while one stands within 3 tiles, breaking
    // rock never triggers a cave-in (level/tile/fossick.rs).
    // TODO(art): dedicated prop icon (two uprights + header beam) - placeholder
    // reuses the wall cell in raw-timber tones.
    items.push(tile_item(
        "Timber Prop",
        (16, 4),
        get4(-1, 100, 310, 431),
        "Timber Prop",
        &["dirt"],
    ));

    // ToolItem.getAllInstances()
    for ttype in ToolType::VALUES {
        if ttype.flat_name().is_some() {
            // single-prototype tools: Fishing Rod, Crossbow, Slingshot
            items.push(new_tool_item(ttype, 0));
            continue;
        }
        // level 0 = the post-port Crude tier, 1..=5 = the Java Wood..Gem tiers.
        for lvl in 0..=5 {
            items.push(new_tool_item(ttype, lvl));
        }
    }

    // FoodItem.getAllInstances()
    items.push(food("Bread", (8, 4), get4(-1, 110, 330, 550), 2));
    items.push(food("Apple", (9, 4), get4(-1, 100, 300, 500), 1));
    items.push(food("Raw Pork", (20, 4), get4(-1, 211, 311, 411), 1));
    items.push(food("Raw Fish", (24, 4), get4(-1, 660, 670, 680), 1));
    items.push(food("Raw Beef", (20, 4), get4(-1, 200, 300, 400), 1));
    items.push(food("Pork Chop", (20, 4), get4(-1, 220, 440, 330), 3));
    items.push(food("Cooked Fish", (24, 4), get4(-1, 220, 330, 440), 3));
    items.push(food("Cooked Pork", (20, 4), get4(-1, 220, 440, 330), 3));
    items.push(food("Steak", (20, 4), get4(-1, 100, 333, 211), 3));
    items.push(food("Gold Apple", (9, 4), get4(-1, 110, 440, 550), 10));
    // Forage foods (world spawning lives with the flora work; these are the item
    // prototypes it drops). Heal values sit on the existing raw=1..2 / cooked=3 scale.
    // TODO(art): final icons — placeholders reuse nearby cells recolored.
    items.push(food("Berry", (5, 4), get4(-1, 102, 203, 415), 1));
    items.push(food("Mushroom", (0, 4), get4(-1, 210, 433, 544), 1));
    items.push(food("Cactus Fruit", (9, 4), get4(-1, 10, 40, 525), 1));
    items.push(food("Coconut", (3, 4), get4(-1, 100, 321, 554), 2));
    items.push(food("Cooked Mushroom", (0, 4), get4(-1, 210, 431, 543), 3));
    // Jack-O-Lantern ingredient; the pumpkin tile needs a drop path (flora work).
    items.push(food("Pumpkin", (2, 4), get4(-1, 210, 530, 550), 2));
    items.push(food("Fruit Medley", (8, 4), get4(-1, 102, 304, 525), 3));

    // Post-port first-aid: heals health (not hunger), hand-crafted from cord + fibers.
    items.push(medical("Bandage", (1, 4), get4(-1, 300, 444, 555), 3));

    // StackableItem.getAllInstances()
    items.push(stackable("Grass Fibers", (21, 5), get4(-1, 10, 40, 50)));
    items.push(stackable(
        "Stick",
        (20, 5),
        get4(-1, color::hex("#b5651d"), 532, 532),
    ));
    // Twisted grass fibers — lashing for tools, bowstrings, fishing line.
    items.push(stackable("Cord", (25, 4), get4(-1, 210, 320, 431)));
    // Knapped from 2 Stone in the personal crafting menu; the crude tool head.
    items.push(stackable("Sharp Stone", (23, 4), get4(-1, 111, 333, 444)));
    // Fossicking: swirl creek mud / exposed tidal flats / wet banks for flecks and
    // nuggets; find odds scale with the land's hidden richness field.
    // TODO(art): dedicated pan icon (shallow dish with a glint) - placeholder reuses
    // the bucket cell in beaten-tin gray.
    items.push(stackable(
        "Prospector's Pan",
        (21, 4),
        get4(-1, 111, 333, 444),
    ));
    // Lets the player cross Deep Water while it's in the inventory (multi-level terrain).
    items.push(stackable("Raft", (28, 4), get4(-1, 210, 431, 321)));
    items.push(stackable("Wood", (28, 4), get4(-1, 310, 532, 532)));
    items.push(stackable("Stone", (2, 4), get4(-1, 111, 333, 555)));
    items.push(stackable("Leather", (19, 4), get4(-1, 100, 211, 322)));
    items.push(stackable("Wheat", (6, 4), get4(-1, 110, 330, 550)));
    items.push(stackable("Key", (26, 4), get4(-1, -1, 444, 550)));
    items.push(stackable("arrow", (13, 5), get4(-1, 111, 222, 430)));
    items.push(stackable("string", (25, 4), color::WHITE));
    items.push(stackable("Coal", (10, 4), get4(-1, 0, 111, 111)));
    items.push(stackable("Iron Ore", (10, 4), get4(-1, 100, 322, 544)));
    items.push(stackable("Lapis", (10, 4), get4(-1, 5, 115, 115)));
    items.push(stackable("Gold Ore", (10, 4), get4(-1, 110, 440, 553)));
    items.push(stackable("Iron", (11, 4), get4(-1, 100, 322, 544)));
    items.push(stackable("Gold", (11, 4), get4(-1, 110, 330, 553)));
    items.push(stackable("Rose", (0, 4), get4(-1, 100, 300, 500)));
    items.push(stackable("GunPowder", (2, 4), get4(-1, 111, 222, 333)));
    items.push(stackable("Slime", (10, 4), get4(-1, 10, 30, 50)));
    items.push(stackable("glass", (12, 4), color::WHITE));
    items.push(stackable("cloth", (1, 4), get4(-1, 25, 252, 141)));
    items.push(stackable("gem", (13, 4), get4(-1, 101, 404, 545)));
    items.push(stackable("Scale", (22, 4), get4(-1, 10, 30, 20)));
    items.push(stackable("Shard", (23, 4), get4(-1, 222, 333, 444)));
    // Thrown weapon: consumed on throw, lands as a pickup where it stops.
    items.push(stackable(
        "Throwing Knife",
        (23, 4),
        get4(-1, 111, 444, 555),
    ));
    // Anvil-forged trigger/gear assembly; the Crossbow's metal half.
    items.push(stackable(
        "Crossbow Mechanism",
        (11, 4),
        get4(-1, 100, 222, 433),
    ));

    // ClothingItem.getAllInstances()
    items.push(clothing("Red Clothes", get4(-1, 100, 400, 500), 400));
    items.push(clothing("Blue Clothes", get4(-1, 1, 4, 5), 4));
    items.push(clothing("Green Clothes", get4(-1, 10, 40, 50), 40));
    items.push(clothing("Yellow Clothes", get4(-1, 110, 440, 550), 440));
    items.push(clothing("Black Clothes", get4(-1, 0, 111, 222), 111));
    items.push(clothing("Orange Clothes", get4(-1, 210, 520, 530), 520));
    items.push(clothing("Purple Clothes", get4(-1, 102, 203, 405), 203));
    items.push(clothing("Cyan Clothes", get4(-1, 12, 23, 45), 23));
    items.push(clothing("Reg Clothes", get4(-1, 111, 110, 210), 110));

    // ArmorItem.getAllInstances()
    items.push(armor("Leather Armor", get4(-1, 100, 211, 322), 0.3, 1));
    items.push(armor("Snake Armor", get4(-1, 10, 20, 30), 0.4, 2));
    items.push(armor("Iron Armor", get4(-1, 100, 322, 544), 0.5, 3));
    items.push(armor("Gold Armor", get4(-1, 110, 330, 553), 0.7, 4));
    items.push(armor("Gem Armor", get4(-1, 101, 404, 545), 1.0, 5));

    // PotionItem.getAllInstances()
    for ptype in PotionType::VALUES {
        items.push(new_potion_item(ptype));
    }

    items
}

/// Java `Items.get(name)` — never null (returns UnknownItem instead).
pub fn get(g: &Game, name: &str) -> Item {
    get_opt(g, name, false).unwrap_or_else(|| new_unknown_item("NULL"))
}

/// Java `Items.get(name, allowNull)`.
pub fn get_opt(g: &Game, name: &str, allow_null: bool) -> Option<Item> {
    let mut name = name.to_uppercase();
    let mut amount = 1;
    let mut had_underscore = false;

    if let Some(idx) = name.find('_') {
        had_underscore = true;
        amount = name[idx + 1..].parse().unwrap_or(1);
        name.truncate(idx);
    } else if let Some(idx) = name.find(';') {
        had_underscore = true;
        amount = name[idx + 1..].parse().unwrap_or(1);
        name.truncate(idx);
    }

    if name == "NULL" {
        if allow_null {
            return None;
        }
        println!(
            "WARNING: Items.get passed argument \"null\" when null is not allowed; returning UnknownItem."
        );
        return Some(new_unknown_item("NULL"));
    }

    if name == "UNKNOWN" {
        return Some(new_unknown_item("BLANK"));
    }

    let found = g
        .items
        .iter()
        .find(|i| i.get_name().eq_ignore_ascii_case(&name));

    match found {
        Some(proto) => {
            let mut item = proto.clone();
            if item.is_stackable() {
                item.set_count(amount);
            }
            if had_underscore {
                if let ItemKind::Tool { dur, .. } = &mut item.kind {
                    *dur = amount;
                }
            }
            Some(item)
        }
        None => {
            println!("ITEMS GET: invalid name requested: \"{name}\"");
            Some(new_unknown_item(&name))
        }
    }
}

/// Java `Items.arrowItem`.
pub fn arrow_item(g: &Game) -> Item {
    get(g, "arrow")
}

/// Java `Items.fillCreativeInv(inv, addAll)`.
pub fn fill_creative_inv(g: &Game, inv: &mut Inventory, add_all: bool) {
    for item in g.items.iter() {
        if matches!(item.kind, ItemKind::PowerGlove) {
            continue;
        }
        if add_all || inv.count(item) == 0 {
            inv.add(item.clone());
        }
    }
}
