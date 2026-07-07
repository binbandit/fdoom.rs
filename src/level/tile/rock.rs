//! Port of `fdoom.level.tile.RockTile`, plus the fossicking overhaul's rock reads
//! (see `fossick.rs`):
//!
//! - **Character** — per-position hash: ~20% cracked (darker, breaks ~40% faster),
//!   ~10% dense (pale boss in the face, tougher, better stone yield).
//! - **Rubble** — data bit 7 marks cave-in fall: weak, quick to clear, never coal.
//! - **Highland tier** — infinite-surface mountain rock above the belt's summit line
//!   (`infinite_gen::highland_at`) renders raised (lit north rim, south drop shadow)
//!   and takes double damage to break, dropping extra stone.
//! - **Mineral-seep stains** — surface outcrops on rich ground carry an ochre streak
//!   (`infinite_gen::mineral_stain_at`), advertising the mines directly below.
//! - Breaking (non-rubble) mine rock calls `fossick::collapse_check` (cave-ins).
//!
//! Data layout: bit 7 = rubble flag, low 7 bits = accumulated damage (decays on tick).

use super::fossick::{self, RockCharacter};
use super::{ConnectorSprite, TileDef, TileKind, dirt, dispatch, tool_use};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::entity::particle::{new_smash_particle, new_text_particle};
use crate::gfx::{Screen, Sprite, color};
use crate::item::{Item, ToolType};
use crate::level::drop_items_counted;
use crate::level::infinite_gen;

// Coal eligibility is decided per break, never stored: `hurt_dmg_inner` takes a
// `drops_coal` flag — true from the pickaxe interact (and creative breaks), false from
// mob damage and the generic `hurt_dmg` dispatch entry (explosions). Rock mined with a
// pickaxe can drop coal; rock smashed or blown up just crumbles to stone.

/// Java `RockTile` constructor.
pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Rock);
    def.blocks_light = true; // solid stone occludes emitter light
    def.csprite = Some(ConnectorSprite::new(
        Sprite::new(4, 0, 3, 3, color::get4(111, 444, 555, 321), 3),
        Sprite::new(7, 0, 2, 2, color::get4(111, 444, 555, 321), 3),
        // dedicated fractured-plate texture (artgen `stone_texture`, cells 25..28,3):
        // 0 = lit plate edges, 1 = stone face, 2 = cracks, 3 = deep pits
        Sprite::dots_at(25, 3, color::get4(555, 444, 333, 111)),
    ));
    def
}

/// Highland ("tier-2 summit") rock: only on the infinite surface, where the belt
/// field says this tile sits above the summit line.
fn is_highland(g: &Game, lvl: usize, x: i32, y: i32) -> bool {
    g.level(lvl).depth == 0
        && g.level(lvl).is_infinite()
        && infinite_gen::highland_at(g.world_seed, x, y)
}

pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    // Edge shade blends into the surrounding ground instead of always dirt-brown
    // (a lone rock in a meadow used to get a brown halo). Sample the four neighbors
    // and match the dominant ground family.
    let mut grass_n = 0;
    let mut sand_n = 0;
    let mut snow_n = 0;
    let mut water_n = 0;
    for (nx, ny) in [(x, y - 1), (x, y + 1), (x - 1, y), (x + 1, y)] {
        let t = g.tile_at(lvl, nx, ny);
        if matches!(t.kind, TileKind::Snow) {
            snow_n += 1;
        } else if matches!(t.kind, TileKind::Water | TileKind::DeepWater) {
            water_n += 1; // skerries: sea stacks shade into the water, not dirt
        } else if t.connects_to_sand {
            sand_n += 1;
        } else if t.connects_to_grass {
            grass_n += 1;
        }
    }
    let bg = if snow_n >= grass_n.max(sand_n) && snow_n > 0 {
        color::hex("#e8eef4")
    } else if water_n > grass_n.max(sand_n) {
        115
    } else if sand_n > grass_n {
        550
    } else if grass_n > 0 {
        141
    } else {
        dirt::d_col(g.level(lvl).depth)
    };
    let col = color::get4(111, 444, 555, bg);
    let full = def.csprite.as_ref().map(|cs| cs.full.color).unwrap_or(0);
    dispatch::csprite_render(g, screen, def, lvl, x, y, Some((col, col, full)));

    let (px, py) = (x * 16, y * 16);
    let data = g.level(lvl).get_data(x, y);
    if data & fossick::RUBBLE_FLAG != 0 {
        // cave-in rubble: a darker, broken-faced pile with pale fracture flecks
        screen.darken_rect(px, py, 16, 16, 36);
        let fleck = color::get(-1, 444);
        screen.render(px + 2, py + 6, 2 + 29 * 32, fleck, 0);
        screen.render(px + 8, py + 3, 2 + 29 * 32, fleck, 1);
        return;
    }

    match fossick::rock_character(g.world_seed, x, y) {
        // cracked: same crack art, shaded darker — reads as fault-riddled stone
        RockCharacter::Cracked => screen.darken_rect(px, py, 16, 16, 22),
        // dense: a pale tight-grained boss in the middle of the face
        RockCharacter::Dense => screen.render(px + 4, py + 4, 2 + 29 * 32, color::get(-1, 555), 0),
        RockCharacter::Normal => {}
    }

    if g.level(lvl).depth == 0 && g.level(lvl).is_infinite() {
        let seed = g.world_seed;
        if infinite_gen::highland_at(seed, x, y) {
            // raised summit read: bright chips along the north rim, flanks shaded,
            // and a hard drop shadow off the south edge
            let rim = color::get(-1, 555);
            screen.render(px + 1, py, 2 + 29 * 32, rim, 0);
            screen.render(px + 7, py, 2 + 29 * 32, rim, 1);
            screen.darken_rect(px, py + 2, 1, 12, 30);
            screen.darken_rect(px + 15, py + 2, 1, 12, 30);
            screen.darken_rect(px, py + 14, 16, 2, 70);
        }
        if infinite_gen::mineral_stain_at(seed, x, y) {
            // mineral seep: a damp streak with ochre flecks running down the face
            let h = infinite_gen::hash(seed, 0x5EE9_F1EC, x, y);
            let cx = 3 + ((h >> 8) % 9) as i32;
            let len = 6 + ((h >> 16) % 5) as i32;
            screen.darken_rect(px + cx, py + 2, 2, len, 30);
            let ochre = color::get(-1, 420);
            screen.render(px + cx - 3, py + 2, 2 + 29 * 32, ochre, 0);
            screen.render(px + cx - 2, py + 7, 2 + 29 * 32, ochre, 1);
        }
    }
}

pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
    false
}

#[allow(clippy::too_many_arguments)]
pub fn hurt_by(
    g: &mut Game,
    def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
    _source: &mut Entity,
    _dmg: i32,
    _attack_dir: Direction,
) -> bool {
    // Mob smashing: stone only, no coal (see the drops_coal note above).
    hurt_dmg_inner(g, def, lvl, x, y, 1, false);
    true
}

#[allow(clippy::too_many_arguments)]
pub fn interact(
    g: &mut Game,
    def: &TileDef,
    lvl: usize,
    xt: i32,
    yt: i32,
    player: &mut Entity,
    item: &mut Item,
    _attack_dir: Direction,
) -> bool {
    if let Some(tool_level) = tool_use(g, player, item, ToolType::Pickaxe, 4) {
        let dmg = g.random.next_int_bound(10) + tool_level * 5 + 10;
        // Pickaxe mining: eligible for coal drops.
        hurt_dmg_inner(g, def, lvl, xt, yt, dmg, true);
        return true;
    }
    false
}

/// Generic damage entry (dispatch/explosions) — smashed rock drops stone, not coal.
pub fn hurt_dmg(g: &mut Game, def: &TileDef, lvl: usize, x: i32, y: i32, dmg: i32) {
    hurt_dmg_inner(g, def, lvl, x, y, dmg, false);
}

fn hurt_dmg_inner(
    g: &mut Game,
    _def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
    dmg: i32,
    drops_coal: bool,
) {
    let mut dmg = dmg;
    let data = g.level(lvl).get_data(x, y);
    let rubble = data & fossick::RUBBLE_FLAG != 0;
    let character = if rubble {
        RockCharacter::Normal
    } else {
        fossick::rock_character(g.world_seed, x, y)
    };
    let highland = !rubble && is_highland(g, lvl, x, y);
    // Health varies with the per-position character (cracked ~40% faster, dense
    // slower), doubles for highland summit rock, and cave-in rubble is quick to clear.
    let rock_health = if rubble {
        fossick::RUBBLE_HEALTH
    } else if highland {
        100
    } else {
        match character {
            RockCharacter::Cracked => 30,
            RockCharacter::Dense => 80,
            RockCharacter::Normal => 50,
        }
    };
    let mut damage = (data & fossick::DAMAGE_MASK) + dmg;
    let mut drops_coal = drops_coal && !rubble; // rubble is spent stone: never coal
    if g.is_mode("creative") {
        dmg = rock_health;
        damage = rock_health;
        drops_coal = !rubble;
    }
    g.play_sound(Sound::MonsterHurt);
    g.level_mut(lvl)
        .add(new_smash_particle(x * 16, y * 16), lvl);
    let text = new_text_particle(
        &dmg.to_string(),
        x * 16 + 8,
        y * 16 + 8,
        color::RED,
        &mut g.random,
    );
    g.level_mut(lvl).add(text, lvl);
    if damage >= rock_health {
        // deliberately burn one RNG draw: dropping it would shift every later
        // incidental roll and change seeded outcomes
        let _ = g.random.next_int_bound(1);
        // dense rock shatters into an extra stone, highland rock into two
        let bonus = if highland {
            2
        } else {
            i32::from(character == RockCharacter::Dense)
        };
        if !drops_coal {
            let (min, max) = if rubble { (1, 2) } else { (1, 4) };
            let stone = crate::item::registry::get(g, "Stone");
            drop_items_counted(
                g,
                lvl,
                x * 16 + 8,
                y * 16 + 8,
                min + bonus,
                max + bonus,
                &[stone],
            );
        } else {
            let stone = crate::item::registry::get(g, "Stone");
            drop_items_counted(
                g,
                lvl,
                x * 16 + 8,
                y * 16 + 8,
                1 + bonus,
                2 + bonus,
                &[stone],
            );
            let mut mincoal = 0;
            let mut maxcoal = 1;
            if g.settings.get("diff").as_str() != "Hard" {
                mincoal += 1;
                maxcoal += 1;
            }
            let coal = crate::item::registry::get(g, "coal");
            drop_items_counted(g, lvl, x * 16 + 8, y * 16 + 8, mincoal, maxcoal, &[coal]);
        }
        let dirt = g.tiles.get("dirt");
        g.set_tile_default(lvl, x, y, &dirt);
        // fossicking: a widening unpropped gallery can bring the ceiling down
        // (rubble falls never re-trigger, so collapses don't cascade)
        if !rubble {
            fossick::collapse_check(g, lvl, x, y);
        }
    } else {
        g.level_mut(lvl)
            .set_data(x, y, (data & fossick::RUBBLE_FLAG) | damage);
    }
}

pub fn tick(g: &mut Game, _def: &TileDef, lvl: usize, xt: i32, yt: i32) {
    let data = g.level(lvl).get_data(xt, yt);
    let damage = data & fossick::DAMAGE_MASK;
    if damage > 0 {
        g.level_mut(lvl)
            .set_data(xt, yt, (data & fossick::RUBBLE_FLAG) | (damage - 1));
    }
}
