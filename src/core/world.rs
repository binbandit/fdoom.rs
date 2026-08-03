//! Port of `fdoom.core.World` — world lifecycle: reset, init, level changes — plus the
//! world-population tail of the Java `Level` constructor.

use crate::core::game::Game;
use crate::entity::mob::player;
use crate::level;

/// Java `World.resetGame()` / `resetGame(keepPlayer)` — "half"-starts a new game.
pub fn reset_game(g: &mut Game, keep_player: bool) {
    crate::log_info!("resetting game (keep_player: {keep_player})");
    g.player_dead_time = 0;
    g.current_level = 3;
    g.as_tick = 0;
    g.clear_notifications();

    // adds a new player (isValidServer is always false here)
    let prev_data = if keep_player {
        g.entities.get(g.player_id).map(|e| e.player().clone())
    } else {
        None
    };
    let mut new_player = player::new(g, prev_data.as_ref());
    new_player.c.eid = g.player_id;
    g.entities.delete(g.player_id);
    g.entities.put_back(new_player);
    crate::entity::mob::player_behavior::maybe_fill_creative_inv(g);

    if g.levels[g.current_level].is_none() {
        return;
    }

    // "shouldRespawn" is false on hardcore, or when making a new world
    if g.should_respawn {
        let lvl = g.current_level;
        let mut p = g
            .entities
            .take(g.player_id)
            .expect("the respawn player was put_back into the arena a few lines above");
        crate::entity::mob::player_behavior::respawn(g, &mut p, lvl);
        // adds the player to the current level (always surface here)
        g.level_mut(lvl).add(p, lvl);
    }
}

/// Java `World.scheduleLevelChange(dir)`.
pub fn schedule_level_change(g: &mut Game, dir: i32) {
    g.pending_level_change = dir;
}

/// Take the player entity from wherever it currently lives — the arena, or a level's
/// add-queue when a transition earlier in this tick already moved it.
///
/// `Level::remove` *drops* queued entities outright, so reaching for the player with
/// `remove` + `entities.take` destroyed a queued player and left the next
/// `Game::player()` with nothing to find.
fn take_player_anywhere(g: &mut Game) -> Option<crate::entity::Entity> {
    if let Some(p) = g.entities.take(g.player_id) {
        return Some(p);
    }
    let pid = g.player_id;
    for level in g.levels.iter_mut().flatten() {
        if let Some(i) = level.entities_to_add.iter().position(|e| e.c.eid == pid) {
            return Some(level.entities_to_add.remove(i));
        }
    }
    None
}

/// Snap a pixel coordinate to the centre of its tile without running off the end of
/// the coordinate space (`(x >> 4) * 16 + 8` overflows near `i32::MAX`).
fn tile_center(px: i32) -> i32 {
    let tile = (px >> 4).clamp(crate::level::chunk::MIN_TILE, crate::level::chunk::MAX_TILE);
    tile * 16 + 8
}

/// Java `World.changeLevel(dir)` — moves the player up/down a level.
pub fn change_level(g: &mut Game, dir: i32) {
    let cur = g.current_level;
    let pid = g.player_id;

    // Take the player out first: right after a respawn or a bed wake-up it
    // legitimately sits in a level's add-queue rather than the arena, and
    // `Level::remove` would silently discard it from that queue.
    let Some(mut p) = take_player_anywhere(g) else {
        crate::log_warn!(
            "level change ({dir:+}) skipped: player entity {pid} is in neither the arena nor \
             any level add-queue; staying on level {cur}"
        );
        return;
    };

    // removes the player from the current level
    g.level_mut(cur).remove(pid);

    let mut next_level = cur as i32 + dir;
    if next_level <= -1 {
        next_level = g.levels.len() as i32 - 1; // fix accidental level underflow
    }
    if next_level >= g.levels.len() as i32 {
        next_level = 0; // fix accidental level overflow
    }
    g.current_level = next_level as usize;

    // center the player on the stairs (clamped: the shift-and-add overflowed near i32::MAX)
    p.c.x = tile_center(p.c.x);
    p.c.y = tile_center(p.c.y);
    let (mut px, mut py) = (p.c.x, p.c.y);

    let lvl = g.current_level;

    // Landing on a finite set-piece level (sky/dungeon) from an infinite layer: the
    // matching stairs may be at completely different coordinates, so relocate onto the
    // nearest counterpart stairs (descending arrives on Stairs Up, ascending on Stairs
    // Down). Classic finite<->finite transitions already line up and are left alone.
    if !g.level(lvl).is_infinite() {
        let expected = if dir > 0 { "Stairs Down" } else { "Stairs Up" };
        let expected_id = g.tiles.get(expected).id;
        let (xt, yt) = (px >> 4, py >> 4);
        let in_bounds = xt >= 0 && yt >= 0 && xt < g.level(lvl).w && yt < g.level(lvl).h;
        if !in_bounds || g.tile_at(lvl, xt, yt).id != expected_id {
            let matches = level::get_matching_tiles(g, lvl, |_, t, _, _| t.id == expected_id);
            let target = matches
                .iter()
                .min_by_key(|pt| {
                    let dx = pt.x - xt;
                    let dy = pt.y - yt;
                    dx * dx + dy * dy
                })
                .map(|pt| (pt.x, pt.y))
                .unwrap_or((g.level(lvl).w / 2, g.level(lvl).h / 2));
            px = target.0 * 16 + 8;
            py = target.1 * 16 + 8;
            p.c.x = px;
            p.c.y = py;
        }
    }

    g.level_mut(lvl).add_at(p, px, py, false, lvl);
    crate::level::ensure_chunks(g, lvl);
}

/// Java `Level` constructor tail: link stairs with the parent level, then dungeon/surface
/// specials. Called by initWorld for each generated level.
pub fn populate_from_parent(g: &mut Game, lvl: usize, parent: Option<usize>) {
    let depth = g.level(lvl).depth;
    if let Some(parent) = parent {
        let (w, h) = {
            let l = g.level(lvl);
            (l.w, l.h)
        };
        let stairs_down_id = g.tiles.get("Stairs Down").id;
        let stairs_up = g.tiles.get("Stairs Up");
        for y in 0..h {
            for x in 0..w {
                if g.tile_at(parent, x, y).id == stairs_down_id {
                    g.set_tile_default(lvl, x, y, &stairs_up);
                    if depth == -4 {
                        // make the obsidian wall formation around the dungeon stairs
                        level::structure::draw_dungeon_gate(g, lvl, x, y);
                    } else if depth == 0 {
                        crate::log_debug!("surface stairs at ({x},{y}): ringing with Hard Rock");
                        let hard_rock = g.tiles.get("Hard Rock");
                        level::set_area_tiles(g, lvl, x, y, 1, &hard_rock, 0, false);
                    } else {
                        let dirt = g.tiles.get("dirt");
                        level::set_area_tiles(g, lvl, x, y, 1, &dirt, 0, false);
                    }
                }
            }
        }
    }

    check_chest_count(g, lvl, false);

    if depth < 0 {
        generate_spawner_structures(g, lvl);
    }
}

/// Java `Level.checkChestCount(check)`.
pub fn check_chest_count(g: &mut Game, lvl: usize, check: bool) {
    // if the level is the dungeon, and we're not just loading the world...
    if g.level(lvl).depth != -4 {
        return;
    }

    let mut num_chests = 0;
    if check {
        num_chests += g
            .level(lvl)
            .entities_to_add
            .iter()
            .filter(|e| matches!(e.kind, crate::entity::EntityKind::DungeonChest(_)))
            .count() as i32;
        num_chests += g
            .entities
            .entities_on_level(lvl)
            .filter(|e| matches!(e.kind, crate::entity::EntityKind::DungeonChest(_)))
            .count() as i32;
        crate::log_debug!("dungeon chest census: {num_chests} present before top-up");
    }

    let (w, h) = {
        let l = g.level(lvl);
        (l.w, l.h)
    };
    let obsidian_id = g.tiles.get("Obsidian").id;
    let obsidian_wall_id = g.tiles.get("Obsidian Wall").id;
    let obsidian = g.tiles.get("Obsidian");

    // The obsidian search below retries until it lands on an obsidian tile. A dungeon
    // level with none at all — a damaged save whose level file parses but holds no
    // obsidian — would spin here forever, hanging the load with no error at all. Cap
    // the picks: well-formed dungeons hit obsidian within a handful of tries, so this
    // is unreachable in normal play and leaves generation bit-for-bit identical.
    const MAX_TILE_PICKS: u32 = 10_000;

    // make DungeonChests!
    for _ in num_chests..10 * (w / 128) {
        let mut d = crate::entity::furniture::dungeon_chest::new(g);
        let mut picks = 0;
        loop {
            // pick a random tile:
            let x2 = g.level_mut(lvl).random.next_int_bound(16 * w) / 16;
            let y2 = g.level_mut(lvl).random.next_int_bound(16 * h) / 16;
            picks += 1;
            if g.tile_at(lvl, x2, y2).id != obsidian_id {
                if picks >= MAX_TILE_PICKS {
                    crate::log_warn!(
                        "no obsidian found on level {lvl} after {MAX_TILE_PICKS} tries; \
                         placing no further dungeon chests there"
                    );
                    return;
                }
                continue;
            }
            let xaxis = g.level_mut(lvl).random.next_boolean();
            if xaxis {
                let mut s = x2;
                while s < w - s {
                    if g.tile_at(lvl, s, y2).id == obsidian_wall_id {
                        d.c.x = s * 16 - 24;
                        d.c.y = y2 * 16 - 24;
                    }
                    s += 1;
                }
            } else {
                // Inherited quirk kept for old-save parity: with s starting at y2, the
                // condition `s < y2 - s` is false whenever y2 >= 0, so this vertical
                // scan effectively never runs and the fallback
                // placement below wins instead. Do not "fix" without a save-compat plan.
                let mut s = y2;
                while s < y2 - s {
                    if g.tile_at(lvl, x2, s).id == obsidian_wall_id {
                        d.c.x = x2 * 16 - 24;
                        d.c.y = s * 16 - 24;
                    }
                    s += 1;
                }
            }
            if d.c.x == 0 && d.c.y == 0 {
                d.c.x = x2 * 16 - 8;
                d.c.y = y2 * 16 - 8;
            }
            if g.tile_at(lvl, d.c.x / 16, d.c.y / 16).id == obsidian_wall_id {
                g.set_tile_default(lvl, d.c.x / 16, d.c.y / 16, &obsidian);
            }
            g.level_mut(lvl).add(d, lvl);
            g.level_mut(lvl).chest_count += 1;
            break;
        }
    }
}

/// Java `Level.generateSpawnerStructures()`.
pub fn generate_spawner_structures(g: &mut Game, lvl: usize) {
    let (w, depth) = {
        let l = g.level(lvl);
        (l.w, l.depth)
    };
    let dirt_id = g.tiles.get("dirt").id;
    let rock_id = g.tiles.get("rock").id;
    let dirt = g.tiles.get("dirt");
    let stone_bricks = g.tiles.get("Stone Bricks");
    let stone_wall = g.tiles.get("Stone Wall");
    let stairs_down_id = g.tiles.get("Stairs Down").id;

    for _ in 0..18 / -depth * (w / 128) {
        // for generating spawner dungeons
        let r = g.level_mut(lvl).random.next_int_bound(5);
        let m = if r == 1 {
            crate::entity::mob::stone_golem::new(g, -depth)
        } else if r == 2 || r == 0 {
            crate::entity::mob::snake::new(g, -depth)
        } else {
            crate::entity::mob::zombie::new(g, -depth)
        };

        let mut random_for_spawner = g.random.clone();
        let mut sp = crate::entity::furniture::spawner::new(m, &mut random_for_spawner);
        g.random = random_for_spawner;

        let x3 = g.level_mut(lvl).random.next_int_bound(16 * w) / 16;
        let y3 = g.level_mut(lvl).random.next_int_bound(16 * w) / 16;
        if g.tile_at(lvl, x3, y3).id != dirt_id {
            continue;
        }
        let xaxis2 = g.level_mut(lvl).random.next_boolean();

        if xaxis2 {
            let mut s2 = x3;
            while s2 < w - s2 {
                if g.tile_at(lvl, s2, y3).id == rock_id {
                    sp.c.x = s2 * 16 - 24;
                    sp.c.y = y3 * 16 - 24;
                }
                s2 += 1;
            }
        } else {
            // Same inherited quirk as the dungeon-door scan above: `s2 < y3 - s2` is
            // false whenever y3 >= 0, so the fallback placement below effectively decides.
            let mut s2 = y3;
            while s2 < y3 - s2 {
                if g.tile_at(lvl, x3, s2).id == rock_id {
                    sp.c.x = x3 * 16 - 24;
                    sp.c.y = s2 * 16 - 24;
                }
                s2 += 1;
            }
        }

        if sp.c.x == 0 && sp.c.y == 0 {
            sp.c.x = x3 * 16 - 8;
            sp.c.y = y3 * 16 - 8;
        }

        if g.tile_at(lvl, sp.c.x / 16, sp.c.y / 16).id == rock_id {
            g.set_tile_default(lvl, sp.c.x / 16, sp.c.y / 16, &dirt);
        }

        for xx in 0..5 {
            for yy in 0..5 {
                let tx = sp.c.x / 16 - 2 + xx;
                let ty = sp.c.y / 16 - 2 + yy;
                if g.tile_at(lvl, tx, ty).id != stairs_down_id {
                    g.set_tile_default(lvl, tx, ty, &stone_bricks);
                    if (xx < 1 || yy < 1 || xx > 3 || yy > 3)
                        && (xx != 2 || yy != 0)
                        && (xx != 2 || yy != 4)
                        && (xx != 0 || yy != 2)
                        && (xx != 4 || yy != 2)
                    {
                        g.set_tile_default(lvl, tx, ty, &stone_wall);
                    }
                }
            }
        }

        let (spx, spy) = (sp.c.x, sp.c.y);
        g.level_mut(lvl).add(sp, lvl);

        for _ in 0..2 {
            if g.level_mut(lvl).random.next_int_bound(2) != 0 {
                continue;
            }
            let mut c = crate::entity::furniture::chest::new();
            let chance = -depth;
            fill_spawner_chest(g, &mut c, chance);
            g.level_mut(lvl).add_at(c, spx - 16, spy - 16, false, lvl);
        }
    }
}

/// The spawner-dungeon chest loot table (Java inline in `generateSpawnerStructures`).
fn fill_spawner_chest(g: &mut Game, c: &mut crate::entity::Entity, chance: i32) {
    use crate::item::registry::{get, new_furniture_item, new_tool_item};
    let mut rnd = g.random.clone();

    // `c` comes from `chest::new()` at the sole call site; should anything else ever
    // arrive here, leave it unfilled rather than panic.
    let Some(chest) = c.chest_mut() else {
        crate::log_warn!("spawner-structure loot skipped: the entity handed in is not a chest");
        return;
    };
    let inv = &mut chest.inventory;

    {
        let tnt = new_furniture_item(crate::entity::furniture::tnt::new());
        let anvil = new_furniture_item(crate::entity::furniture::crafter::new(
            crate::entity::furniture::crafter::CrafterType::Anvil,
        ));
        let lantern = new_furniture_item(crate::entity::furniture::lantern::new(
            crate::entity::furniture::lantern::LanternType::Norm,
        ));
        inv.try_add(&mut rnd, 9 / chance, Some(tnt));
        inv.try_add(&mut rnd, 10 / chance, Some(anvil));
        inv.try_add(&mut rnd, 7 / chance, Some(lantern));
    }

    let loot: &[(i32, &str, i32)] = &[
        (3, "bread", 2),
        (4, "bread", 3),
        (7, "Leather Armor", 1),
        (50, "Gold Apple", 1),
        (3, "Lapis", 2),
        (4, "glass", 2),
        (4, "Gunpowder", 3),
        (4, "Gunpowder", 3),
        (4, "Torch", 4),
        (14, "swim potion", 1),
        (16, "haste potion", 1),
        (14, "light potion", 1),
        (14, "speed potion", 1),
        (16, "Iron Armor", 1),
        (5, "Stone Brick", 4),
        (5, "Stone Brick", 6),
        (4, "string", 3),
        (4, "bone", 2),
        (3, "bone", 1),
    ];
    for &(ch, name, num) in loot {
        let item = get(g, name);
        inv.try_add_num(&mut rnd, ch / chance, Some(item), num);
    }

    {
        let claymore = new_tool_item(crate::item::ToolType::Claymore, 1);
        inv.try_add_num(&mut rnd, 7 / chance, Some(claymore), 1);
    }

    let loot2: &[(i32, &str, i32)] = &[
        (5, "Torch", 3),
        (6, "Torch", 6),
        (6, "Torch", 6),
        (7, "steak", 3),
        (9, "steak", 4),
        (7, "gem", 3),
        (7, "gem", 5),
        (7, "gem", 4),
        (10, "yellow clothes", 1),
        (10, "black clothes", 1),
        (12, "orange clothes", 1),
        (12, "cyan clothes", 1),
        (12, "purple clothes", 1),
        (4, "arrow", 5),
    ];
    for &(ch, name, num) in loot2 {
        let item = get(g, name);
        inv.try_add_num(&mut rnd, ch / chance, Some(item), num);
    }

    if inv.inv_size() < 1 {
        let potion = get(g, "potion");
        let coal = get(g, "coal");
        let apple = get(g, "apple");
        let dirt = get(g, "dirt");
        inv.add_num(potion, 1);
        inv.add_num(coal, 3);
        inv.add_num(apple, 3);
        inv.add_num(dirt, 7);
    }

    g.random = rnd;
}

/// Java `World.initWorld()` — full world creation/loading ("this is a full reset").
/// The day-clock tick a brand-new world spawns at: seed-random (not always morning —
/// user request; deterministic per world so respawns of the same seed match), but
/// floored out of the dark band so the first minutes are readable — rolls landing in
/// night or the pre-dawn murk shift forward to early morning (the sleep wake-up
/// tick). Loads are untouched; this only runs at world creation.
pub fn new_world_spawn_time(world_seed: i64) -> i32 {
    let rolled = crate::rng::Rng::new(world_seed ^ 0x7135_A17E)
        .next_int_bound(crate::core::updater::DAY_LENGTH);
    if crate::core::updater::in_dark_band(rolled) {
        crate::core::updater::SLEEP_END_TIME
    } else {
        rolled
    }
}

/// Generate one finite layer (the classic dungeon set piece) from the world seed and
/// the current world settings. Shared by world creation and by the loader, which
/// rebuilds a layer whose save file turned out to be missing or damaged.
pub fn generate_level(g: &mut Game, depth: i32) -> crate::level::Level {
    // a loaded world never restores world_size, so fall back to the size setting
    let world_size = if g.world_size > 0 {
        g.world_size
    } else {
        g.settings.get("size").as_int().max(64)
    };
    let diff_idx = g.settings.get_idx("diff");
    let gen_type = g.settings.get("type").as_str().to_string();
    let theme = g.settings.get("theme").as_str().to_string();
    let world_seed = g.world_seed;

    let mut level = crate::level::Level::empty(world_size, world_size, depth, diff_idx);
    let mut history_random = g.random.clone();
    let maps = crate::level::level_gen::create_and_validate_map(
        world_size,
        world_size,
        depth,
        &g.tiles,
        world_seed,
        &gen_type,
        &theme,
        &mut history_random,
    );
    g.random = history_random;
    match maps {
        Some((tiles, data)) => {
            level.tiles = tiles;
            level.data = data;
        }
        None => crate::log_error!(
            "level generation at depth {depth} produced no maps \
             (size {world_size}, type {gen_type:?}, theme {theme:?}); the level is left empty"
        ),
    }
    level
}

pub fn init_world(g: &mut Game) {
    crate::log_info!("initializing world (seed {:#x})", g.world_seed);

    g.should_respawn = false;
    // Incidental gameplay randomness derives from the world seed, like generation
    // does — the single choke point every world (new, loaded, or test) passes
    // through. Wall-clock seeding made the same save roll differently every run.
    g.random.set_seed(g.world_seed ^ 0x9E37_79B9);
    reset_game(g, true);
    // a brand-new player (fresh stats and inventory) replaces the respawn-reset one
    reset_game_fresh_player(g);
    crate::entity::furniture::bed_behavior::remove_players(g);
    g.game_time = 0;
    g.gamespeed = 1.0;

    // resets tickCount; game starts in the day, so that it's nice and bright
    g.change_time_of_day(crate::core::updater::Time::Morning);
    g.game_over = false;

    g.levels = (0..crate::level::IDX_TO_DEPTH.len())
        .map(|_| None)
        .collect();
    // clear all non-player entities from the arena (Java replaced the levels array)
    let ids: Vec<i32> = g
        .entities
        .iter()
        .map(|e| e.c.eid)
        .filter(|&id| id != g.player_id)
        .collect();
    for eid in ids {
        g.entities.delete(eid);
    }
    if let Some(p) = g.entities.get_mut(g.player_id) {
        p.c.level = None;
    }

    g.loading_percentage = 0.0;

    g.world_load_failed = false;

    if crate::screen::world_select::loaded_world(g) {
        let name = crate::screen::world_select::get_world_name(g);
        if !crate::saveload::load::load_world(g, &name) {
            // The save could not be read at all. Leave it untouched on disk and tell
            // the caller to bounce back to the title screen: entering gameplay on a
            // world that never loaded took the game down on the very next tick, and
            // an autosave would then have overwritten the recoverable save.
            g.world_load_failed = true;
        }
    } else {
        g.world_size = g.settings.get("size").as_int();
        let world_size = g.world_size;
        let infinite = true; // worlds are always infinite (user direction)

        let loading_inc =
            100.0 / (crate::level::MAX_LEVEL_DEPTH - crate::level::MIN_LEVEL_DEPTH + 1) as f32;
        let diff_idx = g.settings.get_idx("diff");
        let world_seed = g.world_seed;

        // i = level depth; starts from the top because the parent level is the reference
        let mut i = crate::level::MAX_LEVEL_DEPTH;
        while i >= crate::level::MIN_LEVEL_DEPTH {
            crate::log_info!("generating level at depth {i}");
            g.loading_message = crate::level::get_depth_string(i);

            let idx = crate::level::lvl_idx(i);
            let parent_idx = crate::level::lvl_idx(i + 1);
            let parent = if i == crate::level::MAX_LEVEL_DEPTH {
                None
            } else {
                Some(parent_idx)
            };

            if infinite && (-3..=0).contains(&i) {
                // infinite layer: chunks stream in around the player, no upfront gen
                let mut level = crate::level::Level::empty(world_size, world_size, i, diff_idx);
                level.chunks = Some(crate::level::chunk::ChunkMap::default());
                level.reseed(g.world_seed);
                g.levels[idx] = Some(level);
                g.loading_percentage += loading_inc;
                i -= 1;
                continue;
            }

            g.levels[idx] = Some(generate_level(g, i));
            if let Some(l) = g.levels[idx].as_mut() {
                l.reseed(g.world_seed);
            }
            // parent-stairs linkage only applies between finite neighbors
            let parent_is_finite = parent
                .map(|pidx| g.levels[pidx].as_ref().is_some_and(|l| !l.is_infinite()))
                .unwrap_or(false);
            if parent_is_finite {
                populate_from_parent(g, idx, parent);
            } else if i == crate::level::MIN_LEVEL_DEPTH && infinite {
                // the dungeon under an infinite world gets a landing gate in the middle,
                // where players arriving through deep-mine gates are relocated
                let (cx, cy) = (world_size / 2, world_size / 2);
                let stairs_up = g.tiles.get("Stairs Up");
                g.set_tile_default(idx, cx, cy, &stairs_up);
                level::structure::draw_dungeon_gate(g, idx, cx, cy);
            }

            g.loading_percentage += loading_inc;
            i -= 1;
        }

        crate::log_info!("level generation complete");

        g.past_day1 = false;
        g.set_time(new_world_spawn_time(world_seed));

        let lvl = g.current_level; // sets level to the current level (3; surface)
        let mut p = g.entities.take(g.player_id).expect(
            "reset_game_fresh_player put the new player in the arena and the purge above skips it",
        );
        // First-day thread: brand-new worlds (never loads) get the one-time
        // tall-grass hint about a minute in (player_behavior counts this down).
        p.player_mut().grass_cue_delay = 60 * crate::core::updater::NORM_SPEED;
        if infinite {
            let (sx, sy) = crate::level::infinite_gen::find_surface_spawn(world_seed, &g.tiles);
            p.c.x = sx * 16 + 8;
            p.c.y = sy * 16 + 8;
        } else {
            crate::entity::mob::player_behavior::find_start_pos(g, &mut p, lvl, Some(g.world_seed));
        }
        g.level_mut(lvl).add(p, lvl);
        crate::level::ensure_chunks(g, lvl);
    }

    // Last line of defence: `Game::level()` is a panicking accessor, so an empty layer
    // slot is a crash waiting for the first tick that touches it. Anything still empty
    // here (a refused load, a level file we could not make sense of) gets a real layer.
    for depth in crate::level::IDX_TO_DEPTH {
        let idx = crate::level::lvl_idx(depth);
        if g.levels[idx].is_none() {
            eprintln!("WORLD WARNING: layer {depth} never came up; filling it in.");
            let mut level = crate::level::Level::empty(
                g.world_size.max(64),
                g.world_size.max(64),
                depth,
                g.settings.get_idx("diff"),
            );
            if crate::level::is_infinite_depth(depth) {
                level.chunks = Some(crate::level::chunk::ChunkMap::default());
            }
            level.reseed(g.world_seed);
            g.levels[idx] = Some(level);
        }
    }
    // ...and the player has to be *somewhere* valid
    if g.current_level >= g.levels.len() {
        g.current_level = crate::level::lvl_idx(0);
    }

    g.ready_to_render_gameplay = true;
    g.should_respawn = true;

    crate::log_info!("world initialized");
}

/// The `player = new Player(null, input)` line in Java initWorld.
fn reset_game_fresh_player(g: &mut Game) {
    let mut new_player = player::new(g, None);
    new_player.c.eid = g.player_id;
    g.entities.delete(g.player_id);
    g.entities.put_back(new_player);
    crate::entity::mob::player_behavior::maybe_fill_creative_inv(g);
}
