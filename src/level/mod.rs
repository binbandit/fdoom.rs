//! Port of `fdoom.level.Level` (plus `LevelGen`/`Structure` in their own modules).
//!
//! Levels own the tile arrays; entities live in the global arena (`g.entities`) with a
//! `level` index, so the entity-related Level methods are free functions taking
//! `(g, lvl)` — see PORTING.md.

pub mod chunk;
pub mod infinite_gen;
pub mod level_gen;
pub mod structure;
pub mod tile;

use std::rc::Rc;

use crate::core::game::Game;
use crate::entity::{Entity, EntityKind};
use crate::gfx::{Point, Rectangle};
use crate::rng::Rng;

use tile::TileDef;

const LEVEL_NAMES: [&str; 6] = ["Sky", "Surface", "Iron", "Gold", "Lava", "Dungeon"];

/// Java `Level.getLevelName(depth)`.
pub fn get_level_name(depth: i32) -> &'static str {
    LEVEL_NAMES[(-depth + 1) as usize]
}

/// Java `Level.getDepthString(depth)`.
pub fn get_depth_string(depth: i32) -> String {
    format!(
        "Level {}",
        if depth < 0 {
            format!("B{}", -depth)
        } else {
            depth.to_string()
        }
    )
}

/// Java `Level.MOB_SPAWN_FACTOR`.
pub const MOB_SPAWN_FACTOR: i32 = 100;

/// Java `World.idxToDepth`.
pub const IDX_TO_DEPTH: [i32; 6] = [-3, -2, -1, 0, 1, -4];
pub const MIN_LEVEL_DEPTH: i32 = -4;
pub const MAX_LEVEL_DEPTH: i32 = 1;

/// Java `World.lvlIdx(depth)`.
pub fn lvl_idx(depth: i32) -> usize {
    if depth > MAX_LEVEL_DEPTH {
        return lvl_idx(MIN_LEVEL_DEPTH);
    }
    if depth < MIN_LEVEL_DEPTH {
        return lvl_idx(MAX_LEVEL_DEPTH);
    }
    if depth == -4 {
        return 5;
    }
    (depth + 3) as usize
}

pub struct Level {
    /// Finite dimensions (classic worlds, sky, dungeon). Infinite layers ignore these
    /// for bounds and use `chunks`.
    pub w: i32,
    pub h: i32,
    pub depth: i32,

    pub tiles: Vec<u8>,
    pub data: Vec<u8>,
    pub visible: Vec<bool>,

    /// Chunked storage for infinite layers (None = classic finite level).
    pub chunks: Option<chunk::ChunkMap>,

    pub grass_color: i32,
    pub dirt_color: i32,
    pub sand_color: i32,

    /// affects the number of monsters on the level; bigger = fewer spawns.
    pub monster_density: i32,
    pub max_mob_count: i32,
    pub chest_count: i32,
    pub mob_count: i32,

    /// entities queued to be added to the arena on the next level tick (Java
    /// `entitiesToAdd`).
    pub entities_to_add: Vec<Entity>,
    /// eids queued for removal on the next level tick (Java `entitiesToRemove`).
    pub entities_to_remove: Vec<i32>,

    pub random: Rng,
}

impl Level {
    /// The non-generating part of the Java constructor (`makeWorld == false` path plus
    /// common setup). World generation/population is in `level::init` (needs `g`).
    pub fn empty(w: i32, h: i32, depth: i32, diff_idx: i32) -> Level {
        let mut level = Level {
            w,
            h,
            depth,
            tiles: vec![0; (w * h) as usize],
            data: vec![0; (w * h) as usize],
            visible: vec![false; (w * h) as usize],
            chunks: None,
            grass_color: 141,
            dirt_color: 322,
            sand_color: 550,
            monster_density: 16,
            max_mob_count: 0,
            chest_count: 0,
            mob_count: 0,
            entities_to_add: Vec::new(),
            entities_to_remove: Vec::new(),
            random: Rng::from_time(),
        };
        if depth != -4 && depth != 0 {
            level.monster_density = 8;
        }
        level.update_mob_cap(diff_idx);
        level
    }

    /// Java `updateMobCap()`.
    pub fn update_mob_cap(&mut self, diff_idx: i32) {
        self.max_mob_count = 150 + 150 * diff_idx;
        if self.depth == 1 {
            self.max_mob_count /= 2;
        }
        if self.depth == 0 || self.depth == -4 {
            self.max_mob_count = self.max_mob_count * 2 / 3;
        }
    }

    /// True for chunked infinite layers.
    pub fn is_infinite(&self) -> bool {
        self.chunks.is_some()
    }

    /// Raw tile id at (x, y); None = out of bounds (finite) or unloaded chunk (infinite);
    /// callers treat that as rock (`Game::tile_at`).
    pub fn tile_id(&self, x: i32, y: i32) -> Option<u8> {
        if let Some(chunks) = &self.chunks {
            return chunks.tile(x, y);
        }
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return None;
        }
        Some(self.tiles[(x + y * self.w) as usize])
    }

    /// Java `getData(x, y)`.
    pub fn get_data(&self, x: i32, y: i32) -> i32 {
        if let Some(chunks) = &self.chunks {
            return chunks.data(x, y).unwrap_or(0) as i32;
        }
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return 0;
        }
        // JAVA: `data[x + y * w] & 0xff` (bytes are signed in Java; kept for fidelity)
        #[allow(clippy::identity_op)]
        (self.data[(x + y * self.w) as usize] as i32 & 0xff)
    }

    /// Java `setData(x, y, val)`.
    pub fn set_data(&mut self, x: i32, y: i32, val: i32) {
        if let Some(chunks) = &mut self.chunks {
            chunks.set_data(x, y, val as u8);
            return;
        }
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        self.data[(x + y * self.w) as usize] = val as u8;
    }

    /// Java `setTile(x, y, t, dataVal)` (the singleplayer path).
    pub fn set_tile_id(&mut self, x: i32, y: i32, id: u8, data_val: i32) {
        if let Some(chunks) = &mut self.chunks {
            chunks.set_tile(x, y, id, data_val as u8);
            return;
        }
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        self.tiles[(x + y * self.w) as usize] = id;
        self.data[(x + y * self.w) as usize] = data_val as u8;
    }

    /// Java `add(entity, x, y, tileCoords)` — queues the entity for the next tick.
    pub fn add_at(
        &mut self,
        mut entity: Entity,
        x: i32,
        y: i32,
        tile_coords: bool,
        lvl_idx: usize,
    ) {
        let (x, y) = if tile_coords {
            (x * 16 + 8, y * 16 + 8)
        } else {
            (x, y)
        };
        // Java entity.setLevel(level, x, y)
        entity.c.level = Some(lvl_idx);
        entity.c.removed = false;
        entity.c.x = x;
        entity.c.y = y;

        // to make sure the most recent request is satisfied
        if entity.c.eid >= 0 {
            self.entities_to_remove.retain(|&eid| eid != entity.c.eid);
            self.entities_to_add.retain(|e| e.c.eid != entity.c.eid);
        }
        self.entities_to_add.push(entity);
    }

    /// Java `add(entity)` — uses the entity's current position.
    pub fn add(&mut self, entity: Entity, lvl_idx: usize) {
        let (x, y) = (entity.c.x, entity.c.y);
        self.add_at(entity, x, y, false, lvl_idx);
    }

    /// Java `remove(e)`.
    pub fn remove(&mut self, eid: i32) {
        self.entities_to_add.retain(|e| e.c.eid != eid);
        if !self.entities_to_remove.contains(&eid) {
            self.entities_to_remove.push(eid);
        }
    }
}

/* ------------- Game-level helpers (Java Level methods that need globals) ------------- */

impl Game {
    /// Panicking accessor mirroring Java's implicit non-null level references.
    pub fn level(&self, i: usize) -> &Level {
        self.levels[i].as_ref().expect("level not loaded")
    }

    pub fn level_mut(&mut self, i: usize) -> &mut Level {
        self.levels[i].as_mut().expect("level not loaded")
    }

    /// Java `level.getTile(x, y)` — out of bounds returns "rock".
    pub fn tile_at(&self, lvl: usize, x: i32, y: i32) -> Rc<TileDef> {
        match self.levels[lvl].as_ref().and_then(|l| l.tile_id(x, y)) {
            Some(id) => self.tiles.get_id(id as i32),
            None => self.tiles.get("rock"),
        }
    }

    /// Java `level.setTile(x, y, t, dataVal)`.
    pub fn set_tile(&mut self, lvl: usize, x: i32, y: i32, t: &TileDef, data_val: i32) {
        if let Some(level) = self.levels[lvl].as_mut() {
            level.set_tile_id(x, y, t.id, data_val);
        }
    }

    /// Java `level.setTile(x, y, t)` — uses the tile's default data.
    pub fn set_tile_default(&mut self, lvl: usize, x: i32, y: i32, t: &TileDef) {
        self.set_tile(lvl, x, y, t, tile::dispatch::get_default_data(t));
    }

    /// Java `level.setTile(x, y, "name_data")`.
    pub fn set_tile_named(&mut self, lvl: usize, x: i32, y: i32, tilewithdata: &str) {
        if !tilewithdata.contains('_') {
            let t = self.tiles.get(tilewithdata);
            self.set_tile_default(lvl, x, y, &t);
            return;
        }
        let idx = tilewithdata.find('_').unwrap();
        let name = &tilewithdata[..idx];
        let t = self.tiles.get(name);
        let data = tile::dispatch::get_data_str(&t, &tilewithdata[idx + 1..]);
        self.set_tile(lvl, x, y, &t, data);
    }
}

/* ---------------- entity queries (Java Level methods over the arena) ---------------- */

/// Java `level.getEntitiesInRect(area)` — eids of entities on the level touching area.
pub fn get_entities_in_rect(g: &Game, lvl: usize, area: &Rectangle) -> Vec<i32> {
    g.entities
        .entities_on_level(lvl)
        .filter(|e| e.c.is_touching(area))
        .map(|e| e.c.eid)
        .collect()
}

/// Java `level.getEntitiesInTiles(xt0, yt0, xt1, yt1)`.
pub fn get_entities_in_tiles(
    g: &Game,
    lvl: usize,
    xt0: i32,
    yt0: i32,
    xt1: i32,
    yt1: i32,
) -> Vec<i32> {
    g.entities
        .entities_on_level(lvl)
        .filter(|e| {
            let xt = e.c.x >> 4;
            let yt = e.c.y >> 4;
            xt >= xt0 && xt <= xt1 && yt >= yt0 && yt <= yt1
        })
        .map(|e| e.c.eid)
        .collect()
}

/// Java `level.getPlayers()` — in singleplayer, the player's eid if on this level.
pub fn get_players(g: &Game, lvl: usize) -> Vec<i32> {
    g.entities
        .entities_on_level(lvl)
        .filter(|e| e.is_player())
        .map(|e| e.c.eid)
        .collect()
}

/// Java `level.getClosestPlayer(x, y)`.
pub fn get_closest_player(g: &Game, lvl: usize, x: i32, y: i32) -> Option<i32> {
    let mut best: Option<(i32, i64)> = None;
    for e in g.entities.entities_on_level(lvl).filter(|e| e.is_player()) {
        let xd = (e.c.x - x) as i64;
        let yd = (e.c.y - y) as i64;
        let d = xd * xd + yd * yd;
        if best.is_none() || d < best.unwrap().1 {
            best = Some((e.c.eid, d));
        }
    }
    best.map(|(eid, _)| eid)
}

/// Java `level.getAreaTilePositions(x, y, rx, ry)`.
pub fn get_area_tile_positions(
    g: &Game,
    lvl: usize,
    x: i32,
    y: i32,
    rx: i32,
    ry: i32,
) -> Vec<Point> {
    let level = g.level(lvl);
    let mut local = Vec::new();
    for yp in y - ry..=y + ry {
        for xp in x - rx..=x + rx {
            if xp >= 0 && xp < level.w && yp >= 0 && yp < level.h {
                local.push(Point::new(xp, yp));
            }
        }
    }
    local
}

/// Java `level.getAreaTiles(x, y, rx, ry)`.
pub fn get_area_tiles(g: &Game, lvl: usize, x: i32, y: i32, rx: i32, ry: i32) -> Vec<Rc<TileDef>> {
    get_area_tile_positions(g, lvl, x, y, rx, ry)
        .into_iter()
        .map(|p| g.tile_at(lvl, p.x, p.y))
        .collect()
}

/// Java `level.setAreaTiles(xt, yt, r, tile, data, overwriteStairs)`.
#[allow(clippy::too_many_arguments)]
pub fn set_area_tiles(
    g: &mut Game,
    lvl: usize,
    xt: i32,
    yt: i32,
    r: i32,
    tile: &TileDef,
    data: i32,
    overwrite_stairs: bool,
) {
    for y in yt - r..=yt + r {
        for x in xt - r..=xt + r {
            if overwrite_stairs || !g.tile_at(lvl, x, y).name.to_lowercase().contains("stairs") {
                g.set_tile(lvl, x, y, tile, data);
            }
        }
    }
}

/// Java `level.getMatchingTiles(condition)`.
pub fn get_matching_tiles(
    g: &Game,
    lvl: usize,
    mut condition: impl FnMut(&Game, &TileDef, i32, i32) -> bool,
) -> Vec<Point> {
    let (w, h) = {
        let level = g.level(lvl);
        (level.w, level.h)
    };
    let mut matches = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if condition(g, &g.tile_at(lvl, x, y), x, y) {
                matches.push(Point::new(x, y));
            }
        }
    }
    matches
}

/// Java `level.isLight(x, y)`.
pub fn is_light(g: &Game, lvl: usize, x: i32, y: i32) -> bool {
    get_area_tiles(g, lvl, x, y, 3, 3)
        .iter()
        .any(|t| matches!(t.kind, tile::TileKind::Torch { .. }))
}

/// Java `level.dropItem(x, y, item)`.
pub fn drop_item(g: &mut Game, lvl: usize, x: i32, y: i32, item: crate::item::Item) -> i32 {
    let (mut ranx, mut rany);
    loop {
        let level = g.level_mut(lvl);
        ranx = x + level.random.next_int_bound(11) - 5;
        rany = y + level.random.next_int_bound(11) - 5;
        if ranx >> 4 == x >> 4 && rany >> 4 == y >> 4 {
            break;
        }
    }
    let ie = crate::entity::item_entity::new(item, ranx, rany, &mut g.random);
    let eid_tmp = ie.c.eid;
    g.level_mut(lvl).add(ie, lvl);
    eid_tmp
}

/// Java `level.dropItem(x, y, mincount, maxcount, items...)`.
pub fn drop_items_counted(
    g: &mut Game,
    lvl: usize,
    x: i32,
    y: i32,
    mincount: i32,
    maxcount: i32,
    items: &[crate::item::Item],
) {
    let count = mincount
        + g.level_mut(lvl)
            .random
            .next_int_bound(maxcount - mincount + 1);
    for _ in 0..count {
        for item in items {
            drop_item(g, lvl, x, y, item.clone());
        }
    }
}

/// Java `level.clearEntities()` (offline path).
pub fn clear_entities(g: &mut Game, lvl: usize) {
    let ids = g.entities.ids_on_level(lvl);
    for eid in ids {
        if eid != g.player_id {
            g.entities.delete(eid);
        } else if let Some(p) = g.entities.get_mut(eid) {
            p.c.level = None;
        }
    }
}

/// Java `level.removeAllEnemies()`.
pub fn remove_all_enemies(g: &mut Game, lvl: usize) {
    let creative = g.is_mode("creative");
    let ids: Vec<i32> = g
        .entities
        .entities_on_level(lvl)
        .filter(|e| e.is_enemy_mob() && (!matches!(e.kind, EntityKind::AirWizard(_)) || creative))
        .map(|e| e.c.eid)
        .collect();
    for eid in ids {
        // Java calls e.remove(), which queues level removal
        if let Some(e) = g.entities.get_mut(eid) {
            e.c.removed = true;
        }
        g.level_mut(lvl).remove(eid);
    }
}

/* ------------------------------- Level.tick + spawn ------------------------------- */

/// Java `Level.updateVisible()`.
pub fn update_visible(g: &mut Game, lvl: usize) {
    let Some(player) = g.try_player() else { return };
    let px = player.c.x / crate::gfx::sprite_sheet::TILE_SIZE;
    let py = player.c.y / crate::gfx::sprite_sheet::TILE_SIZE;
    let view_size = 4;
    if g.level(lvl).is_infinite() {
        let level = g.level_mut(lvl);
        let chunks = level.chunks.as_mut().expect("infinite");
        for yy in py - view_size..py + view_size {
            let yd = (yy - py) * (yy - py);
            for xx in px - view_size..px + view_size {
                let xd = xx - px;
                if xd * xd + yd <= view_size * view_size {
                    chunks.mark_visible(xx, yy);
                }
            }
        }
        return;
    }
    let level = g.level_mut(lvl);
    let x0 = (px - view_size).max(0);
    let y0 = (py - view_size).max(0);
    let x1 = (px + view_size).min(level.w);
    let y1 = (py + view_size).min(level.h);
    for yy in y0..y1 {
        let yd = (yy - py) * (yy - py);
        for xx in x0..x1 {
            let xd = xx - px;
            let dist = xd * xd + yd;
            if dist <= view_size * view_size {
                level.visible[(xx + yy * level.w) as usize] = true;
            }
        }
    }
}

/// Java `Level.tick(fullTick)`.
pub fn tick_level(g: &mut Game, lvl: usize, full_tick: bool) {
    let mut count = 0;

    update_visible(g, lvl);

    // drain entitiesToAdd into the arena
    while let Some(entity) = {
        let level = g.level_mut(lvl);
        if level.entities_to_add.is_empty() {
            None
        } else {
            Some(level.entities_to_add.remove(0))
        }
    } {
        let mut random = g.random.clone();
        g.entities.insert(entity, &mut random);
        g.random = random;
    }

    if full_tick {
        // random tile ticks
        {
            let (w, h) = {
                let level = g.level(lvl);
                (level.w, level.h)
            };
            for _ in 0..w * h / 50 {
                let (xt, yt) = {
                    let level = g.level_mut(lvl);
                    // JAVA: yt also uses nextInt(w) — preserved quirk
                    (
                        level.random.next_int_bound(w),
                        level.random.next_int_bound(w),
                    )
                };
                let tile = g.tile_at(lvl, xt, yt);
                tile::dispatch::tick(g, &tile, lvl, xt, yt);
            }
        }

        // entity loop
        let ids = g.entities.ids_on_level(lvl);
        for eid in &ids {
            let Some(e) = g.entities.get(*eid) else {
                continue;
            };
            if e.c.removed {
                continue;
            }

            if *eid != g.player_id {
                // the main entity tick call (player is ticked separately by the Updater)
                g.with_entity(*eid, |e, g| crate::entity::behavior::entity_tick(g, e));
            }

            let Some(e) = g.entities.get(*eid) else {
                continue;
            };
            if e.c.removed {
                continue;
            }
            if e.is_mob() {
                count += 1;
            }
        }

        for eid in &ids {
            let Some(e) = g.entities.get(*eid) else {
                continue;
            };
            if e.c.removed || e.c.level != Some(lvl) {
                g.level_mut(lvl).remove(*eid);
            }
        }
    }

    // mob cap enforcement: remove random MobAi's while over the cap
    while count > g.level(lvl).max_mob_count {
        let ids = g.entities.ids_on_level(lvl);
        if ids.is_empty() {
            break;
        }
        let pick = ids[g.level_mut(lvl).random.next_int_bound(ids.len() as i32) as usize];
        let is_mob_ai = g.entities.get(pick).map(|e| e.is_mob_ai()).unwrap_or(false);
        if is_mob_ai {
            if let Some(e) = g.entities.get_mut(pick) {
                e.c.removed = true;
            }
            g.level_mut(lvl).remove(pick);
            count -= 1;
        }
    }

    // drain entitiesToRemove
    while let Some(eid) = {
        let level = g.level_mut(lvl);
        if level.entities_to_remove.is_empty() {
            None
        } else {
            Some(level.entities_to_remove.remove(0))
        }
    } {
        if eid == g.player_id {
            // the player object persists (Java kept the Game.player reference)
            if let Some(p) = g.entities.get_mut(eid) {
                if p.c.level == Some(lvl) {
                    p.c.level = None;
                }
            }
        } else if g
            .entities
            .get(eid)
            .map(|e| e.c.level == Some(lvl) || e.c.removed)
            .unwrap_or(false)
        {
            g.entities.delete(eid);
        }
    }

    g.level_mut(lvl).mob_count = count;

    if full_tick && count < g.level(lvl).max_mob_count {
        try_spawn(g, lvl);
    }
}

/// Java `Level.trySpawn()`.
pub fn try_spawn(g: &mut Game, lvl: usize) {
    let (mob_count, max_mob_count, depth, w, h) = {
        let level = g.level(lvl);
        (
            level.mob_count,
            level.max_mob_count,
            level.depth,
            level.w,
            level.h,
        )
    };
    let spawn_skip_chance = (MOB_SPAWN_FACTOR as f64 * (mob_count as f64).powi(2)
        / (max_mob_count as f64).powi(2)) as i32;
    if spawn_skip_chance > 0 && g.level_mut(lvl).random.next_int_bound(spawn_skip_chance) != 0 {
        return; // hopefully will make mobs spawn a lot slower
    }

    let mut spawned = false;
    for _ in 0..30 {
        if spawned {
            break;
        }
        let mut min_level = 1;
        let mut max_level = 1;
        if depth < 0 {
            max_level = (-depth) + (if g.random.next_double() > 0.75 { 1 } else { 0 });
        }
        if depth > 0 {
            min_level = 4;
            max_level = 4;
        }

        let (mlvl, rnd, nx, ny) = {
            // infinite layers spawn within the loaded area around the player
            let (px, py) = match g.try_player() {
                Some(p) if g.level(lvl).is_infinite() => (p.c.x >> 4, p.c.y >> 4),
                _ => (0, 0),
            };
            let infinite = g.level(lvl).is_infinite();
            let level = g.level_mut(lvl);
            let mlvl = level.random.next_int_bound(max_level - min_level + 1) + min_level;
            let rnd = level.random.next_int_bound(100);
            let span = chunk::CHUNK_SIZE * chunk::LOAD_RADIUS * 2;
            let (nx, ny) = if infinite {
                (
                    (px - span / 2 + level.random.next_int_bound(span)) * 16 + 8,
                    (py - span / 2 + level.random.next_int_bound(span)) * 16 + 8,
                )
            } else {
                (
                    level.random.next_int_bound(w) * 16 + 8,
                    level.random.next_int_bound(h) * 16 + 8,
                )
            };
            (mlvl, rnd, nx, ny)
        };

        // enemy mobs; first condition prevents enemy spawn on surface on day 1
        if (g.get_time() == crate::core::updater::Time::Night && g.past_day1 || depth != 0)
            && crate::entity::behavior::enemy_check_start_pos(g, lvl, nx, ny)
        {
            // JAVA (this fork): only Zombies and Snakes spawn naturally
            if rnd <= 40 {
                let e = crate::entity::mob::zombie::new(g, mlvl);
                g.level_mut(lvl).add_at(e, nx, ny, false, lvl);
            } else if rnd <= 75 {
                let e = crate::entity::mob::snake::new(g, mlvl);
                g.level_mut(lvl).add_at(e, nx, ny, false, lvl);
            }
            spawned = true;
        }

        if depth == 0 && crate::entity::behavior::passive_check_start_pos(g, lvl, nx, ny) {
            // spawns the friendly mobs
            let night = g.get_time() == crate::core::updater::Time::Night;
            if rnd <= (if night { 22 } else { 33 }) {
                let e = crate::entity::mob::cow::new(g);
                g.level_mut(lvl).add_at(e, nx, ny, false, lvl);
            } else if rnd >= 68 {
                let e = crate::entity::mob::pig::new(g);
                g.level_mut(lvl).add_at(e, nx, ny, false, lvl);
            } else {
                let e = crate::entity::mob::sheep::new(g);
                g.level_mut(lvl).add_at(e, nx, ny, false, lvl);
            }

            // JAVA: also always adds a GlowWorm (at its default position)
            let e = crate::entity::mob::glow_worm::new(g);
            g.level_mut(lvl).add(e, lvl);

            spawned = true;
        }
    }
}

/* --------------------------------- Level rendering --------------------------------- */

/// Java `Level.renderBackground(screen, xScroll, yScroll)`.
pub fn render_background(
    g: &mut Game,
    screen: &mut crate::gfx::Screen,
    lvl: usize,
    x_scroll: i32,
    y_scroll: i32,
) {
    let xo = x_scroll >> 4;
    let yo = y_scroll >> 4;
    let w = crate::gfx::screen::W >> 4;
    let h = crate::gfx::screen::H >> 4;
    screen.set_offset(x_scroll, y_scroll);
    for y in yo..=h + yo {
        for x in xo..=w + xo {
            let tile = g.tile_at(lvl, x, y);
            tile::dispatch::render(g, screen, &tile, lvl, x, y);
        }
    }
    screen.set_offset(0, 0);
}

/// Java `Level.renderSprites(screen, xScroll, yScroll)` — y-sorted entity rendering.
pub fn render_sprites(
    g: &mut Game,
    screen: &mut crate::gfx::Screen,
    lvl: usize,
    x_scroll: i32,
    y_scroll: i32,
) {
    let xo = x_scroll >> 4;
    let yo = y_scroll >> 4;
    let w = (crate::gfx::screen::W + 15) >> 4;
    let h = (crate::gfx::screen::H + 15) >> 4;

    screen.set_offset(x_scroll, y_scroll);

    let mut ids: Vec<(i32, i32)> = get_entities_in_tiles(g, lvl, xo, yo, xo + w, yo + h)
        .into_iter()
        .filter_map(|eid| g.entities.get(eid).map(|e| (e.c.y, eid)))
        .collect();
    ids.sort_by_key(|(y, _)| *y); // Java spriteSorter
    for (_, eid) in ids {
        let (on_level, removed) = match g.entities.get(eid) {
            Some(e) => (e.c.level == Some(lvl), e.c.removed),
            None => continue,
        };
        if on_level && !removed {
            g.with_entity(eid, |e, g| {
                crate::entity::behavior::entity_render(g, screen, e)
            });
        } else {
            g.level_mut(lvl).remove(eid);
        }
    }

    screen.set_offset(0, 0);
}

/// Java `Level.renderLight(screen, xScroll, yScroll, brightness)`.
pub fn render_light(
    g: &mut Game,
    screen: &mut crate::gfx::Screen,
    lvl: usize,
    x_scroll: i32,
    y_scroll: i32,
    brightness: i32,
) {
    let xo = x_scroll >> 4;
    let yo = y_scroll >> 4;
    let w = (crate::gfx::screen::W + 15) >> 4;
    let h = (crate::gfx::screen::H + 15) >> 4;

    screen.set_offset(x_scroll, y_scroll);
    let r = 4;

    let ids = get_entities_in_tiles(g, lvl, xo - r, yo - r, w + xo + r, h + yo + r);
    for eid in ids {
        let Some(e) = g.entities.get(eid) else {
            continue;
        };
        let lr = crate::entity::behavior::get_light_radius(e);
        if lr > 0 {
            screen.render_light(e.c.x - 1, e.c.y - 4, lr * brightness);
        }
    }

    let (lw, lh) = {
        let level = g.level(lvl);
        (level.w, level.h)
    };
    for y in yo - r..=h + yo + r {
        for x in xo - r..=w + xo + r {
            if x < 0 || y < 0 || x >= lw || y >= lh {
                continue;
            }
            let tile = g.tile_at(lvl, x, y);
            let lr = tile::dispatch::get_light_radius(g, &tile, lvl, x, y);
            if lr > 0 {
                screen.render_light(x * 16 + 8, y * 16 + 8, lr * brightness);
            }
        }
    }
    screen.set_offset(0, 0);
}

/* ------------------------------- infinite-world support ------------------------------- */

/// Keep the chunks around the player generated, and drop clean far-away chunks.
/// (Dirty chunks stay resident until the world saves them.)
pub fn ensure_chunks(g: &mut Game, lvl: usize) {
    let Some(player) = g.try_player() else { return };
    if player.c.level != Some(lvl) {
        return;
    }
    let (px, py) = (player.c.x, player.c.y);
    ensure_chunks_at(g, lvl, px >> 4, py >> 4);
}

/// Same as [`ensure_chunks`] but around an arbitrary tile position (title flyover camera).
pub fn ensure_chunks_at(g: &mut Game, lvl: usize, tile_x: i32, tile_y: i32) {
    if !g.levels[lvl]
        .as_ref()
        .map(|l| l.is_infinite())
        .unwrap_or(false)
    {
        return;
    }
    let pcx = chunk::chunk_coord(tile_x);
    let pcy = chunk::chunk_coord(tile_y);
    let seed = g.world_seed;
    let depth = g.level(lvl).depth;

    // generate (or load from disk) the ring around the player
    let mut to_generate = Vec::new();
    {
        let level = g.level(lvl);
        let chunks = level.chunks.as_ref().expect("checked infinite");
        for cy in pcy - chunk::LOAD_RADIUS..=pcy + chunk::LOAD_RADIUS {
            for cx in pcx - chunk::LOAD_RADIUS..=pcx + chunk::LOAD_RADIUS {
                if !chunks.is_loaded(cx, cy) {
                    to_generate.push((cx, cy));
                }
            }
        }
    }
    for (cx, cy) in to_generate {
        let chunk = match crate::saveload::save::load_chunk(g, depth, cx, cy) {
            Some(c) => c,
            None => infinite_gen::generate_chunk(seed, depth, cx, cy, &g.tiles),
        };
        g.level_mut(lvl)
            .chunks
            .as_mut()
            .expect("checked infinite")
            .insert(cx, cy, chunk);
    }

    // unload far chunks (persist dirty ones to disk first)
    let far: Vec<(i32, i32)> = {
        let chunks = g.level(lvl).chunks.as_ref().expect("checked infinite");
        chunks
            .loaded_coords()
            .into_iter()
            .filter(|(cx, cy)| {
                (cx - pcx).abs() > chunk::UNLOAD_RADIUS || (cy - pcy).abs() > chunk::UNLOAD_RADIUS
            })
            .collect()
    };
    for (cx, cy) in far {
        let removed = g
            .level_mut(lvl)
            .chunks
            .as_mut()
            .expect("checked infinite")
            .remove(cx, cy);
        if let Some(chunk) = removed {
            if chunk.dirty {
                crate::saveload::save::save_chunk(g, depth, cx, cy, &chunk);
            }
        }
    }
}
