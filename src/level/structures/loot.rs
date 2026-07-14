//! Structure entity positions and deterministic loot spawning.

use super::super::chunk::{CHUNK_SIZE, chunk_coord};
use super::super::infinite_gen::{hash, unit};
use super::placement::{MAX_RADIUS, Placement, StructureKind, placements_in_rect, variant_of};
use super::towns::{
    TownAge, house_container_offsets, town_age, town_buildings, village_buildings,
    village_lantern_offset,
};
use crate::core::game::Game;
use crate::rng::Rng;

/// Where a ruins chest sits relative to the origin — interior floor in every shape
/// (the L-shape's origin lies outside the L, in the notch, so its chest moves into
/// the room overlap).
pub(crate) fn ruins_chest_offset(variant: u32) -> (i32, i32) {
    if variant == 1 { (-3, -3) } else { (0, 0) }
}

/// The global tiles the structure's loot chests sit on (empty for chestless kinds).
/// Pure, so exactly one chunk (the one containing each tile) owns each spawn.
pub fn chest_positions(seed: i64, p: Placement) -> Vec<(i32, i32)> {
    match p.kind {
        // ~60% of ruins hide a chest on the room floor
        StructureKind::Ruins => {
            if unit(hash(seed, 0xB1DE_0005, p.x, p.y)) < 0.60 {
                let (dx, dy) = ruins_chest_offset(variant_of(seed, p));
                vec![(p.x + dx, p.y + dy)]
            } else {
                Vec::new()
            }
        }
        // every camp has one, beside the fire
        StructureKind::Camp => vec![(p.x + 2, p.y)],
        // villages hold 1-2, centered in the first buildings (always plank floor)
        StructureKind::Village => {
            let b = village_buildings(seed, p.x, p.y, variant_of(seed, p));
            let mut out = vec![(b[0].0, b[0].1)];
            if unit(hash(seed, 0x56C4_0008, p.x, p.y)) < 0.5 {
                out.push((b[1].0, b[1].1));
            }
            out
        }
        _ => Vec::new(),
    }
}

/// Where a placement spawns a burnt-out (ember) campfire entity: the fire-ring
/// center of every *cold-camp* variant (lean-to camps keep their torch instead).
/// Pure, like [`chest_positions`], so exactly one chunk owns the spawn.
pub fn campfire_positions(seed: i64, p: Placement) -> Vec<(i32, i32)> {
    match p.kind {
        StructureKind::Camp if variant_of(seed, p) != 0 => vec![(p.x, p.y)],
        _ => Vec::new(),
    }
}

/// Where a placement spawns lit lantern entities: one per town house, in the
/// interior corner away from the doorway (see [`village_lantern_offset`] — same
/// lore as the plaza Jack-O-Lantern: someone, or something, keeps them burning).
/// At night the glow leaves through the window panes and wall gaps, which is what
/// makes town houses read as destinations instead of dead shells (playtest #8).
///
/// The town's age bends the count: OVERGROWN towns burnt out ages ago (at most one
/// stubborn flame survives), WEATHERED keeps the classic one-per-house, SETTLED adds
/// a lamp by the town center on top — the lit-up skyline IS the freshness read.
/// Pure, like [`chest_positions`], so exactly one chunk owns each spawn.
pub fn lantern_positions(seed: i64, p: Placement) -> Vec<(i32, i32)> {
    match p.kind {
        StructureKind::Village | StructureKind::Hamlet => {
            let mut out: Vec<(i32, i32)> = town_buildings(seed, p)
                .into_iter()
                .map(|(bx, by, hw, hh)| {
                    let (dx, dy) = village_lantern_offset(p.x, p.y, bx, by, hw, hh);
                    (bx + dx, by + dy)
                })
                .collect();
            match town_age(seed, p) {
                TownAge::Overgrown => {
                    let one_survives = hash(seed, 0x0A6E_D001, p.x, p.y) & 1 == 0;
                    out.truncate(if one_survives { 1 } else { 0 });
                }
                TownAge::Weathered => {}
                TownAge::Settled => {
                    // the town-center lamp (its footing is stamped by the blueprint)
                    let off = if p.kind == StructureKind::Village {
                        (-3, -2)
                    } else {
                        (-2, -2)
                    };
                    out.push((p.x + off.0, p.y + off.1));
                }
            }
            out
        }
        _ => Vec::new(),
    }
}

/// Where a placement spawns scavenge containers (supply crates, barrels, cupboards)
/// and which kind each spot holds. Towns carry most of them — a cupboard in the
/// house corner opposite the lantern, a rain barrel flanking the doorway — with the
/// density leaning on the town's age: SETTLED holds the most intact stock, OVERGROWN
/// keeps only the odd untouched hold (whose loot leans time-capsule instead — see
/// [`fill_scav_container`]). Camps sometimes keep a supply crate by the fire, ruins
/// a barrel in the rubble. Pure, like [`chest_positions`], so exactly one chunk owns
/// each spawn.
pub fn container_positions(
    seed: i64,
    p: Placement,
) -> Vec<(i32, i32, crate::entity::furniture::scav_container::ScavKind)> {
    use crate::entity::furniture::scav_container::ScavKind;
    let mut out = Vec::new();
    match p.kind {
        StructureKind::Village | StructureKind::Hamlet => {
            // (cupboard odds, doorway-barrel odds) by age
            let (cup_odds, barrel_odds) = match town_age(seed, p) {
                TownAge::Overgrown => (0.40, 0.10),
                TownAge::Weathered => (0.65, 0.25),
                TownAge::Settled => (0.90, 0.60),
            };
            for (i, (bx, by, hw, hh)) in town_buildings(seed, p).into_iter().enumerate() {
                let (cup, barrel) = house_container_offsets(p.x, p.y, bx, by, hw, hh);
                let h = hash(seed, 0x5CAF_0001_u64.wrapping_add(i as u64), p.x, p.y);
                if unit(h) < cup_odds {
                    out.push((bx + cup.0, by + cup.1, ScavKind::Cupboard));
                }
                if unit(hash(seed, 0x5CAF_0002_u64.wrapping_add(i as u64), p.x, p.y)) < barrel_odds
                {
                    out.push((bx + barrel.0, by + barrel.1, ScavKind::Barrel));
                }
            }
        }
        StructureKind::Camp => {
            // half the camps kept their supply crate, on the clearing's south edge
            if unit(hash(seed, 0x5CAF_0003, p.x, p.y)) < 0.50 {
                out.push((p.x - 2, p.y + 2, ScavKind::Crate));
            }
        }
        StructureKind::Ruins => {
            let h = hash(seed, 0x5CAF_0004, p.x, p.y);
            if unit(h) < 0.45 {
                // interior floor in every ruin shape (the L-shape's interior lies
                // up-left of the origin, like its chest)
                let (dx, dy) = if variant_of(seed, p) == 1 {
                    (-2, -3)
                } else {
                    (1, -1)
                };
                let kind = if h & (1 << 40) != 0 {
                    ScavKind::Barrel
                } else {
                    ScavKind::Crate
                };
                out.push((p.x + dx, p.y + dy, kind));
            }
        }
        _ => {}
    }
    out
}

/// Spawn structure entities (loot chests, scavenge containers, cold-camp ember
/// campfires, town house lanterns) for a chunk that was just generated fresh. Marks
/// the chunk dirty so it persists to disk and never generates fresh again — that's
/// what prevents duplicate spawns (and what makes container loot strictly one-time).
/// Chunks explored before a structure feature shipped are NOT retrofitted: they were
/// saved to disk and never re-run through this path.
pub fn spawn_chunk_entities(g: &mut Game, lvl: usize, cx: i32, cy: i32) {
    if g.level(lvl).depth != 0 || !g.level(lvl).is_infinite() {
        return;
    }
    let seed = g.world_seed;
    let base_x = cx * CHUNK_SIZE;
    let base_y = cy * CHUNK_SIZE;
    let placements = placements_in_rect(
        seed,
        base_x - MAX_RADIUS,
        base_y - MAX_RADIUS,
        base_x + CHUNK_SIZE - 1 + MAX_RADIUS,
        base_y + CHUNK_SIZE - 1 + MAX_RADIUS,
    );
    // touch the tile's data byte (same value) purely to set the chunk's dirty flag
    let touch = |g: &mut Game, tx: i32, ty: i32| {
        let data = g.level(lvl).get_data(tx, ty);
        g.level_mut(lvl).set_data(tx, ty, data);
    };
    for p in placements {
        for (tx, ty) in chest_positions(seed, p) {
            if chunk_coord(tx) != cx || chunk_coord(ty) != cy {
                continue; // another chunk owns this chest
            }
            let mut chest = crate::entity::furniture::chest::new();
            fill_structure_chest(g, &mut chest, p.kind, hash(seed, 0x100D_0006, tx, ty));
            g.level_mut(lvl).add_at(chest, tx, ty, true, lvl);
            touch(g, tx, ty);
        }
        for (tx, ty, kind) in container_positions(seed, p) {
            if chunk_coord(tx) != cx || chunk_coord(ty) != cy {
                continue; // another chunk owns this container
            }
            let mut container = crate::entity::furniture::scav_container::new(kind);
            fill_scav_container(
                g,
                &mut container,
                kind,
                p.kind,
                town_age(seed, p),
                hash(seed, 0x5CAF_100D, tx, ty),
            );
            g.level_mut(lvl).add_at(container, tx, ty, true, lvl);
            touch(g, tx, ty);
        }
        for (tx, ty) in campfire_positions(seed, p) {
            if chunk_coord(tx) != cx || chunk_coord(ty) != cy {
                continue; // another chunk owns this campfire
            }
            let ember = crate::entity::furniture::campfire::new_ember();
            g.level_mut(lvl).add_at(ember, tx, ty, true, lvl);
            touch(g, tx, ty);
        }
        for (tx, ty) in lantern_positions(seed, p) {
            if chunk_coord(tx) != cx || chunk_coord(ty) != cy {
                continue; // another chunk owns this lantern
            }
            let lantern = crate::entity::furniture::lantern::new(
                crate::entity::furniture::lantern::LanternType::Norm,
            );
            g.level_mut(lvl).add_at(lantern, tx, ty, true, lvl);
            touch(g, tx, ty);
        }
    }
}

/// Modest early-game loot, deterministic per chest position.
pub fn fill_structure_chest(
    g: &mut Game,
    chest: &mut crate::entity::Entity,
    kind: StructureKind,
    h: u64,
) {
    use crate::item::registry::get;
    let mut rnd = Rng::new(h as i64);

    // (1-in-chance, item, count) — same convention as the spawner-dungeon chests
    let loot: &[(i32, &str, i32)] = match kind {
        // a sacked village is the richest find of the four
        StructureKind::Village => &[
            (2, "Torch", 3),
            (2, "Stone", 6),
            (2, "Bread", 2),
            (3, "Wood", 6),
            (3, "Cord", 2),
            (4, "Apple", 2),
            (5, "Coal", 4),
            (8, "Iron", 2),
            (12, "Gold", 1),
            // farming wave: the hamlet's seed stock survived in its larders
            (2, "Corn Kernels", 3),
            (3, "Carrot Seeds", 2),
            (4, "Pumpkin Seeds", 2),
            // a village weaver's kit for THE BENCH
            (8, "Spindle", 1),
        ],
        StructureKind::Ruins => &[
            (2, "Torch", 3),
            (2, "Stone", 6),
            (3, "Wood", 5),
            (3, "Cord", 2),
            (3, "Bread", 2),
            (4, "Apple", 2),
            (5, "Coal", 3),
            (10, "Iron", 1),
            // THE BENCH's loot shortcut: whoever worked this place left their
            // kit behind (the module recipes at the bench remain the sure path)
            (9, "Vice", 1),
            (11, "Assay Kit", 1),
            // field-notes journal: the claim's fletcher wrote down the trade
            // (teaches a variant only — a find, never a gate)
            (10, "Fletcher's Diary", 1),
        ],
        _ => &[
            (2, "Torch", 2),
            (2, "Bread", 2),
            (2, "Wood", 4),
            (3, "Cord", 3),
            (4, "arrow", 4),
            (5, "Apple", 2),
            (12, "Iron", 1),
        ],
    };
    let inventory = &mut chest.chest_mut().expect("chest").inventory;
    for &(chance, name, num) in loot {
        let item = get(g, name);
        inventory.try_add_num(&mut rnd, chance, Some(item), num);
    }
    // never leave a completely empty chest
    if inventory.inv_size() < 1 {
        inventory.add_num(get(g, "Wood"), 4);
        inventory.add_num(get(g, "Torch"), 2);
    }
}

/// Seed a scavenge container's one-time finds, deterministic per world position.
/// The base table follows the furniture (cupboards keep pantry goods, barrels
/// stores, crates gear); the structure's age leans it — an OVERGROWN hold is a time
/// capsule (old coins, metal, the odd prospector's note), a SETTLED one still has
/// useful supplies on the shelf. Camps and ruins draw whatever age their spot hashes
/// to: some caches are simply older than others.
pub fn fill_scav_container(
    g: &mut Game,
    container: &mut crate::entity::Entity,
    kind: crate::entity::furniture::scav_container::ScavKind,
    structure: StructureKind,
    age: TownAge,
    h: u64,
) {
    use crate::entity::furniture::scav_container::ScavKind;
    use crate::item::registry::get;
    let mut rnd = Rng::new(h as i64);

    // (1-in-chance, item, count) — same convention as the structure chests
    let base: &[(i32, &str, i32)] = match kind {
        ScavKind::Cupboard => &[
            (2, "Old Food Can", 2),
            (2, "Bread", 1),
            (3, "Water Bottle", 1),
            (3, "Apple", 2),
            (4, "Empty Can", 2),
            (4, "Mushroom", 2),
        ],
        ScavKind::Barrel => &[
            (2, "Water Bottle", 2),
            (2, "Cord", 3),
            (3, "Grass Fibers", 4),
            (3, "Apple", 2),
            (4, "Coal", 3),
            (5, "Old Food Can", 1),
        ],
        ScavKind::Crate => &[
            (2, "Torch", 3),
            (2, "arrow", 5),
            (3, "Cord", 2),
            (3, "Bandage", 1),
            (4, "Coal", 4),
            (4, "Throwing Knife", 2),
            (6, "Iron", 2),
        ],
    };
    let lean: &[(i32, &str, i32)] = match age {
        // untouched for generations: worth the hunt through the bracken
        TownAge::Overgrown => &[
            (2, "Old Coin", 3),
            (3, "Iron", 2),
            (4, "Prospector's Note", 1),
            (5, "Gold", 1),
            (8, "gem", 2),
        ],
        TownAge::Weathered => &[
            (3, "Empty Can", 1),
            (5, "Old Coin", 1),
            (10, "Prospector's Note", 1),
        ],
        TownAge::Settled => &[
            (2, "Bread", 1),
            (3, "Torch", 2),
            (4, "Water Bottle", 1),
            (8, "Old Coin", 1),
        ],
    };
    let inventory = &mut container.chest_mut().expect("scav container").inventory;
    // Field-notes journals — a found technique, never a gate (UI_REDESIGN §4).
    // Each rides its own row, never sharing a slot with the Prospector's Note
    // entries in the age leans above: Tanner's in hamlet cupboards, Wickmaker's
    // in camp crates, Trapper's in overgrown-town time capsules.
    let journal: &[(i32, &str, i32)] = match (structure, kind) {
        (StructureKind::Hamlet, ScavKind::Cupboard) => &[(9, "Tanner's Notes", 1)],
        (StructureKind::Camp, ScavKind::Crate) => &[(8, "Wickmaker's Page", 1)],
        (StructureKind::Hamlet | StructureKind::Village, _) if age == TownAge::Overgrown => {
            &[(12, "Trapper's Field Guide", 1)]
        }
        _ => &[],
    };
    for &(chance, name, num) in base.iter().chain(lean).chain(journal) {
        let item = get(g, name);
        inventory.try_add_num(&mut rnd, chance, Some(item), num);
    }
    // a rummage should never come up completely dry
    if inventory.inv_size() < 1 {
        inventory.add_num(get(g, "Empty Can"), 1);
        inventory.add_num(get(g, "Cord"), 2);
    }
}
