//! Port of `fdoom.item.ToolType` — extended post-port with the survival weapons
//! (`Spear`, `Crossbow`, `Slingshot`); those have no Java origin.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolType {
    Shovel,
    Hoe,
    Sword,
    Pickaxe,
    Axe,
    Bow,
    FishingRod,
    Claymore,
    /// Post-port: reach melee weapon; SHIFT-attack throws it (see `player_behavior`).
    Spear,
    /// Post-port: assembled ranged weapon (single tier, like the Fishing Rod).
    Crossbow,
    /// Post-port: early ranged weapon firing Stone pellets (single tier).
    Slingshot,
}

impl ToolType {
    pub const VALUES: [ToolType; 11] = [
        ToolType::Shovel,
        ToolType::Hoe,
        ToolType::Sword,
        ToolType::Pickaxe,
        ToolType::Axe,
        ToolType::Bow,
        ToolType::FishingRod,
        ToolType::Claymore,
        ToolType::Spear,
        ToolType::Crossbow,
        ToolType::Slingshot,
    ];

    /// Sprite location on the spritesheet.
    pub fn sprite(self) -> i32 {
        match self {
            ToolType::Shovel => 0,
            ToolType::Hoe => 1,
            ToolType::Sword => 2,
            ToolType::Pickaxe => 3,
            ToolType::Axe => 4,
            ToolType::Bow => 5,
            ToolType::FishingRod => 6,
            ToolType::Claymore => 7,
            // TODO(art): dedicated icons wanted at cells (8,5), (9,5), (10,5);
            // placeholders reuse the sword and bow cells recolored (see tool_color).
            ToolType::Spear => 2,
            ToolType::Crossbow => 5,
            ToolType::Slingshot => 5,
        }
    }

    pub fn durability(self) -> i32 {
        match self {
            ToolType::Shovel => 24,
            ToolType::Hoe => 20,
            ToolType::Sword => 42,
            ToolType::Pickaxe => 28,
            ToolType::Axe => 24,
            ToolType::Bow => 20,
            ToolType::FishingRod => 16,
            ToolType::Claymore => 34,
            ToolType::Spear => 30,
            ToolType::Crossbow => 40,
            ToolType::Slingshot => 18,
        }
    }

    /// Java `ToolType.name()` / `toString()`.
    pub fn name(self) -> &'static str {
        match self {
            ToolType::Shovel => "Shovel",
            ToolType::Hoe => "Hoe",
            ToolType::Sword => "Sword",
            ToolType::Pickaxe => "Pickaxe",
            ToolType::Axe => "Axe",
            ToolType::Bow => "Bow",
            ToolType::FishingRod => "FishingRod",
            ToolType::Claymore => "Claymore",
            ToolType::Spear => "Spear",
            ToolType::Crossbow => "Crossbow",
            ToolType::Slingshot => "Slingshot",
        }
    }

    /// Single-prototype tools (no `TOOL_LEVEL_NAMES` tier prefix, one registry entry at
    /// level 0). `Some(name)` is the item's full display/registry name.
    pub fn flat_name(self) -> Option<&'static str> {
        match self {
            ToolType::FishingRod => Some("Fishing Rod"),
            ToolType::Crossbow => Some("Crossbow"),
            ToolType::Slingshot => Some("Slingshot"),
            _ => None,
        }
    }
}
