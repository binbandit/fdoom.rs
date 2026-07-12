//! Scavenge containers (towns & scavenge wave): supply crates, barrels, and
//! cupboards scattered through generated towns, camps, and ruins. Unlike a Chest
//! (player storage, browsable), a scavenge container is a one-time *rummage*: the
//! first use spills its seeded finds onto the floor, then it visibly reads emptied
//! (open/dark palette — same state-color scheme as the dungeon chest's lock) and
//! only coughs up dust afterwards.

use crate::entity::{Entity, EntityKind};
use crate::gfx::{Sprite, color};

use super::chest::ChestData;
use super::furniture_common;

/// Which piece of scavenge furniture this is. Keep new variants at the END of
/// `VALUES`: saves store the kind as its `VALUES` ordinal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScavKind {
    Crate,
    Barrel,
    Cupboard,
}

impl ScavKind {
    pub const VALUES: [ScavKind; 3] = [ScavKind::Crate, ScavKind::Barrel, ScavKind::Cupboard];

    pub fn title(self) -> &'static str {
        match self {
            ScavKind::Crate => "Supply Crate",
            ScavKind::Barrel => "Barrel",
            ScavKind::Cupboard => "Cupboard",
        }
    }

    /// Sprite color: sound and shut when unsearched, dark and hollow once rummaged.
    // TODO(art): dedicated cells (closed + open lid) for crate slats, barrel staves
    // and a shelved cupboard; placeholders reuse the chest cells (2,8 2x2) recolored.
    pub fn col(self, searched: bool) -> i32 {
        match (self, searched) {
            // pale slat wood, iron banding
            (ScavKind::Crate, false) => color::get4(-1, 100, 321, 543),
            (ScavKind::Crate, true) => color::get4(-1, 0, 210, 321),
            // dark stave wood, dull hoops
            (ScavKind::Barrel, false) => color::get4(-1, 100, 210, 421),
            (ScavKind::Barrel, true) => color::get4(-1, 0, 110, 210),
            // painted kitchen furniture
            (ScavKind::Cupboard, false) => color::get4(-1, 110, 432, 554),
            (ScavKind::Cupboard, true) => color::get4(-1, 0, 221, 332),
        }
    }

    pub fn sprite(self, searched: bool) -> Sprite {
        Sprite::new(2, 8, 2, 2, self.col(searched), 0)
    }
}

#[derive(Debug, Clone)]
pub struct ScavContainerData {
    /// Furniture + the not-yet-spilled finds (drained on the first rummage).
    pub chest: ChestData,
    pub kind: ScavKind,
    pub searched: bool,
}

/// A fresh, shut container with an empty hold (worldgen fills it before spawning).
pub fn new(kind: ScavKind) -> Entity {
    let mut chest = ChestData::with_name(kind.title(), kind.col(false));
    chest.furniture.sprite = kind.sprite(false);
    let c = furniture_common(chest.furniture.sprite.color, 3, 3);
    Entity::new(
        c,
        EntityKind::ScavContainer(ScavContainerData {
            chest,
            kind,
            searched: false,
        }),
    )
}
