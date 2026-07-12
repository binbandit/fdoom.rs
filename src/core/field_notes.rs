//! Field Notes — the survivor's journal. A sandbox with no win condition still
//! deserves soft goals: days survived, country seen, places found, rare nights
//! witnessed, the deepest dig, and a few tallies. The tone is a field journal,
//! not an achievement system — the only feedback is one quiet cue line the first
//! time each biome or place goes into the notes.
//!
//! State lives on [`PlayerData`](crate::entity::mob::player::PlayerData) (`notes`)
//! and rides the player save behind a tolerant trailing marker
//! ([`NOTES_MARKER`](crate::saveload::save::NOTES_MARKER)); old saves default to a
//! blank journal. The bitmask bit order is the enum declaration order — append new
//! biomes/places/events at the end of their enums, never reorder.

use crate::core::events;
use crate::core::game::Game;
use crate::level::infinite_gen::{self, Biome};
use crate::level::structures_gen::{self, StructureKind};

/// Every biome, declaration order (= journal bit order).
pub const ALL_BIOMES: [Biome; 11] = [
    Biome::Ocean,
    Biome::DeepOcean,
    Biome::Beach,
    Biome::Mountains,
    Biome::Tundra,
    Biome::Desert,
    Biome::Badlands,
    Biome::Marsh,
    Biome::Forest,
    Biome::Savanna,
    Biome::Plains,
];

/// How many biomes/places/events the journal tracks (the "x/N" denominators).
pub const BIOME_COUNT: u32 = 11;
pub const PLACE_COUNT: u32 = 6;
pub const EVENT_COUNT: u32 = 5;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldNotes {
    /// Day wraps survived (persistent; `events.day_number` restarts each session).
    pub days_survived: i32,
    /// Bit per [`Biome`] stood in, declaration order.
    pub biomes_seen: u16,
    /// Bit per [`StructureKind`] whose grounds were walked, declaration order.
    pub places_found: u8,
    /// Bit per [`WorldEvent`](events::WorldEvent) seen happening, declaration order.
    pub events_witnessed: u8,
    /// Deepest level depth stood on (0 = surface, mines negative).
    pub deepest_depth: i32,
    pub trees_felled: i32,
    pub fish_caught: i32,
    pub ore_panned: i32,
}

impl FieldNotes {
    /// Record standing in `b`; true the first time.
    pub fn see_biome(&mut self, b: Biome) -> bool {
        let bit = 1u16 << (b as u16);
        let new = self.biomes_seen & bit == 0;
        self.biomes_seen |= bit;
        new
    }

    /// Record walking `k`'s grounds; true the first time.
    pub fn find_place(&mut self, k: StructureKind) -> bool {
        let bit = 1u8 << (k as u8);
        let new = self.places_found & bit == 0;
        self.places_found |= bit;
        new
    }

    /// Record seeing `ev` happen; true the first time.
    pub fn witness_event(&mut self, ev: events::WorldEvent) -> bool {
        let bit = 1u8 << (ev as u8);
        let new = self.events_witnessed & bit == 0;
        self.events_witnessed |= bit;
        new
    }

    pub fn biomes_seen_count(&self) -> u32 {
        self.biomes_seen.count_ones()
    }

    pub fn places_found_count(&self) -> u32 {
        self.places_found.count_ones()
    }

    pub fn events_witnessed_count(&self) -> u32 {
        self.events_witnessed.count_ones()
    }

    /// Save payload behind `Notes:v1:` — the eight fields, `;`-separated.
    pub fn encode(&self) -> String {
        format!(
            "{};{};{};{};{};{};{};{}",
            self.days_survived,
            self.biomes_seen,
            self.places_found,
            self.events_witnessed,
            self.deepest_depth,
            self.trees_felled,
            self.fish_caught,
            self.ore_panned,
        )
    }

    /// Tolerant inverse of [`encode`](Self::encode): missing or malformed fields
    /// read as 0 (a shorter/older payload never fails to load).
    pub fn decode(payload: &str) -> FieldNotes {
        let f: Vec<i64> = payload
            .split(';')
            .map(|s| s.trim().parse::<i64>().unwrap_or(0))
            .collect();
        let at = |i: usize| f.get(i).copied().unwrap_or(0);
        FieldNotes {
            days_survived: at(0) as i32,
            biomes_seen: at(1) as u16,
            places_found: at(2) as u8,
            events_witnessed: at(3) as u8,
            deepest_depth: at(4) as i32,
            trees_felled: at(5) as i32,
            fish_caught: at(6) as i32,
            ore_panned: at(7) as i32,
        }
    }
}

/// Journal name of a biome, for the first-sighting cue and the NOTES pane.
pub fn biome_title(b: Biome) -> &'static str {
    match b {
        Biome::Ocean => "THE OCEAN",
        Biome::DeepOcean => "THE DEEP OCEAN",
        Biome::Beach => "THE BEACH",
        Biome::Mountains => "THE MOUNTAINS",
        Biome::Tundra => "THE TUNDRA",
        Biome::Desert => "THE DESERT",
        Biome::Badlands => "THE BADLANDS",
        Biome::Marsh => "THE MARSH",
        Biome::Forest => "THE FOREST",
        Biome::Savanna => "THE SAVANNA",
        Biome::Plains => "THE PLAINS",
    }
}

/// Journal name of a structure kind, for the first-find cue.
pub fn place_title(k: StructureKind) -> &'static str {
    match k {
        StructureKind::Ruins => "RUINS",
        StructureKind::Cemetery => "A CEMETERY",
        StructureKind::StandingStones => "STANDING STONES",
        StructureKind::Camp => "AN OLD CAMP",
        StructureKind::Hamlet => "A HAMLET",
        StructureKind::Village => "A VILLAGE",
    }
}

/// The world event happening in front of the player right now, if any.
fn event_happening(g: &Game) -> Option<events::WorldEvent> {
    if events::hollow_night_active(g) {
        Some(events::WorldEvent::HollowNight)
    } else if events::aurora_active(g) {
        Some(events::WorldEvent::Aurora)
    } else if events::ember_rain_active(g) {
        Some(events::WorldEvent::EmberRain)
    } else if events::whisper_fog_active(g) {
        Some(events::WorldEvent::WhisperFog)
    } else if events::caravan_active(g) {
        Some(events::WorldEvent::Caravan)
    } else {
        None
    }
}

/// Once-a-second journal sweep (called from `Game::tick`): the biome underfoot,
/// any structure grounds the player is standing in, a world event happening
/// overhead (surface only — you can't witness an aurora from a mine), the depth
/// record, and the pending tree tally. Deliberately not per-tick: a biome lookup
/// plus one small placement query per second is noise-level work.
pub fn tick(g: &mut Game) {
    if g.paused || g.game_time % crate::core::updater::NORM_SPEED != 0 {
        return;
    }
    let Some(p) = g.try_player() else { return };
    let Some(lvl) = p.c.level else { return };
    let (px, py) = (p.c.x >> 4, p.c.y >> 4);
    let depth = g.level(lvl).depth;
    let on_surface = depth == 0;
    let infinite_surface = on_surface && g.level(lvl).is_infinite();
    let seed = g.world_seed;

    let biome = if infinite_surface {
        Some(infinite_gen::biome_at(seed, px, py))
    } else {
        None
    };

    let mut place = None;
    if infinite_surface {
        // "found" = standing inside the placement's radius
        const R: i32 = 40;
        for pl in structures_gen::placements_in_rect(seed, px - R, py - R, px + R, py + R) {
            let r = structures_gen::kind_radius(pl.kind);
            let (dx, dy) = (pl.x - px, pl.y - py);
            if dx * dx + dy * dy <= r * r {
                place = Some(pl.kind);
                break;
            }
        }
    }

    let event = if on_surface { event_happening(g) } else { None };
    let felled = std::mem::take(&mut g.trees_felled_pending);

    let mut cues: Vec<String> = Vec::new();
    {
        let notes = &mut g.player_mut().player_mut().notes;
        notes.trees_felled += felled;
        if depth < notes.deepest_depth {
            notes.deepest_depth = depth;
        }
        if let Some(b) = biome {
            if notes.see_biome(b) {
                cues.push(format!("New country: {}.", biome_title(b).to_lowercase()));
            }
        }
        if let Some(k) = place {
            if notes.find_place(k) {
                cues.push(format!(
                    "Marked in the notes: {}.",
                    place_title(k).to_lowercase()
                ));
            }
        }
        if let Some(ev) = event {
            // no cue — events announce themselves (core::events dusk warnings)
            notes.witness_event(ev);
        }
    }
    for cue in cues {
        g.push_cue(&cue);
    }
}
