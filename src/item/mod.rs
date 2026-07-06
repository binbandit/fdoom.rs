//! Port of the `fdoom.item` package.
//!
//! Java's `Item` class hierarchy becomes the `Item` struct (name + sprite) plus the
//! `ItemKind` enum. Item *behaviors* (`interactOn` etc., which need level/player context)
//! live in `interact.rs`; this module holds the data model and the `Items` registry.

pub mod interact;
pub mod inventory;
pub mod potion_type;
pub mod recipe;
pub mod registry;
pub mod tool_type;

pub use inventory::Inventory;
pub use potion_type::PotionType;
pub use recipe::Recipe;
pub use registry::fill_creative_inv;
pub use tool_type::ToolType;

use crate::entity::Entity;
use crate::gfx::{color, Sprite};

/// Java `BucketItem.Fill`. (`contained` tile ids resolve via `fill_contained_tile`.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fill {
    Empty,
    Water,
    Lava,
}

impl Fill {
    pub const VALUES: [Fill; 3] = [Fill::Empty, Fill::Water, Fill::Lava];

    pub fn inner_color(self) -> i32 {
        match self {
            Fill::Empty => 333,
            Fill::Water => 5,
            Fill::Lava => 400,
        }
    }

    /// The tile name Java stored as `contained` (resolved to a Tile at use time).
    pub fn contained_tile(self) -> &'static str {
        match self {
            Fill::Empty => "hole",
            Fill::Water => "water",
            Fill::Lava => "lava",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Fill::Empty => "Empty",
            Fill::Water => "Water",
            Fill::Lava => "Lava",
        }
    }
}

/// Java `Lantern.Type` (lives here to avoid an item→furniture ordering knot; the furniture
/// module re-exports it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanternType {
    Norm,
    Iron,
    Gold,
}

#[derive(Debug, Clone)]
pub enum ItemKind {
    /// Java `PowerGloveItem`.
    PowerGlove,
    /// Plain `StackableItem` (wood, stone, coal, ...).
    Stackable { count: i32 },
    /// Java `UnknownItem` (a StackableItem subclass).
    Unknown { count: i32 },
    Food {
        count: i32,
        heal: i32,
        stamina_cost: i32,
    },
    Armor {
        count: i32,
        armor: f32,
        level: i32,
        stamina_cost: i32,
    },
    Clothing {
        count: i32,
        player_col: i32,
    },
    Potion {
        count: i32,
        ptype: PotionType,
    },
    /// Java `TileItem`.
    TileItem {
        count: i32,
        model: String,
        valid_tiles: Vec<String>,
    },
    /// Java `TorchItem` (a TileItem subclass with empty model).
    Torch {
        count: i32,
        valid_tiles: Vec<String>,
    },
    Bucket {
        count: i32,
        filling: Fill,
    },
    Tool {
        ttype: ToolType,
        level: i32,
        dur: i32,
    },
    /// Java `FurnitureItem` — holds a whole furniture entity, exactly like Java.
    Furniture {
        furniture: Box<Entity>,
        placed: bool,
    },
    Book {
        /// None = blank book; Some = static book text key.
        book: Option<&'static str>,
        has_title_page: bool,
    },
}

#[derive(Debug, Clone)]
pub struct Item {
    name: String,
    pub sprite: Sprite,
    pub kind: ItemKind,
}

impl Item {
    pub fn new(name: &str, sprite: Sprite, kind: ItemKind) -> Item {
        Item { name: name.to_string(), sprite, kind }
    }

    /// Java `getName()`.
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Java `instanceof StackableItem`.
    pub fn is_stackable(&self) -> bool {
        self.count_ref().is_some()
    }

    fn count_ref(&self) -> Option<&i32> {
        Some(match &self.kind {
            ItemKind::Stackable { count }
            | ItemKind::Unknown { count }
            | ItemKind::Food { count, .. }
            | ItemKind::Armor { count, .. }
            | ItemKind::Clothing { count, .. }
            | ItemKind::Potion { count, .. }
            | ItemKind::TileItem { count, .. }
            | ItemKind::Torch { count, .. }
            | ItemKind::Bucket { count, .. } => count,
            _ => return None,
        })
    }

    pub fn count_mut(&mut self) -> Option<&mut i32> {
        Some(match &mut self.kind {
            ItemKind::Stackable { count }
            | ItemKind::Unknown { count }
            | ItemKind::Food { count, .. }
            | ItemKind::Armor { count, .. }
            | ItemKind::Clothing { count, .. }
            | ItemKind::Potion { count, .. }
            | ItemKind::TileItem { count, .. }
            | ItemKind::Torch { count, .. }
            | ItemKind::Bucket { count, .. } => count,
            _ => return None,
        })
    }

    /// The stack count (1 for non-stackables, mirroring how Java code uses casts).
    pub fn count(&self) -> i32 {
        self.count_ref().copied().unwrap_or(1)
    }

    pub fn set_count(&mut self, n: i32) {
        if let Some(c) = self.count_mut() {
            *c = n;
        }
    }

    /// Java `StackableItem.stacksWith(other)` — other is stackable and names match
    /// (JAVA: the class is deliberately not compared).
    pub fn stacks_with(&self, other: &Item) -> bool {
        other.is_stackable() && other.name == self.name
    }

    /// Java `Item.equals(Item)` and its overrides.
    pub fn item_equals(&self, other: &Item) -> bool {
        match (&self.kind, &other.kind) {
            // ToolItem: type and level only
            (ItemKind::Tool { ttype: t1, level: l1, .. }, ItemKind::Tool { ttype: t2, level: l2, .. }) => {
                t1 == t2 && l1 == l2
            }
            // TorchItem: any torch
            (ItemKind::Torch { .. }, ItemKind::Torch { .. }) => true,
            // TileItem: class + name + model
            (ItemKind::TileItem { model: m1, .. }, ItemKind::TileItem { model: m2, .. }) => {
                self.name == other.name && m1 == m2
            }
            // PotionItem: class + name + type
            (ItemKind::Potion { ptype: p1, .. }, ItemKind::Potion { ptype: p2, .. }) => {
                self.name == other.name && p1 == p2
            }
            // BucketItem: class + name + filling
            (ItemKind::Bucket { filling: f1, .. }, ItemKind::Bucket { filling: f2, .. }) => {
                self.name == other.name && f1 == f2
            }
            // default: same class, same name
            (a, b) => std::mem::discriminant(a) == std::mem::discriminant(b) && self.name == other.name,
        }
    }

    /// Java `isDepleted()`.
    pub fn is_depleted(&self) -> bool {
        match &self.kind {
            ItemKind::Tool { dur, ttype, .. } => *dur <= 0 && ttype.durability() > 0,
            ItemKind::Furniture { placed, .. } => *placed,
            _ => match self.count_ref() {
                Some(count) => *count <= 0,
                None => false,
            },
        }
    }

    /// Java `canAttack()`.
    pub fn can_attack(&self) -> bool {
        matches!(self.kind, ItemKind::Tool { .. })
    }

    /// Java `interactsWithWorld()`.
    pub fn interacts_with_world(&self) -> bool {
        !matches!(
            self.kind,
            ItemKind::Food { .. }
                | ItemKind::Armor { .. }
                | ItemKind::Clothing { .. }
                | ItemKind::Potion { .. }
                | ItemKind::Book { .. }
        )
    }

    /// Java `getData()` — save/network string; `Items.get` re-parses it.
    pub fn get_data(&self) -> String {
        match &self.kind {
            ItemKind::Tool { dur, .. } => format!("{}_{}", self.name, dur),
            _ => match self.count_ref() {
                Some(count) => format!("{}_{}", self.name, count),
                None => self.name.clone(),
            },
        }
    }

    /// Java `getDisplayName()`.
    pub fn get_display_name(&self, g: &crate::core::game::Game) -> String {
        match &self.kind {
            ItemKind::Tool { ttype, level, .. } => {
                if *ttype == ToolType::FishingRod {
                    format!(" {}", g.localization.get_localized("Fishing Rod"))
                } else {
                    format!(
                        " {} {}",
                        g.localization.get_localized(registry::TOOL_LEVEL_NAMES[*level as usize]),
                        g.localization.get_localized(ttype.name())
                    )
                }
            }
            _ => match self.count_ref() {
                Some(count) => {
                    let amt = (*count).min(999);
                    format!(" {} {}", amt, g.localization.get_localized(&self.name))
                }
                None => format!(" {}", g.localization.get_localized(&self.name)),
            },
        }
    }

    /// Java `renderInventory(screen, x, y, ininv)`.
    pub fn render_inventory(
        &self,
        screen: &mut crate::gfx::Screen,
        g: &crate::core::game::Game,
        x: i32,
        y: i32,
        ininv: bool,
    ) {
        let disp_name = self.get_display_name(g);
        self.sprite.render(screen, x, y);
        if ininv {
            let shortname: String = disp_name.chars().take(20).collect();
            crate::gfx::font::draw(&shortname, screen, x + 8, y, color::WHITE);
        } else {
            crate::gfx::font::draw(&disp_name, screen, x + 8, y, color::get(-1, 555));
        }
    }
}

impl std::fmt::Display for Item {
    /// Java `toString()`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.count_ref() {
            Some(count) => write!(f, "{}-Item-Stack_Size:{}", self.name, count),
            None => write!(f, "{}-Item", self.name),
        }
    }
}
