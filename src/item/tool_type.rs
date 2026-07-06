//! Port of `fdoom.item.ToolType`.

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
}

impl ToolType {
    pub const VALUES: [ToolType; 8] = [
        ToolType::Shovel,
        ToolType::Hoe,
        ToolType::Sword,
        ToolType::Pickaxe,
        ToolType::Axe,
        ToolType::Bow,
        ToolType::FishingRod,
        ToolType::Claymore,
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
        }
    }
}
