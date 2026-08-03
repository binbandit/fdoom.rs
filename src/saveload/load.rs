//! Port of `fdoom.saveload.Load`.
//!
//! Java's constructor overloads become free functions: `new Load(worldname)` →
//! [`load_world_named`] (with [`new_world`] as the `Load(worldname, loadGame)` form),
//! `new Load(true)` (the startup global-prefs load) → [`load_prefs`], and
//! `new Load(worldVersion)` → [`Load::with_version`].
//!
//! `LegacyLoad` (pre-1.9.2 saves) is NOT ported: loading such a world prints an error and
//! leaves the world unloaded. The two `LegacyLoad` pieces the modern path still needs are
//! kept here: `Tiles.oldids` (as [`OLD_IDS`], used for 1.9.2..1.9.4-dev6 numeric tile ids)
//! and `updateUnlocks` (the old `unlocks` → `Unlocks` file migration).
//! Java's `Load.loadFile` (a classpath-resource reader) has no equivalent here — resources
//! are embedded via `include_bytes!` in this port.

use std::io::Write as _;
use std::path::Path;

use crate::core::game::Game;
use crate::entity::mob::player::{WearSlot, wear_slot_for};
use crate::entity::{Entity, EntityKind};
use crate::item::Inventory;
use crate::level::Level;
use crate::saveload::save::EXTENSION;
use crate::saveload::version::Version;

/// Java `Tiles.oldids` — the pre-1.9.4-dev6 numeric tile id table (from `Tiles.java`'s
/// static block). Unlisted ids were `null` in Java.
pub const OLD_IDS: &[(i32, &str)] = &[
    (0, "grass"),
    (1, "rock"),
    (2, "water"),
    (3, "flower"),
    (4, "tree"),
    (5, "dirt"),
    (41, "wool"),
    (42, "red wool"),
    (43, "blue wool"),
    (45, "green wool"),
    (127, "yellow wool"),
    (56, "black wool"),
    (6, "sand"),
    (7, "cactus"),
    (8, "hole"),
    (9, "tree Sapling"),
    (10, "cactus Sapling"),
    (11, "farmland"),
    (12, "wheat"),
    (13, "lava"),
    (14, "stairs Down"),
    (15, "stairs Up"),
    (17, "cloud"),
    (30, "explode"),
    (31, "Wood Planks"),
    (33, "plank wall"),
    (34, "stone wall"),
    (35, "wood door"),
    (36, "wood door"),
    (37, "stone door"),
    (38, "stone door"),
    (39, "lava brick"),
    (32, "Stone Bricks"),
    (120, "Obsidian"),
    (121, "Obsidian wall"),
    (122, "Obsidian door"),
    (123, "Obsidian door"),
    (18, "hard Rock"),
    (19, "iron Ore"),
    (24, "Lapis"),
    (20, "gold Ore"),
    (21, "gem Ore"),
    (22, "cloud Cactus"),
    (16, "infinite Fall"),
    // light/torch versions, for compatibility with before 1.9.4-dev3.
    (100, "grass"),
    (101, "sand"),
    (102, "tree"),
    (103, "cactus"),
    (104, "water"),
    (105, "dirt"),
    (107, "flower"),
    (108, "stairs Up"),
    (109, "stairs Down"),
    (110, "Wood Planks"),
    (111, "Stone Bricks"),
    (112, "wood door"),
    (113, "wood door"),
    (114, "stone door"),
    (115, "stone door"),
    (116, "Obsidian door"),
    (117, "Obsidian door"),
    (119, "hole"),
    (57, "wool"),
    (58, "red wool"),
    (59, "blue wool"),
    (60, "green wool"),
    (61, "yellow wool"),
    (62, "black wool"),
    (63, "Obsidian"),
    (64, "tree Sapling"),
    (65, "cactus Sapling"),
    (44, "torch grass"),
    (40, "torch sand"),
    (46, "torch dirt"),
    (47, "torch wood planks"),
    (48, "torch stone bricks"),
    (49, "torch Obsidian"),
    (50, "torch wool"),
    (51, "torch red wool"),
    (52, "torch blue wool"),
    (53, "torch green wool"),
    (54, "torch yellow wool"),
    (55, "torch black wool"),
];

fn old_id(id: i32) -> Option<&'static str> {
    OLD_IDS
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, name)| *name)
}

/// Java `String.split(sep)` semantics: keeps interior empty strings, drops all trailing
/// empty strings, and returns `[s]` when the separator does not occur.
fn java_split(s: &str, sep: char) -> Vec<String> {
    if !s.contains(sep) {
        return vec![s.to_string()];
    }
    let mut parts: Vec<String> = s.split(sep).map(String::from).collect();
    while parts.last().map(|p| p.is_empty()).unwrap_or(false) {
        parts.pop();
    }
    parts
}

/// Java `Boolean.parseBoolean`.
fn parse_bool(s: &str) -> bool {
    s.eq_ignore_ascii_case("true")
}

/* ---------------------------- tolerant field readers -----------------------------
 *
 * Hard convention 10: a save that is truncated, hand-edited, written by a future
 * build, or corrupted mid-write degrades with a warning — it never panics. Every
 * field read below goes through these, so a short file simply runs out of fields
 * and the rest take their defaults.
 */

/// Take the next field, or `""` once the save has run out.
fn pop(data: &mut Vec<String>) -> String {
    if data.is_empty() {
        String::new()
    } else {
        data.remove(0)
    }
}

/// Take the next field as an `i32`; missing or unparseable reads as `default`.
fn pop_i32(data: &mut Vec<String>, default: i32) -> i32 {
    parse_i32(&pop(data), default)
}

/// Field `idx`, or `""` when the save is short.
fn at(data: &[String], idx: usize) -> &str {
    data.get(idx).map(|s| s.as_str()).unwrap_or("")
}

/// Field `idx` as an `i32`; missing or unparseable reads as `default`.
fn at_i32(data: &[String], idx: usize, default: i32) -> i32 {
    parse_i32(at(data, idx), default)
}

/// `i32::from_str` that survives junk, blanks and out-of-range magnitudes (a value
/// too big for an `i32` saturates rather than throwing the save away).
fn parse_i32(s: &str, default: i32) -> i32 {
    let s = s.trim();
    match s.parse::<i32>() {
        Ok(v) => v,
        Err(_) => match s.parse::<i128>() {
            Ok(v) => {
                let clamped = v.clamp(i32::MIN as i128, i32::MAX as i128) as i32;
                crate::log_warn!("save field {s:?} is out of range; clamped to {clamped}");
                clamped
            }
            Err(_) => {
                // Substituting a default silently is how corrupt data turns into a
                // mystery bug report; say what was wrong and what replaced it.
                if !s.is_empty() {
                    crate::log_warn!("save field {s:?} is not a number; using {default}");
                }
                default
            }
        },
    }
}

/// Java `Enum.valueOf(PotionType.class, name)`. Unknown names (a removed or
/// future potion) drop the effect with a warning instead of throwing.
fn potion_type_from_name(name: &str) -> Option<crate::item::PotionType> {
    crate::item::PotionType::VALUES
        .iter()
        .copied()
        .find(|p| p.enum_name() == name)
}

/// Java `Level.printLevelLoc(prefix, x, y)`.
fn print_level_loc(g: &Game, lvl: usize, prefix: &str, x: i32, y: i32) {
    let level_name = crate::level::get_level_name(g.level(lvl).depth);
    crate::log_info!("{prefix} on {level_name} level ({x},{y})");
}

/// The state of Java's `Load` object.
pub struct Load {
    location: String,
    percent_inc: f32,
    data: Vec<String>,
    extradata: Vec<String>,
    world_ver: Option<Version>,
    has_global_prefs: bool,
    /// The save could not be read at all (missing/damaged `Game` file, or a world
    /// shape this build cannot load). The world is left untouched on disk.
    failed: bool,
}

/// Java `new Load(worldname)` — loads the whole world. Returns whether the world
/// actually came up; `false` means the save was unreadable and the caller must not
/// drop the player into gameplay (see `core::world::init_world`).
pub fn load_world_named(g: &mut Game, world_name: &str) -> bool {
    !new_world(g, world_name, true).failed
}

/// Alias of [`load_world_named`] (call-site name used by `core::world::init_world`).
pub fn load_world(g: &mut Game, world_name: &str) -> bool {
    load_world_named(g, world_name)
}

/// Java `new Load(worldname, loadGame)`.
pub fn new_world(g: &mut Game, worldname: &str, load_game: bool) -> Load {
    let mut l = Load::init(g);

    let game_file = format!("{}/saves/{}/Game{}", l.location, worldname, EXTENSION);
    l.load_from_file(g, &game_file);
    // A missing/empty/unreadable Game file means there is no world to read here: the
    // save was deleted, is being written, or the disk lost it. Refuse the load rather
    // than half-building a world on top of the player's save.
    let game_file_readable = !l.data.is_empty();
    if at(&l.data, 0).contains('.') {
        l.world_ver = Some(Version::new(at(&l.data, 0)));
    }
    if l.world_ver.is_none() {
        l.world_ver = Some(Version::new("1.8"));
    }

    if !l.has_global_prefs {
        l.has_global_prefs = *l.wv() >= Version::new("1.9.2");
    }

    if !load_game {
        return l;
    }

    if !game_file_readable {
        crate::log_warn!(
            "LOAD ERROR: world \"{worldname}\" has no readable Game{EXTENSION}; refusing to load (the save is missing or damaged — nothing was overwritten)."
        );
        l.failed = true;
    } else if *l.wv() < Version::new("3.0") {
        // Pre-3.0 worlds have six levels (sky included) and Score-mode state; the
        // sandbox pivot changed the world shape, so they can't be loaded.
        crate::log_warn!(
            "LOAD ERROR: world \"{}\" was saved by version {}; worlds from before 3.0 (the sandbox pivot) are not supported.",
            worldname,
            l.wv()
        );
        l.failed = true;
    } else {
        l.location.push_str(&format!("/saves/{worldname}/"));

        // for the methods below, and world.
        l.percent_inc = 5.0 + g.levels.len() as f32 - 1.0;
        l.percent_inc = 100.0 / l.percent_inc;

        g.loading_percentage = 0.0; // Java LoadingDisplay.setPercentage(0)
        l.load_game(g, "Game"); // more of the version will be determined here
        l.load_world(g, "Level");
        l.load_entities(g, "Entities");
        l.load_inventory_file(g, "Inventory");
        l.load_player_file(g, "Player");
        if g.is_mode("creative") {
            fill_creative_inv_on_player(g, false);
        }
    }

    l
}

/// Install a layer built from scratch: a chunked layer at the infinite depths, a
/// freshly generated map otherwise.
///
/// Used for the world's normal chunked layers, and — loudly — for any layer whose
/// save file turned out to be missing or damaged. Losing one layer must not cost the
/// player the other four.
fn rebuild_layer(g: &mut Game, depth: i32) {
    let idx = crate::level::lvl_idx(depth);
    if crate::level::is_infinite_depth(depth) {
        let mut level = Level::empty(
            g.world_size,
            g.world_size,
            depth,
            g.settings.get_idx("diff"),
        );
        level.chunks = Some(crate::level::chunk::ChunkMap::default());
        level.reseed(g.world_seed);
        g.levels[idx] = Some(level);
        return;
    }

    g.levels[idx] = Some(crate::core::world::generate_level(g, depth));
    if let Some(l) = g.levels[idx].as_mut() {
        l.reseed(g.world_seed);
    }
    if depth == crate::level::MIN_LEVEL_DEPTH {
        // the dungeon needs its landing gate, or players arriving from the deep mines
        // materialize inside solid obsidian (init_world stamps the same one)
        let (cx, cy) = (g.level(idx).w / 2, g.level(idx).h / 2);
        let stairs_up = g.tiles.get("Stairs Up");
        g.set_tile_default(idx, cx, cy, &stairs_up);
        crate::level::structure::draw_dungeon_gate(g, idx, cx, cy);
    }
}

/// Java `new Load(true)` — the startup load of `Preferences` + `Unlocks`.
pub fn load_prefs(g: &mut Game) {
    let mut l = Load::init(g);
    l.location.push('/');

    if l.has_global_prefs {
        l.load_prefs(g, "Preferences");
    } else {
        crate::saveload::save::save_prefs(g); // Java `new Save()`
    }

    let test_file_old = format!("{}unlocks{}", l.location, EXTENSION);
    let test_file = format!("{}Unlocks{}", l.location, EXTENSION);
    if Path::new(&test_file_old).exists() && !Path::new(&test_file).exists() {
        let _ = std::fs::rename(&test_file_old, &test_file);
        l.legacy_update_unlocks(g, &test_file);
    } else if !Path::new(&test_file).exists() {
        if let Err(ex) = std::fs::File::create(&test_file) {
            crate::log_warn!("could not create Unlocks{EXTENSION}:");
            crate::log_warn!("{ex}");
        }
    }

    l.load_unlocks(g, "Unlocks");
}

/// Java `Items.fillCreativeInv(Game.player.getInventory(), false)` at the end of a world
/// load. The freshly loaded player sits in the level's entitiesToAdd queue (Java kept a
/// direct reference; we reach into the queue).
fn fill_creative_inv_on_player(g: &mut Game, add_all: bool) {
    fn fill(g: &Game, p: &mut Entity, add_all: bool) {
        let mut inv = std::mem::take(&mut p.player_mut().inventory);
        inv.creative = g.is_mode("creative");
        crate::item::registry::fill_creative_inv(g, &mut inv, add_all);
        p.player_mut().inventory = inv;
    }

    let cur = g.current_level;
    let queued = g.levels[cur].as_ref().and_then(|l| {
        l.entities_to_add
            .iter()
            .position(|e| e.c.eid == g.player_id)
    });
    if let Some(idx) = queued {
        let mut p = g.level_mut(cur).entities_to_add.remove(idx);
        fill(g, &mut p, add_all);
        g.level_mut(cur).entities_to_add.insert(idx, p);
    } else if let Some(mut p) = g.entities.take(g.player_id) {
        fill(g, &mut p, add_all);
        g.entities.put_back(p);
    }
}

impl Load {
    /// The Java instance-initializer block.
    fn init(g: &Game) -> Load {
        let location = format!("{}", g.game_dir.display());
        let test_file = format!("{location}/Preferences{EXTENSION}");
        Load {
            location,
            percent_inc: 0.0,
            data: Vec::new(),
            extradata: Vec::new(),
            world_ver: None,
            has_global_prefs: Path::new(&test_file).exists(),
            failed: false,
        }
    }

    /// Whether the world refused to load (see [`Load::failed`]).
    pub fn has_failed(&self) -> bool {
        self.failed
    }

    /// Java `new Load(worldVersion)` — a Load object for parsing data of a known version
    /// (no file IO).
    pub fn with_version(g: &Game, world_version: Version) -> Load {
        let mut l = Load::init(g); // Java this(false)
        l.world_ver = Some(world_version);
        l
    }

    /// Java `getWorldVersion()`.
    pub fn get_world_version(&self) -> Option<&Version> {
        self.world_ver.as_ref()
    }

    fn wv(&self) -> &Version {
        self.world_ver
            .as_ref()
            .expect("world version not determined")
    }

    /// Java `loadFromFile(String filename)` (the instance method).
    fn load_from_file(&mut self, g: &mut Game, filename: &str) {
        self.data.clear();
        self.extradata.clear();

        match load_from_file_str(filename, true) {
            Ok(total) => {
                if !total.is_empty() {
                    self.data.extend(java_split(&total, ','));
                }
            }
            Err(ex) => crate::log_warn!("{ex}"), // Java ex.printStackTrace()
        }

        if filename.contains("Level") {
            // keep "LevelN" (7 chars past the last slash), then append "data": the
            // companion tile-data file sits beside the tile file
            let cut = filename.rfind('/').map(|i| i + 7).unwrap_or(filename.len());
            let datafile = format!("{}data{}", &filename[..cut], EXTENSION);
            match load_from_file_str(&datafile, true) {
                Ok(total) => self.extradata.extend(java_split(&total, ',')),
                Err(ex) => crate::log_warn!("{ex}"),
            }
        }

        // Java LoadingDisplay.progress(percentInc).
        g.loading_percentage = (g.loading_percentage + self.percent_inc).min(100.0);
    }

    /// Port of `LegacyLoad.updateUnlocks` — the one LegacyLoad piece still reachable
    /// (migrating an old lowercase `unlocks` file).
    fn legacy_update_unlocks(&mut self, g: &mut Game, path: &str) {
        self.data.clear();
        self.extradata.clear();
        match load_from_file_str(path, true) {
            Ok(total) => self.data.extend(java_split(&total, ',')),
            Err(ex) => crate::log_warn!("{ex}"),
        }
        g.loading_percentage = (g.loading_percentage + 13.0).min(100.0);

        let mut i = 0;
        while i < self.data.len() {
            if self.data[i].is_empty() {
                self.data.remove(i);
                continue;
            }
            self.data[i] = self.data[i]
                .replace("HOURMODE", "H_ScoreTime")
                .replace("MINUTEMODE", "M_ScoreTime");
            i += 1;
        }

        let _ = std::fs::remove_file(path);

        match std::fs::File::create(path) {
            Ok(mut writer) => {
                for unlock in &self.data {
                    let _ = write!(writer, ",{unlock}");
                }
            }
            Err(ex) => crate::log_warn!("{ex}"),
        }
    }

    /// Java `loadUnlocks(filename)`.
    fn load_unlocks(&mut self, g: &mut Game, filename: &str) {
        let file = format!("{}{}{}", self.location, filename, EXTENSION);
        self.load_from_file(g, &file);

        for unlock in &self.data {
            if unlock == "AirSkin" {
                g.settings.set("unlockedskin", true);
            }

            let unlock = unlock
                .replace("HOURMODE", "H_ScoreTime")
                .replace("MINUTEMODE", "M_ScoreTime")
                .replace("M_ScoreTime", "_ScoreTime")
                .replace("2H_ScoreTime", "120_ScoreTime");

            // legacy "<n>_ScoreTime" unlocks are ignored (Score mode was removed)
            let _ = unlock;
        }
    }

    /// Java `loadGame(filename)`.
    fn load_game(&mut self, g: &mut Game, filename: &str) {
        let file = format!("{}{}{}", self.location, filename, EXTENSION);
        self.load_from_file(g, &file);

        if self.data.len() < 3 {
            crate::log_warn!(
                "Game file is short ({} field(s)); world settings fall back to defaults",
                self.data.len()
            );
        }
        self.world_ver = Some(Version::new(&pop(&mut self.data))); // gets the world version
        if *self.wv() >= Version::new("2.0.4-dev8") {
            let modedata = pop(&mut self.data);
            self.load_mode(g, &modedata);
        }

        g.set_time(pop_i32(&mut self.data, 0));

        g.game_time = pop_i32(&mut self.data, 0);
        if *self.wv() >= Version::new("1.9.3-dev2") {
            g.past_day1 = g.game_time > 65000;
        } else {
            g.game_time = 65000; // prevents time cheating.
        }

        // a truncated Game file keeps the current difficulty rather than dropping to 0
        let mut diff_idx = pop_i32(&mut self.data, g.settings.get_idx("diff"));
        if *self.wv() < Version::new("1.9.3-dev3") {
            diff_idx -= 1; // account for change in difficulty
        }

        g.settings.set_idx("diff", diff_idx);

        g.air_wizard_beaten = parse_bool(&pop(&mut self.data));
    }

    /// Java `loadMode(modedata)` — Score mode was removed in the sandbox pivot; a
    /// score-mode payload (mode 3) falls back to Survival.
    fn load_mode(&self, g: &mut Game, modedata: &str) {
        let raw = modedata.split(';').next().unwrap_or(modedata);
        let mode: i32 = raw.parse().unwrap_or(0);
        g.settings.set_idx("mode", if mode == 3 { 0 } else { mode });
    }

    /// Java `loadPrefs(filename)`.
    fn load_prefs(&mut self, g: &mut Game, filename: &str) {
        let file = format!("{}{}{}", self.location, filename, EXTENSION);
        self.load_from_file(g, &file);

        // the default, b/c this doesn't really matter much being specific past this if
        // it's not set below.
        let mut pref_ver = Version::new("2.0.2");

        // A Preferences file truncated by a crash mid-write would otherwise take the
        // game down on every launch; every read below tolerates a short file and
        // falls back to the built-in default.
        if self.data.len() < 3 {
            crate::log_warn!(
                "preferences file is short ({} field(s)); falling back to defaults",
                self.data.len()
            );
        }
        if !at(&self.data, 2).contains(';') {
            // signifies that this file was last written to by a version after 2.0.2.
            pref_ver = Version::new(&pop(&mut self.data));
        }

        g.settings.set("sound", parse_bool(&pop(&mut self.data)));
        g.settings.set("autosave", parse_bool(&pop(&mut self.data)));

        if pref_ver >= Version::new("2.0.4-dev2") {
            let fps = pop_i32(&mut self.data, g.settings.get("fps").as_int());
            g.settings.set("fps", fps);
        }

        let subdata: Vec<String> = if pref_ver < Version::new("2.0.3-dev1") {
            // pre-2.0.3 prefs are all keymap from here on (and carry no daycycle field)
            std::mem::take(&mut self.data)
        } else {
            // discard the reserved multiplayer fields (IP/UUID/username slots)
            let _saved_ip = pop(&mut self.data);
            if pref_ver > Version::new("2.0.3-dev3") {
                let _saved_uuid = pop(&mut self.data);
                let _saved_username = pop(&mut self.data);
            }

            if pref_ver >= Version::new("2.0.4-dev3") {
                let lang = pop(&mut self.data);
                if !lang.is_empty() {
                    g.settings.set("language", lang.clone());
                    g.localization.change_language(&lang);
                }
            }

            // consume the keymap field, so the appended daycycle field below is read
            // from its own slot (it used to re-read the keymap and silently drop the
            // player's day-cycle setting)
            let key_data = pop(&mut self.data);
            java_split(&key_data, ':')
        };

        for keymap in &subdata {
            // a keymap entry without its ";binding" half is skipped, not fatal
            let map = java_split(keymap, ';');
            if map.len() >= 2 && !map[0].is_empty() {
                g.input.set_key(&map[0], &map[1], g.debug);
            } else if !keymap.trim().is_empty() {
                crate::log_warn!(
                    "preferences: keymap entry {keymap:?} has no binding; keeping the default"
                );
            }
        }

        // day-cycle pacing (appended field; absent in older prefs files)
        if !self.data.is_empty() {
            let dc = pop(&mut self.data);
            if !dc.is_empty() {
                g.settings.set("daycycle", dc);
            }
        }
    }

    /// Java `loadWorld(filename)`.
    fn load_world(&mut self, g: &mut Game, filename: &str) {
        // infinite worlds: seed comes from WorldMeta; chunked layers rebuild lazily
        let meta_file = format!("{}WorldMeta{}", self.location, EXTENSION);
        let mut infinite = false;
        if let Ok(txt) = std::fs::read_to_string(&meta_file) {
            let mut parts = txt.trim().split(',');
            if parts.next() == Some("Infinite") {
                infinite = true;
                match parts.next().and_then(|s| s.parse::<i64>().ok()) {
                    Some(seed) => {
                        g.world_seed = seed;
                        g.random.set_seed(seed ^ 0x9E37_79B9);
                    }
                    None => crate::log_warn!(
                        "LOAD WARNING: WorldMeta{EXTENSION} carries no readable seed; explored chunks still load, unexplored ground will differ."
                    ),
                }
            }
        }
        // A lost/damaged WorldMeta used to send an infinite world down the finite path,
        // where the (never written) Level0..3 files took the load down. Chunk data on
        // disk is proof the world is infinite, so trust that instead of crashing.
        if !infinite && Path::new(&format!("{}chunks", self.location)).is_dir() {
            crate::log_warn!(
                "LOAD WARNING: WorldMeta{EXTENSION} is missing or damaged, but this world has chunk data; loading it as infinite."
            );
            infinite = true;
        }

        for l in (crate::level::MIN_LEVEL_DEPTH..=crate::level::MAX_LEVEL_DEPTH).rev() {
            g.loading_message = crate::level::get_depth_string(l); // LoadingDisplay.setMessage
            let lvlidx = crate::level::lvl_idx(l);
            if infinite && crate::level::is_infinite_depth(l) {
                rebuild_layer(g, l);
                continue;
            }
            let file = format!("{}{}{}{}", self.location, filename, lvlidx, EXTENSION);
            self.load_from_file(g, &file);

            let lvlw = at_i32(&self.data, 0, 0);
            let lvlh = at_i32(&self.data, 1, 0);
            // i64 so a bogus header ("100000,100000") can't overflow before it is
            // rejected; the file must actually carry a tile (and data) entry per tile.
            let area = lvlw as i64 * lvlh as i64;
            let usable = lvlw > 0
                && lvlh > 0
                && area <= self.data.len() as i64 - 3
                && area <= self.extradata.len() as i64;
            if !usable {
                // A missing or truncated layer file is not worth losing the whole world
                // over: rebuild this one layer from the seed and keep going, loudly.
                crate::log_error!(
                    "LOAD ERROR: {file} is missing or damaged ({lvlw}x{lvlh}, {} tile fields, {} data fields); rebuilding this layer.",
                    self.data.len().saturating_sub(3),
                    self.extradata.len()
                );
                rebuild_layer(g, l);
                continue;
            }

            let mut tiles = vec![0u8; area as usize];
            let mut tdata = vec![0u8; area as usize];

            for x in 0..lvlw {
                for y in 0..lvlh {
                    let tile_arr_idx = (y + x * lvlw) as usize;
                    // the tiles are saved with x outer loop, and y inner loop, meaning that
                    // the list reads down, then right one, rather than right, then down one.
                    let tileidx = (x + y * lvlw) as usize;
                    let mut tilename = self.data[tileidx + 3].clone();
                    if *self.wv() < Version::new("1.9.4-dev6") {
                        // they were id numbers, not names, at this point
                        let tile_id = parse_i32(&tilename, -1);
                        match old_id(tile_id) {
                            Some(name) => tilename = name.to_string(),
                            None => {
                                crate::log_info!("tile list doesn't contain tile {tile_id}");
                                tilename = "grass".to_string();
                            }
                        }
                    }
                    if l == crate::level::MIN_LEVEL_DEPTH + 1
                        && tilename.eq_ignore_ascii_case("LAPIS")
                        && *self.wv() < Version::new("2.0.3-dev6")
                    {
                        // incidental randomness, so g.random (not seed-derived)
                        if g.random.next_double() < 0.8 {
                            // don't replace *all* the lapis
                            tilename = "Gem Ore".to_string();
                        }
                    }
                    tiles[tile_arr_idx] = g.tiles.get(&tilename).id;
                    // legacy saves store data as a signed byte; values above 127 have
                    // never been valid in this format
                    tdata[tile_arr_idx] =
                        parse_i32(&self.extradata[tileidx], 0).clamp(-128, 127) as i8 as u8;
                }
            }

            let parent_idx = crate::level::lvl_idx(l + 1);
            let parent_exists = g.levels[parent_idx].is_some();

            // Java `new Level(lvlw, lvlh, l, parent, false)`.
            let mut cur_level = Level::empty(lvlw, lvlh, l, g.settings.get_idx("diff"));
            cur_level.tiles = tiles;
            cur_level.data = tdata;
            cur_level.reseed(g.world_seed);
            g.levels[lvlidx] = Some(cur_level);

            if g.debug {
                // Java curLevel.printTileLocs(Tiles.get("Stairs Down"))
                let t = g.tiles.get("Stairs Down");
                for x in 0..lvlw {
                    for y in 0..lvlh {
                        if g.tile_at(lvlidx, x, y).id == t.id {
                            print_level_loc(g, lvlidx, &t.name, x, y);
                        }
                    }
                }
            }

            if !parent_exists {
                continue;
            }
            // confirm that there are stairs in all the places that should have stairs.
            let stairs_down = g.tiles.get("Stairs Down");
            let stairs_up = g.tiles.get("Stairs Up");
            let down_id = stairs_down.id;
            let up_id = stairs_up.id;
            for p in crate::level::get_matching_tiles(g, parent_idx, |_, t, _, _| t.id == down_id) {
                if g.tile_at(lvlidx, p.x, p.y).id != up_id {
                    print_level_loc(
                        g,
                        lvlidx,
                        "INCONSISTENT STAIRS detected; placing stairsUp",
                        p.x,
                        p.y,
                    );
                    g.set_tile_default(lvlidx, p.x, p.y, &stairs_up);
                }
            }
            for p in crate::level::get_matching_tiles(g, lvlidx, |_, t, _, _| t.id == up_id) {
                if g.tile_at(parent_idx, p.x, p.y).id != down_id {
                    print_level_loc(
                        g,
                        parent_idx,
                        "INCONSISTENT STAIRS detected; placing stairsDown",
                        p.x,
                        p.y,
                    );
                    g.set_tile_default(parent_idx, p.x, p.y, &stairs_down);
                }
            }
        }
    }

    /// Java `loadPlayer(String filename, Player player)` — loads the main player.
    pub fn load_player_file(&mut self, g: &mut Game, filename: &str) {
        g.loading_message = "Player".to_string(); // LoadingDisplay.setMessage
        let file = format!("{}{}{}", self.location, filename, EXTENSION);
        self.load_from_file(g, &file);
        let data = self.data.clone();
        self.load_player(g, &data);
    }

    /// Java `loadPlayer(Player player, List<String> origData)` — applied to `g.player_id`.
    pub fn load_player(&self, g: &mut Game, orig_data: &[String]) {
        let mut data: Vec<String> = orig_data.to_vec();
        // the classic record carries 12 fields before any trailing marker
        if data.len() < 12 {
            crate::log_warn!(
                "player record is short ({} field(s)); missing values fall back to a fresh player",
                data.len()
            );
        }
        let mut player = g.entities.take(g.player_id).expect("player entity missing");

        // Defaults are the *fresh* player's own values, so a truncated Player file
        // yields a healthy player at the spawn point rather than a 0-HP corpse at
        // the origin.
        player.c.x = pop_i32(&mut data, player.c.x);
        player.c.y = pop_i32(&mut data, player.c.y);
        {
            let pd = player.player_mut();
            pd.spawnx = pop_i32(&mut data, pd.spawnx);
            pd.spawny = pop_i32(&mut data, pd.spawny);
            pd.mob.health = pop_i32(&mut data, pd.mob.health);
        }
        if *self.wv() >= Version::new("2.0.4-dev7") {
            let pd = player.player_mut();
            pd.hunger = pop_i32(&mut data, pd.hunger);
        }
        player.player_mut().armor = pop_i32(&mut data, 0);

        if player.player().armor > 0 && !data.is_empty() {
            if *self.wv() < Version::new("2.0.4-dev7") {
                // reverse order b/c we are taking from the end
                let idx = data.len() - 1;
                let cur_armor = crate::item::registry::get(g, &data.remove(idx));
                player.player_mut().cur_armor = Some(cur_armor);
                let buffer = if data.is_empty() {
                    0
                } else {
                    let idx = data.len() - 1;
                    parse_i32(&data.remove(idx), 0)
                };
                player.player_mut().armor_damage_buffer = buffer;
            } else {
                player.player_mut().armor_damage_buffer = pop_i32(&mut data, 0);
                let cur_armor = crate::item::registry::get(g, &pop(&mut data));
                player.player_mut().cur_armor = Some(cur_armor);
            }

            // Saves from before the wear-slot split wore hats on the lone armor
            // slot; migrate them to HEAD (their token hit meter retires with the
            // move — head gear has none).
            let is_head = player
                .player()
                .cur_armor
                .as_ref()
                .is_some_and(|a| wear_slot_for(a) == Some(WearSlot::Head));
            if is_head {
                let pd = player.player_mut();
                pd.worn_head = pd.cur_armor.take();
                pd.armor = 0;
                pd.armor_damage_buffer = 0;
            }
        }
        let score = pop_i32(&mut data, 0);
        player.player_mut().set_score(score);

        if *self.wv() < Version::new("2.0.4-dev7") {
            let arrow_count = pop_i32(&mut data, 0);
            if *self.wv() < Version::new("2.0.1-dev1") && arrow_count > 0 {
                let arrow = crate::item::registry::get(g, "arrow");
                player.player_mut().inventory.add_num(arrow, arrow_count);
            }
        }

        // A level index outside the world's five layers (an old six-level save, a
        // hand-edited file) would index straight off `g.levels`: clamp to the surface.
        let saved_level = pop_i32(&mut data, g.current_level as i32);
        g.current_level = if (0..g.levels.len() as i32).contains(&saved_level) {
            saved_level as usize
        } else {
            crate::log_warn!(
                "LOAD WARNING: player saved on level {saved_level}, which this world does not have; placing them on the surface."
            );
            crate::level::lvl_idx(0)
        };
        // removes the user player from the level, in case they would be added twice.
        if !player.c.removed {
            crate::entity::behavior::remove_entity(g, &mut player);
        }
        // The player entity is queued onto the level at the END of this function, after
        // the remaining fields are set — adding it earlier would freeze a half-loaded
        // player in the level queue.

        if *self.wv() < Version::new("2.0.4-dev8") {
            let modedata = pop(&mut data);
            self.load_mode(g, &modedata);
        }

        let potioneffects = pop(&mut data);
        if potioneffects != "PotionEffects[]" && !potioneffects.is_empty() {
            let effects = potioneffects.replace("PotionEffects[", "").replace(']', "");
            for effect in java_split(&effects, ':') {
                let effect = java_split(&effect, ';');
                // a removed/renamed potion, or an entry with no duration, is dropped
                // rather than taking the whole save down
                let Some(p_name) = potion_type_from_name(at(&effect, 0)) else {
                    crate::log_warn!(
                        "LOAD WARNING: unknown potion effect {:?} skipped",
                        at(&effect, 0)
                    );
                    continue;
                };
                let time = at_i32(&effect, 1, 0);
                if time <= 0 {
                    crate::log_warn!(
                        "potion effect {:?} has no usable duration; dropping it",
                        at(&effect, 0)
                    );
                    continue;
                }
                // Java PotionItem.applyPotion(player, pName, time).
                crate::item::interact::apply_potion_time(g, &mut player, p_name, time);
            }
        }

        if *self.wv() < Version::new("1.9.4-dev4") {
            let colors = pop(&mut data).replace(['[', ']'], "");
            let color = java_split(&colors, ';');
            let cols: Vec<i32> = (0..3).map(|i| at_i32(&color, i, 0) / 50).collect();
            let col = format!("{}{}{}", cols[0], cols[1], cols[2]);
            crate::log_info!("getting color as {col}");
            player.player_mut().shirt_color = parse_i32(&col, 0);
        } else {
            let pd = player.player_mut();
            pd.shirt_color = pop_i32(&mut data, pd.shirt_color);
        }

        player.player_mut().skinon = parse_bool(&pop(&mut data));

        // HEAD wear slot: a tagged trailing entry (save::WORN_HEAD_MARKER). Old
        // saves have no entry. A payload that isn't head-class gear (a hand-edited
        // or future save) skips with a warning — never a panic, never a bad slot.
        if let Some(name) = data
            .first()
            .and_then(|d| d.strip_prefix(crate::saveload::save::WORN_HEAD_MARKER))
        {
            let head = crate::item::registry::get(g, name);
            if wear_slot_for(&head) == Some(WearSlot::Head) {
                player.player_mut().worn_head = Some(head);
            } else {
                crate::log_warn!("WARNING: ignoring non-head worn item {name:?} in player save");
            }
            data.remove(0);
        }

        // Set-aside armor meter: same tolerant scheme; malformed fields read 0.
        if let Some(payload) = data
            .first()
            .and_then(|d| d.strip_prefix(crate::saveload::save::ARMOR_METER_MARKER))
        {
            let mut it = payload.split(';');
            let name = it.next().unwrap_or("").to_string();
            let hits: i32 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            let buffer: i32 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
            if !name.is_empty() {
                player.player_mut().worn_meter = Some((name, hits, buffer));
            }
            data.remove(0);
        }

        // Field Notes journal: same tolerant trailing-marker scheme. Old saves have
        // no entry and open a blank journal; a malformed payload reads as zeros.
        if let Some(payload) = data
            .first()
            .and_then(|d| d.strip_prefix(crate::saveload::save::NOTES_MARKER))
        {
            player.player_mut().notes = crate::core::field_notes::FieldNotes::decode(payload);
            data.remove(0);
        }

        // Gentle thirst (L6): tolerant trailing marker. Old saves have no entry and
        // load at full thirst; a malformed payload also reads as full, never a panic.
        if let Some(payload) = data
            .first()
            .and_then(|d| d.strip_prefix(crate::saveload::save::THIRST_MARKER))
        {
            use crate::entity::mob::player::MAX_THIRST;
            player.player_mut().thirst = payload
                .parse::<i32>()
                .map(|t| t.clamp(0, MAX_THIRST))
                .unwrap_or(MAX_THIRST);
            data.remove(0);
        }

        // Journal-learned recipe variants: tolerant trailing marker. Old saves have
        // no entry and know none; a malformed payload reads as none.
        if let Some(payload) = data
            .first()
            .and_then(|d| d.strip_prefix(crate::saveload::save::VARIANTS_MARKER))
        {
            player.player_mut().variants_learned = payload.trim().parse().unwrap_or(0);
            data.remove(0);
        }

        let cur = g.current_level;
        if g.levels[cur].is_some() {
            g.level_mut(cur).add(player, cur);
        } else {
            if g.debug {
                crate::log_info!("game level to add player Player to is null.");
            }
            g.entities.put_back(player);
        }
    }

    /// Java `loadInventory(String filename, Inventory inventory)` — loads the main
    /// player's inventory.
    pub fn load_inventory_file(&mut self, g: &mut Game, filename: &str) {
        let file = format!("{}{}{}", self.location, filename, EXTENSION);
        self.load_from_file(g, &file);
        let data = self.data.clone();

        let mut player = g.entities.take(g.player_id).expect("player entity missing");

        // Re-equip the held item if the save marked one (see save::HELD_MARKER).
        // Unmarked saves keep the historical behavior: everything into the inventory.
        let mut data = data.as_slice();
        if let Some(held) = data
            .first()
            .and_then(|d| d.strip_prefix(crate::saveload::save::HELD_MARKER))
        {
            player.player_mut().active_item = Some(crate::item::registry::get(g, held));
            data = &data[1..];
        }

        let mut inventory = std::mem::take(&mut player.player_mut().inventory);
        self.load_inventory(g, &mut inventory, data);
        player.player_mut().inventory = inventory;
        g.entities.put_back(player);
    }

    /// Java `loadInventory(Inventory inventory, List<String> data)`.
    pub fn load_inventory(&self, g: &Game, inventory: &mut Inventory, data: &[String]) {
        inventory.clear_inv();

        for item in data {
            let mut item = item.clone();
            if item.is_empty() {
                crate::log_warn!("loadInventory: item in data list is \"\", skipping item");
                continue;
            }

            if *self.wv() < Version::new("1.9.4") {
                item = sub_old_name(&item, self.wv());
            }

            if item.contains("Power Glove") {
                continue; // just pretend it doesn't exist. Because it doesn't. :P
            }

            if *self.wv() <= Version::new("2.0.4") && item.contains(';') {
                let cur_data = java_split(&item, ';');
                let item_name = at(&cur_data, 0);

                let mut new_item = crate::item::registry::get(g, item_name);

                // a stack whose count is missing or junk is worth one item, not a crash
                let count = at_i32(&cur_data, 1, 1).max(0);

                if new_item.is_stackable() {
                    new_item.set_count(count);
                    inventory.add(new_item);
                } else {
                    inventory.add_num(new_item, count);
                }
            } else {
                let to_add = crate::item::registry::get(g, &item);
                inventory.add(to_add);
            }
        }
    }

    /// Java `loadEntities(filename)`.
    fn load_entities(&mut self, g: &mut Game, filename: &str) {
        g.loading_message = "Entities".to_string(); // LoadingDisplay.setMessage
        let file = format!("{}{}{}", self.location, filename, EXTENSION);
        self.load_from_file(g, &file);

        for i in 0..g.levels.len() {
            crate::level::clear_entities(g, i);
        }
        let lines = self.data.clone();
        for line in &lines {
            if line.starts_with("Player") {
                continue;
            }
            load_entity(g, line, self.wv(), true);
        }

        for i in 0..g.levels.len() {
            crate::core::world::check_chest_count(g, i, true);
        }
    }
}

/// Java `Load.subOldName(name, worldVer)` — pre-1.9.4 item-name substitutions.
pub fn sub_old_name(name: &str, world_ver: &Version) -> String {
    let mut name = name.to_string();
    if *world_ver < Version::new("1.9.4-dev4") {
        name = name
            .replace("Hatchet", "Axe")
            .replace("Pick", "Pickaxe")
            .replace("Pickaxeaxe", "Pickaxe")
            .replace("Spade", "Shovel")
            .replace("Pow glove", "Power Glove")
            .replace("II", "")
            .replace("W.Bucket", "Water Bucket")
            .replace("L.Bucket", "Lava Bucket")
            .replace("G.Apple", "Gold Apple")
            .replace("St.", "Stone")
            .replace("Ob.", "Obsidian")
            .replace("I.Lantern", "Iron Lantern")
            .replace("G.Lantern", "Gold Lantern")
            .replace("BrickWall", "Wall")
            .replace("Brick", " Brick")
            .replace("Wall", " Wall")
            .replace("  ", " ");
        if name == "Bucket" {
            name = "Empty Bucket".to_string();
        }
    }

    if *world_ver < Version::new("1.9.4") {
        name = name
            .replace("I.Armor", "Iron Armor")
            .replace("S.Armor", "Snake Armor")
            .replace("L.Armor", "Leather Armor")
            .replace("G.Armor", "Gold Armor")
            .replace("BrickWall", "Wall");
    }

    name
}

/// Java static `Load.loadFromFile(filename, isWorldSave)`.
pub fn load_from_file_str(filename: &str, is_world_save: bool) -> std::io::Result<String> {
    let content = std::fs::read_to_string(filename)?;
    let mut total = String::new();
    for cur_line in content.lines() {
        total.push_str(cur_line);
        if !is_world_save {
            total.push('\n');
        }
    }
    Ok(total)
}

/// Java static `Load.loadEntity(entityData, worldVer, isLocalSave)`.
///
/// Returns the eid the entity carried into the level queue (-1 for local saves; a fresh
/// one is generated when the level drains its queue into the arena), or `None` when
/// nothing was loaded (Java returned null).
pub fn load_entity(
    g: &mut Game,
    entity_data: &str,
    world_ver: &Version,
    is_local_save: bool,
) -> Option<i32> {
    let entity_data = entity_data.trim();
    if entity_data.is_empty() {
        return None;
    }

    // "Name[a:b:c]" — a record missing either bracket (or with them the wrong way
    // round) is unreadable: skip that one entity, keep the rest of the world.
    let (Some(bracket_open), Some(bracket_close)) = (entity_data.find('['), entity_data.rfind(']'))
    else {
        crate::log_warn!("LOAD WARNING: malformed entity record skipped: {entity_data:?}");
        return None;
    };
    if bracket_close < bracket_open {
        crate::log_warn!("LOAD WARNING: malformed entity record skipped: {entity_data:?}");
        return None;
    }
    // this gets everything inside the "[...]" after the entity name.
    let mut info: Vec<String> = java_split(&entity_data[bracket_open + 1..bracket_close], ':');

    // this gets the text before "[", which is the entity name.
    let entity_name = &entity_data[..bracket_open];

    // every record carries at least "x:y:...:level" (plus an eid when not a local save)
    if info.len() < if is_local_save { 3 } else { 4 } {
        crate::log_warn!(
            "LOAD WARNING: entity record has too few fields, skipped: {entity_data:?}"
        );
        return None;
    }

    let x = at_i32(&info, 0, 0);
    let y = at_i32(&info, 1, 0);

    let mut eid = -1;
    if !is_local_save {
        eid = parse_i32(&info.remove(2), -1);
    }

    let new_entity: Option<Entity> = if entity_name == "RemotePlayer" {
        if is_local_save {
            crate::log_warn!("remote player found in local save file.");
        }
        return None; // a relic of old multiplayer saves; never loaded
    } else if entity_name == "Zap" && !is_local_save {
        let wisp_id = at_i32(&info, 2, -1);
        let zap_owner = g
            .entities
            .get(wisp_id)
            .filter(|e| matches!(e.kind, EntityKind::NightWisp(_)))
            .map(|e| (e.c.x, e.c.y));
        match zap_owner {
            Some((ox, oy)) => {
                // quirk kept from the original wire format: the stored x/y land in the
                // (xa, ya) velocity parameters, not the position
                let mut rnd = g.random.clone();
                let e = crate::entity::projectile::new_zap(
                    wisp_id, ox, oy, x as f64, y as f64, &mut rnd,
                );
                g.random = rnd;
                Some(e)
            }
            None => {
                crate::log_warn!("failed to load zap; owner id doesn't point to a correct entity");
                return None;
            }
        }
    } else {
        let mut mob_lvl = 1;
        // enemy mobs carry a level in their save record; crafter names are excluded
        // (a "Furnace" is furniture, not a mob)
        let is_crafter_name = crate::entity::furniture::crafter::CrafterType::VALUES
            .iter()
            .any(|t| t.name() == entity_name);
        let is_enemy_mob_class = matches!(
            entity_name,
            "Zombie"
                | "Knight"
                | "Snake"
                | "MarshLurker"
                | "FeralHound"
                | "StoneGolem"
                | "NightWisp"
                | "GrassSnake"
                | "Adder"
                | "Rattler"
                | "Ghost"
        );
        if !is_crafter_name && is_enemy_mob_class && info.len() >= 2 {
            // clamped: a junk level would otherwise reach the mob constructors' color
            // and stat tables
            mob_lvl = at_i32(&info, info.len() - 2, 1).clamp(0, 4);
        }

        if mob_lvl == 0 {
            if g.debug {
                crate::log_info!("level 0 mob: {entity_name}");
            }
            mob_lvl = 1;
        }

        // Java entityName.substring(entityName.lastIndexOf(".")+1).
        let simple_name = entity_name.rsplit('.').next().unwrap_or(entity_name);
        get_entity(g, simple_name, mob_lvl)
    };

    let mut new_entity = new_entity?;

    if new_entity.is_mob() {
        let hp = new_entity.mob().map(|m| m.health).unwrap_or(1);
        new_entity.mob_mut().unwrap().health = at_i32(&info, 2, hp);
    } else if new_entity.is_chest() {
        let is_death_chest = matches!(new_entity.kind, EntityKind::DeathChest(_));
        let is_dungeon_chest = matches!(new_entity.kind, EntityKind::DungeonChest(_));
        let is_scav_container = matches!(new_entity.kind, EntityKind::ScavContainer(_));
        // fields between "x:y" and the trailing level: the chest's contents plus the
        // per-kind tail below (a short record simply has none)
        let chest_info: Vec<String> = info
            .get(2..info.len().saturating_sub(1))
            .unwrap_or_default()
            .to_vec();

        let tail = if is_death_chest || is_dungeon_chest {
            1
        } else if is_scav_container {
            2 // trailing ScavKind ordinal + searched flag
        } else {
            0
        };
        let end_idx = chest_info.len().saturating_sub(tail);
        for item_data in &chest_info[..end_idx] {
            let mut item_data = item_data.clone();
            if *world_ver < Version::new("1.9.4-dev4") {
                item_data = sub_old_name(&item_data, world_ver);
            }

            if item_data.contains("Power Glove") {
                continue; // ignore it.
            }

            if item_data.contains(';') {
                let aitem_data = java_split(&item_data, ';');
                let mut stack = crate::item::registry::get(g, at(&aitem_data, 0));
                if !matches!(stack.kind, crate::item::ItemKind::Unknown { .. }) {
                    stack.set_count(at_i32(&aitem_data, 1, 1).max(0));
                    new_entity.chest_mut().unwrap().inventory.add(stack);
                } else {
                    crate::log_error!(
                        "LOAD ERROR: encountered invalid item name, expected to be stackable: {}",
                        at(&aitem_data, 0)
                    );
                }
            } else {
                let item = crate::item::registry::get(g, &item_data);
                new_entity.chest_mut().unwrap().inventory.add(item);
            }
        }

        // the per-kind tail fields; `end_idx` already excluded them from the item loop,
        // and a record too short to carry them falls back to the constructor defaults
        let tail_fields = &chest_info[end_idx..];
        if is_death_chest {
            if tail_fields.is_empty() {
                crate::log_warn!("DeathChest record carries no despawn time; keeping the default");
            }
            if let EntityKind::DeathChest(dc) = &mut new_entity.kind {
                dc.time = at_i32(tail_fields, 0, dc.time);
            }
        } else if is_dungeon_chest {
            let is_locked = parse_bool(at(tail_fields, 0));
            if let EntityKind::DungeonChest(dc) = &mut new_entity.kind {
                dc.is_locked = is_locked;
            }
            if is_locked {
                let lvl = at_i32(&info, info.len() - 1, -1);
                if (0..g.levels.len() as i32).contains(&lvl) && g.levels[lvl as usize].is_some() {
                    g.level_mut(lvl as usize).chest_count += 1;
                }
            }
        } else if is_scav_container {
            use crate::entity::furniture::scav_container::ScavKind;
            let ordinal = at_i32(tail_fields, 0, 0).max(0) as usize;
            let searched = parse_bool(at(tail_fields, 1));
            let kind = match ScavKind::VALUES.get(ordinal).copied() {
                Some(k) => k,
                None => {
                    crate::log_warn!(
                        "ScavContainer kind ordinal {ordinal} is unknown; loading it as a Crate"
                    );
                    ScavKind::Crate
                }
            };
            if tail_fields.is_empty() {
                crate::log_warn!("ScavContainer record carries no kind/searched trailer");
            }
            if let EntityKind::ScavContainer(sc) = &mut new_entity.kind {
                sc.kind = kind;
                sc.searched = searched;
                sc.chest.furniture.name = kind.title().to_string();
                sc.chest.furniture.sprite = kind.sprite(searched);
            }
            new_entity.c.col = kind.col(searched);
        }
    } else if matches!(new_entity.kind, EntityKind::Spawner(_)) {
        let raw = at(&info, 2).to_string();
        let mob_name = raw.rsplit('.').next().unwrap_or(&raw).to_string();
        let mob = get_entity(g, &mob_name, at_i32(&info, 3, 1).clamp(1, 4));
        if let Some(mob) = mob {
            let mut rnd = g.random.clone();
            new_entity = crate::entity::furniture::spawner::new(mob, &mut rnd);
            g.random = rnd;
        }
    } else if matches!(new_entity.kind, EntityKind::Lantern(_))
        && *world_ver >= Version::new("1.9.4")
        && info.len() > 3
    {
        // an out-of-range ordinal (a lantern type this build no longer has) falls back
        // to the plain lantern rather than indexing off the table
        use crate::entity::furniture::lantern::LanternType;
        let t = at_i32(&info, 2, 0).max(0) as usize;
        let lantern_type = LanternType::VALUES.get(t).copied().unwrap_or_else(|| {
            crate::log_warn!("LOAD WARNING: unknown lantern type {t}, loading a plain lantern");
            LanternType::Norm
        });
        new_entity = crate::entity::furniture::lantern::new(lantern_type);
    } else if matches!(new_entity.kind, EntityKind::Campfire(_)) && info.len() > 3 {
        // fire wave: restore the remaining fuel (and the matching lit/ember sprite)
        let fuel = at_i32(&info, 2, 0);
        if let EntityKind::Campfire(cf) = &mut new_entity.kind {
            cf.fuel = fuel.max(0);
            cf.furniture.sprite = if cf.fuel > 0 {
                crate::entity::furniture::campfire::lit_sprite()
            } else {
                crate::entity::furniture::campfire::ember_sprite()
            };
        }
    } else if entity_name == "Bench" && info.len() > 3 {
        // THE BENCH: refit the saved module ordinals (unknown ordinals skip —
        // the same old-save tolerance rule as everywhere else)
        use crate::entity::furniture::crafter::Module;
        if let EntityKind::Crafter(c) = &mut new_entity.kind {
            for ord in at(&info, 2).split(';').filter(|s| !s.is_empty()) {
                if let Some(m) = ord
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| Module::VALUES.get(i))
                    && !c.modules.contains(m)
                {
                    c.modules.push(*m);
                }
            }
        }
    }

    if !is_local_save {
        // transient-entity payloads (see write_entity): only present in non-local data
        if matches!(new_entity.kind, EntityKind::Arrow(_)) {
            let owner_id = at_i32(&info, 2, -1);
            let owner_is_mob = g
                .entities
                .get(owner_id)
                .map(|e| e.is_mob())
                .unwrap_or(false);
            if owner_is_mob {
                use crate::entity::Direction;
                let d = at_i32(&info, 3, 0).max(0) as usize;
                let dir = Direction::VALUES.get(d).copied().unwrap_or(Direction::None);
                let dmg = at_i32(&info, 5, 0);
                new_entity = crate::entity::projectile::new_arrow(owner_id, x, y, dir, dmg);
            }
        }
        if matches!(new_entity.kind, EntityKind::ItemEntity(_)) {
            let item = crate::item::registry::get(g, at(&info, 2));
            let f = |i: usize| at(&info, i).trim().parse::<f64>().unwrap_or(0.0);
            let mut rnd = g.random.clone();
            new_entity = crate::entity::item_entity::with_motion(
                item,
                x,
                y,
                f(3),
                at_i32(&info, 4, 0),
                at_i32(&info, 5, 0),
                f(6),
                f(7),
                f(8),
                &mut rnd,
            );
            g.random = rnd;
        }
        if matches!(new_entity.kind, EntityKind::TextParticle(_)) {
            let textcol = at_i32(&info, 3, 0);
            let msg = at(&info, 2).to_string();
            let mut rnd = g.random.clone();
            new_entity = crate::entity::particle::new_text_particle(&msg, x, y, textcol, &mut rnd);
            g.random = rnd;
        }
    }

    // this will be -1 unless set earlier, so a new one will be generated when adding it
    // to the level.
    new_entity.c.eid = eid;
    if matches!(new_entity.kind, EntityKind::ItemEntity(_)) && eid == -1 {
        crate::log_warn!("Warning: item entity was loaded with no eid");
    }

    // The trailing field is the level slot. An index outside the world's layers (an
    // old six-level save, a hand-edited record) used to index straight off `g.levels`.
    let cur_level = at_i32(&info, info.len() - 1, -1);
    if !(0..g.levels.len() as i32).contains(&cur_level) {
        crate::log_warn!(
            "LOAD WARNING: {entity_name} saved on level {cur_level}, which this world does not have; skipped."
        );
        return None;
    }
    let cur_level = cur_level as usize;
    if g.levels[cur_level].is_some() {
        g.level_mut(cur_level)
            .add_at(new_entity, x, y, false, cur_level);
    }

    Some(eid)
}

/// Java static `Load.getEntity(string, moblvl)`.
fn get_entity(g: &mut Game, string: &str, moblvl: i32) -> Option<Entity> {
    use crate::entity::furniture;
    use crate::entity::mob;

    match string {
        "Player" => None,
        "RemotePlayer" => None,
        "Cow" => Some(mob::cow::new(g)),
        "Deer" => Some(mob::deer::new(g)),
        "Sheep" => Some(mob::sheep::new(g)),
        "Pig" => Some(mob::pig::new(g)),
        "Zombie" => Some(mob::zombie::new(g, moblvl)),
        "GlowWorm" => Some(mob::glow_worm::new(g)),
        "Knight" => Some(mob::knight::new(g, moblvl)),
        "Snake" => Some(mob::snake::new(g, moblvl)),
        "GrassSnake" => Some(mob::snake::new_variant(
            g,
            mob::snake::SnakeVariant::Grass,
            moblvl,
        )),
        "Adder" => Some(mob::snake::new_variant(
            g,
            mob::snake::SnakeVariant::Adder,
            moblvl,
        )),
        "Rattler" => Some(mob::snake::new_variant(
            g,
            mob::snake::SnakeVariant::Rattler,
            moblvl,
        )),
        "MarshLurker" => Some(mob::marsh_lurker::new(g, moblvl)),
        "FeralHound" => Some(mob::feral_hound::new(g, moblvl)),
        "StoneGolem" => Some(mob::stone_golem::new(g, moblvl)),
        "NightWisp" => Some(mob::night_wisp::new(g, moblvl)),
        "Ghost" => Some(mob::ghost::new(g, moblvl)),
        "Spawner" => {
            let zombie = mob::zombie::new(g, 1);
            let mut rnd = g.random.clone();
            let e = furniture::spawner::new(zombie, &mut rnd);
            g.random = rnd;
            Some(e)
        }
        "Workbench" => Some(furniture::crafter::new(
            furniture::crafter::CrafterType::Workbench,
        )),
        "Chest" => Some(furniture::chest::new()),
        "DeathChest" => Some(furniture::death_chest::new(g)),
        "DungeonChest" => Some(furniture::dungeon_chest::new(g)),
        // kind + searched state are restored from the trailing save fields
        "ScavContainer" => Some(furniture::scav_container::new(
            furniture::scav_container::ScavKind::Crate,
        )),
        "Anvil" => Some(furniture::crafter::new(
            furniture::crafter::CrafterType::Anvil,
        )),
        "Enchanter" => Some(furniture::crafter::new(
            furniture::crafter::CrafterType::Enchanter,
        )),
        "Loom" => Some(furniture::crafter::new(
            furniture::crafter::CrafterType::Loom,
        )),
        "Furnace" => Some(furniture::crafter::new(
            furniture::crafter::CrafterType::Furnace,
        )),
        "Oven" => Some(furniture::crafter::new(
            furniture::crafter::CrafterType::Oven,
        )),
        "Bench" => Some(furniture::crafter::new(
            furniture::crafter::CrafterType::Bench,
        )),
        "Bed" => Some(furniture::bed::new()),
        "Campfire" => Some(furniture::campfire::new()),
        "Tnt" => Some(furniture::tnt::new()),
        "Lantern" => Some(furniture::lantern::new(
            furniture::lantern::LanternType::Norm,
        )),
        "Arrow" => {
            // owner eid -1 = "no owner" (the real owner is patched in by the caller)
            Some(crate::entity::projectile::new_arrow(
                -1,
                0,
                0,
                crate::entity::Direction::None,
                0,
            ))
        }
        "ItemEntity" => {
            let unknown = crate::item::registry::get(g, "unknown");
            let mut rnd = g.random.clone();
            let e = crate::entity::item_entity::new(unknown, 0, 0, &mut rnd);
            g.random = rnd;
            Some(e)
        }
        "FireParticle" => Some(crate::entity::particle::new_fire_particle(0, 0)),
        "SmashParticle" => Some(crate::entity::particle::new_smash_particle(0, 0)),
        "TextParticle" => {
            let mut rnd = g.random.clone();
            let e = crate::entity::particle::new_text_particle("", 0, 0, 0, &mut rnd);
            g.random = rnd;
            Some(e)
        }
        // Removed kinds (Creeper/Slime/Skeleton/AirWizard/Spark, or anything else
        // unknown) land here: log and skip the entity rather than panicking, so old
        // saves still load minus the missing mobs.
        _ => {
            crate::log_warn!("LOAD WARNING: unknown or outdated entity skipped: {string}");
            None
        }
    }
}
