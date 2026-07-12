//! Port of `fdoom.item.PotionType`. The per-type `toggleEffect` behavior is implemented
//! in `potion_item::toggle_effect` (it needs game context).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PotionType {
    None,
    Speed,
    Light,
    Swim,
    Energy,
    Regen,
    Health,
    Time,
    Lava,
    Shield,
    Haste,
    /// Post-port (farming/cooking wave): food-borne nausea, not a brewable potion —
    /// eating raw flesh (or a raw potato) risks it. Slows stamina recovery while it
    /// lasts. Rides the potion-effect machinery for its timer/HUD/save handling but
    /// is excluded from the registry's potion items (you can't bottle it).
    Queasy,
}

impl PotionType {
    pub const VALUES: [PotionType; 12] = [
        PotionType::None,
        PotionType::Speed,
        PotionType::Light,
        PotionType::Swim,
        PotionType::Energy,
        PotionType::Regen,
        PotionType::Health,
        PotionType::Time,
        PotionType::Lava,
        PotionType::Shield,
        PotionType::Haste,
        PotionType::Queasy,
    ];

    pub fn disp_color(self) -> i32 {
        match self {
            PotionType::None => 5,
            PotionType::Speed => 10,
            PotionType::Light => 440,
            PotionType::Swim => 3,
            PotionType::Energy => 510,
            PotionType::Regen => 504,
            PotionType::Health => 501,
            PotionType::Time => 222,
            PotionType::Lava => 400,
            PotionType::Shield => 115,
            PotionType::Haste => 303,
            PotionType::Queasy => 230, // bilious green
        }
    }

    pub fn duration(self) -> i32 {
        match self {
            PotionType::None => 0,
            PotionType::Speed => 4200,
            PotionType::Light => 6000,
            PotionType::Swim => 4800,
            PotionType::Energy => 8400,
            PotionType::Regen => 1800,
            PotionType::Health => 0,
            PotionType::Time => 1800,
            PotionType::Lava => 7200,
            PotionType::Shield => 5400,
            PotionType::Haste => 4800,
            PotionType::Queasy => 3600, // one in-game minute of a turned stomach
        }
    }

    /// The Java enum constant name ("Speed", ...).
    pub fn enum_name(self) -> &'static str {
        match self {
            PotionType::None => "None",
            PotionType::Speed => "Speed",
            PotionType::Light => "Light",
            PotionType::Swim => "Swim",
            PotionType::Energy => "Energy",
            PotionType::Regen => "Regen",
            PotionType::Health => "Health",
            PotionType::Time => "Time",
            PotionType::Lava => "Lava",
            PotionType::Shield => "Shield",
            PotionType::Haste => "Haste",
            PotionType::Queasy => "Queasy",
        }
    }

    /// Java `PotionType.name` field — "Potion" for None, "<Type> Potion" otherwise.
    pub fn item_name(self) -> String {
        if self == PotionType::None {
            "Potion".to_string()
        } else {
            format!("{} Potion", self.enum_name())
        }
    }

    pub fn ordinal(self) -> i32 {
        self as i32
    }
}

impl std::fmt::Display for PotionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.enum_name())
    }
}
