//! Port of `fdoom.item.Items` and each item class's `getAllInstances()` prototype lists.
//!
//! Java built the list once in a static block; here `build_registry` runs once per game
//! (from `Game::new`) since the furniture prototypes read the difficulty setting, exactly
//! as Java's static init read `Settings` at class-load time.

use crate::core::game::Game;
use crate::entity::furniture::crafter::CrafterType;
use crate::entity::furniture::lantern::LanternType;
use crate::entity::{furniture, mob};
use crate::gfx::{Sprite, color};
use crate::item::{Fill, Inventory, Item, ItemKind, PotionType, ToolType};

/// Java `ToolItem.LEVEL_NAMES`.
pub const TOOL_LEVEL_NAMES: [&str; 5] = ["Wood", "Rock", "Iron", "Gold", "Gem"];

/// Java `ToolItem.LEVEL_COLORS`.
pub const TOOL_LEVEL_COLORS: [i32; 5] = [
    color::get4(-1, 100, 321, 431), // wood
    color::get4(-1, 100, 321, 111), // rock/stone
    color::get4(-1, 100, 321, 555), // iron
    color::get4(-1, 100, 321, 550), // gold
    color::get4(-1, 100, 321, 55),  // gem
];

/// Java `ToolItem.BOW_COLORS`.
pub const TOOL_BOW_COLORS: [i32; 5] = [
    color::get4(-1, 100, 444, 431),
    color::get4(-1, 100, 444, 111),
    color::get4(-1, 100, 444, 555),
    color::get4(-1, 100, 444, 550),
    color::get4(-1, 100, 444, 55),
];

fn tool_color(ttype: ToolType, level: i32) -> i32 {
    if ttype == ToolType::Bow {
        TOOL_BOW_COLORS[level as usize]
    } else if ttype == ToolType::FishingRod {
        color::get4(-1, 320, 320, 444)
    } else {
        TOOL_LEVEL_COLORS[level as usize]
    }
}

/// Java `new ToolItem(type, level)`.
pub fn new_tool_item(ttype: ToolType, level: i32) -> Item {
    let name = if ttype == ToolType::FishingRod {
        "Fishing Rod".to_string()
    } else {
        format!("{} {}", TOOL_LEVEL_NAMES[level as usize], ttype.name())
    };
    Item::new(
        &name,
        Sprite::new1x1(ttype.sprite(), 5, tool_color(ttype, level)),
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
        Sprite::new1x1(27, 4, color::get4(-1, 333, 310, ptype.disp_color())),
        ItemKind::Potion { count: 1, ptype },
    )
}

/// Java `new BucketItem(fill)`.
pub fn new_bucket_item(fill: Fill) -> Item {
    Item::new(
        &format!("{} Bucket", fill.name()),
        Sprite::new1x1(21, 4, color::get4(-1, 222, fill.inner_color(), 555)),
        ItemKind::Bucket {
            count: 1,
            filling: fill,
        },
    )
}

/// Java `new FurnitureItem(furniture)`.
pub fn new_furniture_item(f: crate::entity::Entity) -> Item {
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
        Sprite::new1x1(7, 4, color::get4(-1, 100, 320, 430)),
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

fn stackable(name: &str, sprite: Sprite) -> Item {
    Item::new(name, sprite, ItemKind::Stackable { count: 1 })
}

fn food(name: &str, sprite: Sprite, heal: i32) -> Item {
    Item::new(
        name,
        sprite,
        ItemKind::Food {
            count: 1,
            heal,
            stamina_cost: 5,
        },
    )
}

fn armor(name: &str, sprite: Sprite, armor: f32, level: i32) -> Item {
    Item::new(
        name,
        sprite,
        ItemKind::Armor {
            count: 1,
            armor,
            level,
            stamina_cost: 9,
        },
    )
}

fn clothing(name: &str, color: i32, pcol: i32) -> Item {
    Item::new(
        name,
        Sprite::new1x1(6, 12, color),
        ItemKind::Clothing {
            count: 1,
            player_col: pcol,
        },
    )
}

fn tile_item(name: &str, sprite: Sprite, model: &str, valid_tiles: &[&str]) -> Item {
    Item::new(
        name,
        sprite,
        ItemKind::TileItem {
            count: 1,
            model: model.to_uppercase(),
            valid_tiles: valid_tiles.iter().map(|t| t.to_uppercase()).collect(),
        },
    )
}

/// Builds the full item prototype list, in Java's registration order.
pub fn build_registry(g: &Game) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();

    items.push(new_power_glove());

    // FurnitureItem.getAllInstances()
    {
        let mut r = g.random.clone();
        let rnd = &mut r; // spawner constructors draw a spawn interval
        items.push(new_furniture_item(furniture::spawner::new(
            mob::cow::new(g),
            rnd,
        )));
        items.push(new_furniture_item(furniture::spawner::new(
            mob::pig::new(g),
            rnd,
        )));
        items.push(new_furniture_item(furniture::spawner::new(
            mob::sheep::new(g),
            rnd,
        )));
        items.push(new_furniture_item(furniture::spawner::new(
            mob::slime::new(g, 1),
            rnd,
        )));
        items.push(new_furniture_item(furniture::spawner::new(
            mob::zombie::new(g, 1),
            rnd,
        )));
        items.push(new_furniture_item(furniture::spawner::new(
            mob::creeper::new(g, 1),
            rnd,
        )));
        items.push(new_furniture_item(furniture::spawner::new(
            mob::skeleton::new(g, 1),
            rnd,
        )));
        items.push(new_furniture_item(furniture::spawner::new(
            mob::snake::new(g, 1),
            rnd,
        )));
        items.push(new_furniture_item(furniture::spawner::new(
            mob::knight::new(g, 1),
            rnd,
        )));
        items.push(new_furniture_item(furniture::spawner::new(
            mob::air_wizard::new(g, false),
            rnd,
        )));
        items.push(new_furniture_item(furniture::spawner::new(
            mob::glow_worm::new(g),
            rnd,
        )));

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
        Sprite::new1x1(18, 4, color::get4(-1, 500, 520, 320)),
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
    items.push(Item::new(
        "Book",
        Sprite::new1x1(14, 4, color::get4(-1, 200, 531, 430)),
        ItemKind::Book {
            book: None,
            has_title_page: false,
        },
    ));
    items.push(Item::new(
        "Antidious",
        Sprite::new1x1(14, 4, color::get4(-1, 100, 300, 500)),
        ItemKind::Book {
            book: Some(crate::assets::ANTIDOUS_TXT),
            has_title_page: true,
        },
    ));

    // TileItem.getAllInstances()
    items.push(tile_item(
        "Flower",
        Sprite::new1x1(0, 4, color::get4(-1, 10, 444, 330)),
        "flower",
        &["grass"],
    ));
    items.push(tile_item(
        "Acorn",
        Sprite::new1x1(3, 4, color::get4(-1, 100, 531, 320)),
        "tree Sapling",
        &["grass"],
    ));
    items.push(tile_item(
        "Dirt",
        Sprite::new1x1(2, 4, color::get4(-1, 100, 322, 432)),
        "dirt",
        &["hole", "water", "lava"],
    ));
    items.push(tile_item(
        "Plank",
        Sprite::new1x1(1, 4, color::get4(-1, 200, 531, 530)),
        "Wood Planks",
        &["hole", "water"],
    ));
    items.push(tile_item(
        "Plank Wall",
        Sprite::new1x1(16, 4, color::get4(-1, 200, 531, 530)),
        "Wood Wall",
        &["Wood Planks"],
    ));
    items.push(tile_item(
        "Wood Door",
        Sprite::new1x1(17, 4, color::get4(-1, 200, 531, 530)),
        "Wood Door",
        &["Wood Planks"],
    ));
    items.push(tile_item(
        "Stone Brick",
        Sprite::new1x1(1, 4, color::get4(-1, 333, 444, 444)),
        "Stone Bricks",
        &["hole", "water", "lava"],
    ));
    items.push(tile_item(
        "Stone Wall",
        Sprite::new1x1(16, 4, color::get4(-1, 100, 333, 444)),
        "Stone Wall",
        &["Stone Bricks"],
    ));
    items.push(tile_item(
        "Stone Door",
        Sprite::new1x1(17, 4, color::get4(-1, 111, 333, 444)),
        "Stone Door",
        &["Stone Bricks"],
    ));
    items.push(tile_item(
        "Obsidian Brick",
        Sprite::new1x1(1, 4, color::get4(-1, 159, 59, 59)),
        "Obsidian",
        &["hole", "water", "lava"],
    ));
    items.push(tile_item(
        "Obsidian Wall",
        Sprite::new1x1(16, 4, color::get4(-1, 159, 59, 59)),
        "Obsidian Wall",
        &["Obsidian"],
    ));
    items.push(tile_item(
        "Obsidian Door",
        Sprite::new1x1(17, 4, color::get4(-1, 159, 59, 59)),
        "Obsidian Door",
        &["Obsidian"],
    ));
    items.push(tile_item(
        "Wool",
        Sprite::new1x1(2, 4, color::WHITE),
        "wool",
        &["hole", "water"],
    ));
    items.push(tile_item(
        "Red Wool",
        Sprite::new1x1(2, 4, color::get4(-1, 100, 300, 500)),
        "Wool_RED",
        &["hole", "water"],
    ));
    items.push(tile_item(
        "Blue Wool",
        Sprite::new1x1(2, 4, color::get4(-1, 5, 115, 115)),
        "Wool_BLUE",
        &["hole", "water"],
    ));
    items.push(tile_item(
        "Green Wool",
        Sprite::new1x1(2, 4, color::get4(-1, 10, 40, 50)),
        "Wool_GREEN",
        &["hole", "water"],
    ));
    items.push(tile_item(
        "Yellow Wool",
        Sprite::new1x1(2, 4, color::get4(-1, 110, 440, 552)),
        "Wool_YELLOW",
        &["hole", "water"],
    ));
    items.push(tile_item(
        "Black Wool",
        Sprite::new1x1(2, 4, color::get4(-1, 0, 111, 111)),
        "Wool_BLACK",
        &["hole", "water"],
    ));
    items.push(tile_item(
        "Sand",
        Sprite::new1x1(2, 4, color::get4(-1, 110, 440, 550)),
        "sand",
        &["dirt"],
    ));
    items.push(tile_item(
        "Cactus",
        Sprite::new1x1(4, 4, color::get4(-1, 10, 40, 50)),
        "cactus Sapling",
        &["sand"],
    ));
    items.push(tile_item(
        "Seeds",
        Sprite::new1x1(5, 4, color::get4(-1, 10, 40, 50)),
        "wheat",
        &["farmland"],
    ));
    items.push(tile_item(
        "Grass Seeds",
        Sprite::new1x1(5, 4, color::get4(-1, 10, 30, 50)),
        "grass",
        &["dirt"],
    ));
    items.push(tile_item(
        "Bone",
        Sprite::new1x1(15, 4, color::get4(-1, 222, 555, 555)),
        "tree",
        &["tree Sapling"],
    ));
    items.push(tile_item(
        "Cloud",
        Sprite::new1x1(2, 4, color::get4(-1, 222, 555, 444)),
        "cloud",
        &["Infinite Fall"],
    ));

    // ToolItem.getAllInstances()
    items.push(new_tool_item(ToolType::FishingRod, 0));
    for ttype in ToolType::VALUES {
        if ttype == ToolType::FishingRod {
            continue;
        }
        for lvl in 0..=4 {
            items.push(new_tool_item(ttype, lvl));
        }
    }

    // FoodItem.getAllInstances()
    items.push(food(
        "Bread",
        Sprite::new1x1(8, 4, color::get4(-1, 110, 330, 550)),
        2,
    ));
    items.push(food(
        "Apple",
        Sprite::new1x1(9, 4, color::get4(-1, 100, 300, 500)),
        1,
    ));
    items.push(food(
        "Raw Pork",
        Sprite::new1x1(20, 4, color::get4(-1, 211, 311, 411)),
        1,
    ));
    items.push(food(
        "Raw Fish",
        Sprite::new1x1(24, 4, color::get4(-1, 660, 670, 680)),
        1,
    ));
    items.push(food(
        "Raw Beef",
        Sprite::new1x1(20, 4, color::get4(-1, 200, 300, 400)),
        1,
    ));
    items.push(food(
        "Pork Chop",
        Sprite::new1x1(20, 4, color::get4(-1, 220, 440, 330)),
        3,
    ));
    items.push(food(
        "Cooked Fish",
        Sprite::new1x1(24, 4, color::get4(-1, 220, 330, 440)),
        3,
    ));
    items.push(food(
        "Cooked Pork",
        Sprite::new1x1(20, 4, color::get4(-1, 220, 440, 330)),
        3,
    ));
    items.push(food(
        "Steak",
        Sprite::new1x1(20, 4, color::get4(-1, 100, 333, 211)),
        3,
    ));
    items.push(food(
        "Gold Apple",
        Sprite::new1x1(9, 4, color::get4(-1, 110, 440, 550)),
        10,
    ));

    // StackableItem.getAllInstances()
    items.push(stackable(
        "Grass Fibers",
        Sprite::new1x1(21, 5, color::get4(-1, 10, 40, 50)),
    ));
    items.push(stackable(
        "Stick",
        Sprite::new1x1(20, 5, color::get4(-1, color::hex("#b5651d"), 532, 532)),
    ));
    items.push(stackable(
        "Wood",
        Sprite::new1x1(28, 4, color::get4(-1, 310, 532, 532)),
    ));
    items.push(stackable(
        "Stone",
        Sprite::new1x1(2, 4, color::get4(-1, 111, 333, 555)),
    ));
    items.push(stackable(
        "Leather",
        Sprite::new1x1(19, 4, color::get4(-1, 100, 211, 322)),
    ));
    items.push(stackable(
        "Wheat",
        Sprite::new1x1(6, 4, color::get4(-1, 110, 330, 550)),
    ));
    items.push(stackable(
        "Key",
        Sprite::new1x1(26, 4, color::get4(-1, -1, 444, 550)),
    ));
    items.push(stackable(
        "arrow",
        Sprite::new1x1(13, 5, color::get4(-1, 111, 222, 430)),
    ));
    items.push(stackable("string", Sprite::new1x1(25, 4, color::WHITE)));
    items.push(stackable(
        "Coal",
        Sprite::new1x1(10, 4, color::get4(-1, 0, 111, 111)),
    ));
    items.push(stackable(
        "Iron Ore",
        Sprite::new1x1(10, 4, color::get4(-1, 100, 322, 544)),
    ));
    items.push(stackable(
        "Lapis",
        Sprite::new1x1(10, 4, color::get4(-1, 5, 115, 115)),
    ));
    items.push(stackable(
        "Gold Ore",
        Sprite::new1x1(10, 4, color::get4(-1, 110, 440, 553)),
    ));
    items.push(stackable(
        "Iron",
        Sprite::new1x1(11, 4, color::get4(-1, 100, 322, 544)),
    ));
    items.push(stackable(
        "Gold",
        Sprite::new1x1(11, 4, color::get4(-1, 110, 330, 553)),
    ));
    items.push(stackable(
        "Rose",
        Sprite::new1x1(0, 4, color::get4(-1, 100, 300, 500)),
    ));
    items.push(stackable(
        "GunPowder",
        Sprite::new1x1(2, 4, color::get4(-1, 111, 222, 333)),
    ));
    items.push(stackable(
        "Slime",
        Sprite::new1x1(10, 4, color::get4(-1, 10, 30, 50)),
    ));
    items.push(stackable("glass", Sprite::new1x1(12, 4, color::WHITE)));
    items.push(stackable(
        "cloth",
        Sprite::new1x1(1, 4, color::get4(-1, 25, 252, 141)),
    ));
    items.push(stackable(
        "gem",
        Sprite::new1x1(13, 4, color::get4(-1, 101, 404, 545)),
    ));
    items.push(stackable(
        "Scale",
        Sprite::new1x1(22, 4, color::get4(-1, 10, 30, 20)),
    ));
    items.push(stackable(
        "Shard",
        Sprite::new1x1(23, 4, color::get4(-1, 222, 333, 444)),
    ));

    // ClothingItem.getAllInstances()
    items.push(clothing("Red Clothes", color::get4(-1, 100, 400, 500), 400));
    items.push(clothing("Blue Clothes", color::get4(-1, 1, 4, 5), 4));
    items.push(clothing("Green Clothes", color::get4(-1, 10, 40, 50), 40));
    items.push(clothing(
        "Yellow Clothes",
        color::get4(-1, 110, 440, 550),
        440,
    ));
    items.push(clothing("Black Clothes", color::get4(-1, 0, 111, 222), 111));
    items.push(clothing(
        "Orange Clothes",
        color::get4(-1, 210, 520, 530),
        520,
    ));
    items.push(clothing(
        "Purple Clothes",
        color::get4(-1, 102, 203, 405),
        203,
    ));
    items.push(clothing("Cyan Clothes", color::get4(-1, 12, 23, 45), 23));
    items.push(clothing("Reg Clothes", color::get4(-1, 111, 110, 210), 110));

    // ArmorItem.getAllInstances()
    items.push(armor(
        "Leather Armor",
        Sprite::new1x1(3, 12, color::get4(-1, 100, 211, 322)),
        0.3,
        1,
    ));
    items.push(armor(
        "Snake Armor",
        Sprite::new1x1(3, 12, color::get4(-1, 10, 20, 30)),
        0.4,
        2,
    ));
    items.push(armor(
        "Iron Armor",
        Sprite::new1x1(3, 12, color::get4(-1, 100, 322, 544)),
        0.5,
        3,
    ));
    items.push(armor(
        "Gold Armor",
        Sprite::new1x1(3, 12, color::get4(-1, 110, 330, 553)),
        0.7,
        4,
    ));
    items.push(armor(
        "Gem Armor",
        Sprite::new1x1(3, 12, color::get4(-1, 101, 404, 545)),
        1.0,
        5,
    ));

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
