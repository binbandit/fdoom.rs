//! The Deer — the world's first true prey. Grazes Forest/Plains clearings by day,
//! never fights back, and bolts faster than the player can run: an open chase always
//! fails. The hunt is a stalk — the player is concealed while standing in tall grass
//! (the mob stealth mechanic, inverted), so the way in is through the grass, down to
//! spear-or-knife range.

use std::sync::LazyLock;

use crate::core::game::Game;
use crate::entity::{Entity, EntityCommon, EntityKind};
use crate::gfx::color;
use crate::gfx::sprite::{MobAnims, compile_mob_sprite_animations};

use super::PassiveMobData;

// TODO(art): a dedicated STAG variant (heavier rack, one hand taller) would make a
// fine rare sighting later — hash-picked like the snake variants. One deer first.
static SPRITES: LazyLock<MobAnims> = LazyLock::new(|| {
    // unpinned true-color art: the atlas cell is assigned at stitch time
    let c = crate::assets::sprite_cell("mobs/deer/frames");
    compile_mob_sprite_animations(c.x, c.y)
});

/// A player in the open inside this radius spooks the deer (px; ~6 tiles).
pub const FLEE_RADIUS_OPEN: i32 = 6 * 16;
/// A player standing in tall grass is not "seen" until this close (px; ~2 tiles).
pub const FLEE_RADIUS_STALKED: i32 = 2 * 16;
/// How long one bolt lasts (ticks): a good stretch, so open chases fail cleanly.
const BOLT_TICKS: i32 = 2 * crate::core::updater::NORM_SPEED;

#[derive(Debug, Clone)]
pub struct DeerData {
    pub passive: PassiveMobData,
    /// Bolt countdown: while positive the deer sprints (no skipped ticks, double
    /// step — faster than the player) along `flee_dir`.
    pub flee_time: i32,
    flee_dir: (i32, i32),
}

pub fn new(g: &Game) -> Entity {
    let diff_idx = g.settings.get_idx("diff");
    let (mut passive, col) =
        PassiveMobData::new(&SPRITES, color::get4(-1, 100, 320, 431), 4, diff_idx);
    // grazing personality: shorter ambles, much longer head-down pauses than the
    // barnyard trio (the passive walk already stands still half the time)
    passive.ai.random_walk_duration = 30;
    passive.ai.random_walk_chance = 70;
    let mut c = EntityCommon::new(4, 3);
    c.col = col;
    Entity::new(
        c,
        EntityKind::Deer(DeerData {
            passive,
            flee_time: 0,
            flee_dir: (0, 0),
        }),
    )
}

/// Whether the deer would be spooked by the closest player right now: within the
/// open flee radius, unless the player is concealed in tall grass (then only within
/// stalk range). Returns the away-direction when spooked.
fn spook_dir(g: &Game, e: &Entity) -> Option<(i32, i32)> {
    let pid = crate::entity::behavior::get_closest_player(g, e)?;
    let p = g.entities.get(pid)?;
    let (xd, yd) = (p.c.x - e.c.x, p.c.y - e.c.y);
    let player_hidden =
        p.c.level
            .map(|lvl| {
                matches!(
                    g.tile_at(lvl, p.c.x >> 4, p.c.y >> 4).kind,
                    crate::level::tile::TileKind::TallGrass { .. }
                )
            })
            .unwrap_or(false);
    let radius = if player_hidden {
        FLEE_RADIUS_STALKED
    } else {
        FLEE_RADIUS_OPEN
    } as i64;
    // i64: the closest player can be tens of thousands of px away on infinite maps
    if (xd as i64).pow(2) + (yd as i64).pow(2) < radius * radius {
        Some((-xd.signum(), -yd.signum()))
    } else {
        None
    }
}

pub fn tick(g: &mut Game, e: &mut Entity) {
    // spook check before the base walk; a landed hit always sets it off too
    let hurt = e.mob().map(|m| m.hurt_time > 0).unwrap_or(false);
    let mut away = spook_dir(g, e);
    if hurt && away.is_none() {
        // hit from beyond the radius (thrown weapon): bolt straight away anyway
        away = crate::entity::behavior::get_closest_player(g, e)
            .and_then(|pid| g.entities.get(pid))
            .map(|p| (-(p.c.x - e.c.x).signum(), -(p.c.y - e.c.y).signum()));
    }

    if let EntityKind::Deer(d) = &mut e.kind {
        if let Some(dir) = away {
            d.flee_time = BOLT_TICKS;
            d.flee_dir = dir;
        }
        let fleeing = d.flee_time > 0;
        if fleeing {
            d.flee_time -= 1;
            // hold the bolt heading against the random-walk reroll
            d.passive.ai.xa = d.flee_dir.0;
            d.passive.ai.ya = d.flee_dir.1;
            d.passive.ai.random_walk_time = d.passive.ai.random_walk_duration;
        }
        // sprint while bolting: every tick (walk_time 1) at double step — the open
        // chase fails; grazing resumes at the usual half-pace amble
        d.passive.ai.mob.walk_time = if fleeing { 1 } else { 2 };
        d.passive.ai.mob.speed = if fleeing { 2 } else { 1 };
    }

    crate::entity::behavior::mobai_tick_base(g, e);
}

pub fn die(g: &mut Game, e: &mut Entity) {
    use crate::item::registry;

    // venison follows the cow's difficulty gating; the hide is always one
    let (min, max) = match g.settings.get("diff").as_str() {
        "Easy" => (1, 3),
        "Hard" => (0, 1),
        _ => (1, 2),
    };
    let venison = registry::get(g, "Venison");
    crate::entity::behavior::mobai_drop_items(g, e, min, max, &[venison]);

    let hide = registry::get(g, "Hide");
    crate::entity::behavior::mobai_drop_items(g, e, 1, 1, &[hide]);

    crate::entity::behavior::passive_mob_die(g, e);
}
