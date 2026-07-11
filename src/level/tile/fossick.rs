//! Fossicking (sandbox era, no Java counterpart): mining as *reading the earth*.
//!
//! The game is named for prospecting, so mining should reward attention, not
//! strip-mining. Four systems share one deterministic per-area richness field
//! (`infinite_gen::richness_at`, creek-scale, identical at every depth):
//!
//! - **Prospector's Pan** — used on Mud, exposed Tidal Flats, or water-adjacent
//!   sand/dirt. Mostly washes up nothing or a stone; coal/iron flecks sometimes; a
//!   gold nugget or gem on rich ground. Some creeks are worth working, most aren't.
//! - **Rock character** (`rock_character`) — a per-position hash makes ~20% of rock
//!   *cracked* (darker, breaks ~40% faster) and ~10% *dense* (pale boss, slower,
//!   better stone yield). Pure data + render/hurt modulation on the one rock tile.
//! - **Cave-ins** — breaking mine rock that opens too wide a gallery (5x5 open-floor
//!   count) with no Timber Prop nearby has a chance to arm a collapse. Narrow
//!   unpropped drives get a rarer corridor roll using a wider prop radius, keeping
//!   timber supports meaningful outside big rooms too. Either path uses the same
//!   telegraph: the ceiling groans, and on the broken tile's next tick rubble (rock
//!   with the `RUBBLE_FLAG` data bit — weak, fast to clear) falls on nearby open floor.
//! - **Timber Prop** — a placeable support post (`tile/timber_prop.rs`) that holds
//!   the ceiling within `PROP_RADIUS`; break it to recover the timber.
//!
//! Rock tile data layout (`tile/rock.rs` reads/writes through the masks here):
//! bit 7 = rubble flag, low 7 bits = accumulated break damage.

use super::TileKind;
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Entity;
use crate::entity::mob::player_behavior::pay_stamina;
use crate::entity::particle::new_smash_particle;
use crate::item::Item;
use crate::level::infinite_gen::{hash, richness_at, unit};
use crate::level::{drop_item, get_entities_in_tiles};

/// The panning dish's item name (a plain stackable; tiles recognize it by name).
pub const PAN_NAME: &str = "Prospector's Pan";
/// Stamina cost per pan of gravel.
const PAN_STAMINA: i32 = 3;

/// Rock data bit 7: this rock is collapse rubble (weak, fast to clear).
pub const RUBBLE_FLAG: i32 = 0x80;
/// Rock data low bits: accumulated break damage.
pub const DAMAGE_MASK: i32 = 0x7F;
/// Rubble crumbles after this much damage (plain rock takes 50).
pub const RUBBLE_HEALTH: i32 = 12;

/// Dirt data value marking an armed collapse fuse (set by `collapse_check` on the
/// freshly broken tile, fired by `fuse_tick` on that tile's next random tick).
pub const COLLAPSE_FUSE: i32 = 255;
/// 5x5 open-floor count at/above which a fresh break can trigger a collapse.
pub const COLLAPSE_OPEN_MIN: i32 = 13;
/// Timber Props within this Chebyshev radius hold the ceiling.
pub const PROP_RADIUS: i32 = 3;
/// 1-in-N collapse odds once the geometry qualifies.
const COLLAPSE_ODDS: i32 = 4;
/// Timber Props within this wider Chebyshev radius hold narrow drives.
pub const CORRIDOR_PROP_RADIUS: i32 = 6;
/// 1-in-N collapse odds for unsupported corridor mining.
const CORRIDOR_COLLAPSE_ODDS: i32 = 80;
/// At most this many rubble tiles fall per collapse.
const RUBBLE_MAX: usize = 4;

/* ---------------------------------- rock character ---------------------------------- */

/// Salt of the per-position rock-character hash.
const CHARACTER_SALT: u64 = 0xC4AC_4ED0; // "cracked"

/// Per-position rock grain: ~20% cracked, ~10% dense, the rest plain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RockCharacter {
    Normal,
    /// Fault-riddled: renders darker, breaks ~40% faster.
    Cracked,
    /// Tight-grained: pale boss in the face, slower to break, better stone yield.
    Dense,
}

/// The character of the rock at a position — a pure hash of `(seed, x, y)`, so it
/// needs no tile data and survives save/reload and chunk regeneration for free.
pub fn rock_character(seed: i64, x: i32, y: i32) -> RockCharacter {
    let r = unit(hash(seed, CHARACTER_SALT, x, y));
    if r < 0.20 {
        RockCharacter::Cracked
    } else if r >= 0.90 {
        RockCharacter::Dense
    } else {
        RockCharacter::Normal
    }
}

/* ------------------------------------- panning -------------------------------------- */

/// What one pan of gravel washed up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanFind {
    Nothing,
    Stone,
    Coal,
    Iron,
    Gold,
    Gem,
}

/// The pan-roll table: pure, so tests can pin it down. `richness` is the shared field
/// value at the panned tile; `roll` is a uniform `[0, 1)` draw. Rich ground pays more
/// often *and* better — every find band widens with richness, rarest first.
pub fn pan_outcome(richness: f64, roll: f64) -> PanFind {
    let gem = 0.001 + 0.004 * richness;
    let gold = gem + 0.004 + 0.026 * richness;
    let iron = gold + 0.020 + 0.080 * richness;
    let coal = iron + 0.040 + 0.100 * richness;
    let stone = coal + 0.300;
    if roll < gem {
        PanFind::Gem
    } else if roll < gold {
        PanFind::Gold
    } else if roll < iron {
        PanFind::Iron
    } else if roll < coal {
        PanFind::Coal
    } else if roll < stone {
        PanFind::Stone
    } else {
        PanFind::Nothing
    }
}

/// Is any of the four neighbors wet ground (a creek/pond/sea edge)? Gates panning on
/// sand and dirt: only worked banks pan, not any dry patch.
pub fn water_adjacent(g: &Game, lvl: usize, xt: i32, yt: i32) -> bool {
    [(0, -1), (0, 1), (-1, 0), (1, 0)].iter().any(|(dx, dy)| {
        matches!(
            g.tile_at(lvl, xt + dx, yt + dy).kind,
            TileKind::Water | TileKind::DeepWater | TileKind::TidalFlat | TileKind::Mud
        )
    })
}

/// Swirl the pan over the tile at `(xt, yt)`. Returns true when the item is the
/// Prospector's Pan and a pan was actually worked (stamina paid). Callers gate on the
/// ground being pannable (mud / exposed flat / wet bank) before calling.
pub fn try_pan(
    g: &mut Game,
    lvl: usize,
    xt: i32,
    yt: i32,
    player: &mut Entity,
    item: &Item,
) -> bool {
    if !item.get_name().eq_ignore_ascii_case(PAN_NAME) {
        return false;
    }
    if !pay_stamina(player, PAN_STAMINA) {
        return false;
    }
    g.play_sound(Sound::MonsterHurt);
    g.level_mut(lvl)
        .add(new_smash_particle(xt * 16, yt * 16), lvl);

    let richness = richness_at(g.world_seed, xt, yt);
    let roll = g.random.next_double();
    let (name, note) = match pan_outcome(richness, roll) {
        PanFind::Nothing => (None, "Nothing but gray sand."),
        PanFind::Stone => (Some("Stone"), "A smooth stone clacks in the pan."),
        PanFind::Coal => (Some("Coal"), "Black flecks settle in the pan."),
        PanFind::Iron => (Some("Iron Ore"), "A rusty gleam in the gravel."),
        PanFind::Gold => (Some("Gold Ore"), "A gold nugget winks up at you!"),
        PanFind::Gem => (Some("gem"), "A gemstone glitters in the silt!"),
    };
    if let Some(name) = name {
        let find = crate::item::registry::get(g, name);
        drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, find);
    }
    g.notifications.push(note.to_string());
    true
}

/* ------------------------------------- cave-ins -------------------------------------- */

/// Open floor for collapse purposes: anything that isn't standing stone/wall. Unloaded
/// chunks read as rock through `tile_at`, so the frontier counts as solid — correct.
fn is_open(kind: &TileKind) -> bool {
    !matches!(
        kind,
        TileKind::Rock | TileKind::Ore { .. } | TileKind::HardRock | TileKind::Wall { .. }
    )
}

/// Is a Timber Prop within `r` tiles (Chebyshev)?
fn prop_within(g: &Game, lvl: usize, x: i32, y: i32, r: i32) -> bool {
    for dy in -r..=r {
        for dx in -r..=r {
            if matches!(g.tile_at(lvl, x + dx, y + dy).kind, TileKind::TimberProp) {
                return true;
            }
        }
    }
    false
}

fn arm_collapse(g: &mut Game, lvl: usize, x: i32, y: i32) {
    g.notifications.push("The ceiling groans...".to_string());
    g.play_sound(Sound::Fuse);
    g.level_mut(lvl).set_data(x, y, COLLAPSE_FUSE);
}

/// Called by `rock.rs` right after a (non-rubble) rock breaks underground: if the
/// resulting gallery is too wide and unpropped, or the corridor is unsupported for a
/// long stretch, sometimes arm a collapse — the warning sounds now, the roof comes
/// down on the broken tile's next random tick.
pub fn collapse_check(g: &mut Game, lvl: usize, x: i32, y: i32) {
    if g.level(lvl).depth >= 0 {
        return; // ceilings only exist underground
    }
    let mut open = 0;
    for dy in -2..=2 {
        for dx in -2..=2 {
            if is_open(&g.tile_at(lvl, x + dx, y + dy).kind) {
                open += 1;
            }
        }
    }

    // Open-gallery path: keep the original 5x5 open-count gate, short prop radius,
    // and 1-in-4 roll intact. A qualifying gallery does not fall back to the rarer
    // corridor roll when this roll misses.
    if open >= COLLAPSE_OPEN_MIN && !prop_within(g, lvl, x, y, PROP_RADIUS) {
        if g.random.next_int_bound(COLLAPSE_ODDS) == 0 {
            arm_collapse(g, lvl, x, y);
        }
        return;
    }

    // Corridor path: narrow drives rarely groan unless a timber prop is within the
    // wider support radius. Props are a complete counter here, not a dampener.
    if !prop_within(g, lvl, x, y, CORRIDOR_PROP_RADIUS)
        && g.random.next_int_bound(CORRIDOR_COLLAPSE_ODDS) == 0
    {
        arm_collapse(g, lvl, x, y);
    }
}

/// Dirt's random tile tick: fire an armed collapse fuse. A prop raised in the beat
/// between the groan and the fall still saves the gallery.
pub fn fuse_tick(g: &mut Game, lvl: usize, x: i32, y: i32) {
    if g.level(lvl).get_data(x, y) != COLLAPSE_FUSE {
        return;
    }
    g.level_mut(lvl).set_data(x, y, 0);
    if prop_within(g, lvl, x, y, PROP_RADIUS) {
        return;
    }

    // fill nearby open floor with rubble — the broken tile first, then its cardinal
    // neighbors — skipping any tile a mob (or the player) is standing on
    let rock = g.tiles.get("rock");
    let mut fell = 0usize;
    for (dx, dy) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, -1)] {
        if fell >= RUBBLE_MAX {
            break;
        }
        let (tx, ty) = (x + dx, y + dy);
        if !is_open(&g.tile_at(lvl, tx, ty).kind) {
            continue;
        }
        let occupied = get_entities_in_tiles(g, lvl, tx, ty, tx, ty)
            .into_iter()
            .any(|id| g.entities.get(id).is_some_and(|e| e.mob().is_some()));
        if occupied {
            continue;
        }
        g.set_tile(lvl, tx, ty, &rock, RUBBLE_FLAG);
        g.level_mut(lvl)
            .add(new_smash_particle(tx * 16, ty * 16), lvl);
        fell += 1;
    }
    if fell > 0 {
        g.notifications.push("The ceiling comes down!".to_string());
        g.play_sound(Sound::Explode);
    }
}

/// Vein-chasing ping, called by `ore.rs` when an ore tile is mined out: every ore tile
/// still hiding within 2 tiles sparkles briefly, so the player follows the seam
/// instead of stripping the wall.
pub fn vein_ping(g: &mut Game, lvl: usize, x: i32, y: i32) {
    for dy in -2..=2i32 {
        for dx in -2..=2i32 {
            if dx == 0 && dy == 0 {
                continue;
            }
            if matches!(g.tile_at(lvl, x + dx, y + dy).kind, TileKind::Ore { .. }) {
                let p = new_smash_particle((x + dx) * 16, (y + dy) * 16);
                g.level_mut(lvl).add(p, lvl);
            }
        }
    }
}
