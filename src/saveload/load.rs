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
use crate::entity::{Entity, EntityKind};
use crate::item::Inventory;
use crate::level::Level;
use crate::saveload::save::EXTENSION;
use crate::saveload::version::Version;
use crate::screen::entry::array_entry::Value;

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

/// Java `Enum.valueOf(PotionType.class, name)` — panics on an unknown name, as Java threw.
fn potion_type_from_name(name: &str) -> crate::item::PotionType {
    crate::item::PotionType::VALUES
        .iter()
        .copied()
        .find(|p| p.enum_name() == name)
        .unwrap_or_else(|| panic!("No enum constant PotionType.{name}"))
}

/// Java `Level.printLevelLoc(prefix, x, y)`.
fn print_level_loc(g: &Game, lvl: usize, prefix: &str, x: i32, y: i32) {
    let level_name = crate::level::get_level_name(g.level(lvl).depth);
    println!("{prefix} on {level_name} level ({x},{y})");
}

/// The state of Java's `Load` object.
pub struct Load {
    location: String,
    percent_inc: f32,
    data: Vec<String>,
    extradata: Vec<String>,
    world_ver: Option<Version>,
    has_global_prefs: bool,
}

/// Java `new Load(worldname)` — loads the whole world.
pub fn load_world_named(g: &mut Game, world_name: &str) {
    new_world(g, world_name, true);
}

/// Alias of [`load_world_named`] (call-site name used by `core::world::init_world`).
pub fn load_world(g: &mut Game, world_name: &str) {
    load_world_named(g, world_name);
}

/// Java `new Load(worldname, loadGame)`.
pub fn new_world(g: &mut Game, worldname: &str, load_game: bool) -> Load {
    let mut l = Load::init(g);

    let game_file = format!("{}/saves/{}/Game{}", l.location, worldname, EXTENSION);
    l.load_from_file(g, &game_file);
    if l.data[0].contains('.') {
        l.world_ver = Some(Version::new(&l.data[0]));
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

    if *l.wv() < Version::new("1.9.2") {
        // JAVA: new LegacyLoad(worldname). LegacyLoad is not ported; see the module docs.
        eprintln!(
            "LOAD ERROR: world \"{}\" was saved by version {}; pre-1.9.2 saves (LegacyLoad) are not supported in this port.",
            worldname,
            l.wv()
        );
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
        // JAVA: new LegacyLoad(testFile) → updateUnlocks.
        l.legacy_update_unlocks(g, &test_file);
    } else if !Path::new(&test_file).exists() {
        if let Err(ex) = std::fs::File::create(&test_file) {
            eprintln!("could not create Unlocks{EXTENSION}:");
            eprintln!("{ex}");
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
        }
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
            Err(ex) => eprintln!("{ex}"), // Java ex.printStackTrace()
        }

        if filename.contains("Level") {
            // JAVA: filename.substring(0, filename.lastIndexOf("/") + 7) + "data" + ext —
            // keeps "Level" plus the level number after the last slash.
            let cut = filename.rfind('/').map(|i| i + 7).unwrap_or(filename.len());
            let datafile = format!("{}data{}", &filename[..cut], EXTENSION);
            match load_from_file_str(&datafile, true) {
                Ok(total) => self.extradata.extend(java_split(&total, ',')),
                Err(ex) => eprintln!("{ex}"),
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
            Err(ex) => eprintln!("{ex}"),
        }
        // JAVA: LegacyLoad.loadFromFile progresses the loading bar by 13.
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
            Err(ex) => eprintln!("{ex}"),
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

            if unlock.contains("_ScoreTime") {
                let num: i32 = unlock[..unlock.find('_').unwrap()].parse().unwrap();
                g.settings
                    .get_entry("scoretime")
                    .borrow_mut()
                    .set_value_visibility(&Value::Int(num), true);
            }
        }
    }

    /// Java `loadGame(filename)`.
    fn load_game(&mut self, g: &mut Game, filename: &str) {
        let file = format!("{}{}{}", self.location, filename, EXTENSION);
        self.load_from_file(g, &file);

        self.world_ver = Some(Version::new(&self.data.remove(0))); // gets the world version
        if *self.wv() >= Version::new("2.0.4-dev8") {
            let modedata = self.data.remove(0);
            self.load_mode(g, &modedata);
        }

        g.set_time(self.data.remove(0).parse().unwrap());

        g.game_time = self.data.remove(0).parse().unwrap();
        if *self.wv() >= Version::new("1.9.3-dev2") {
            g.past_day1 = g.game_time > 65000;
        } else {
            g.game_time = 65000; // prevents time cheating.
        }

        let mut diff_idx: i32 = self.data.remove(0).parse().unwrap();
        if *self.wv() < Version::new("1.9.3-dev3") {
            diff_idx -= 1; // account for change in difficulty
        }

        g.settings.set_idx("diff", diff_idx);

        g.air_wizard_beaten = parse_bool(&self.data.remove(0));
    }

    /// Java `loadMode(modedata)`.
    fn load_mode(&self, g: &mut Game, modedata: &str) {
        let mut mode: i32;
        if modedata.contains(';') {
            let modeinfo = java_split(modedata, ';');
            mode = modeinfo[0].parse().unwrap();
            if *self.wv() <= Version::new("2.0.3") {
                mode -= 1; // we changed the min mode idx from 1 to 0.
            }
            if mode == 3 {
                g.score_time = modeinfo[1].parse().unwrap();
                if *self.wv() >= Version::new("1.9.4") {
                    // JAVA: Settings.set("scoretime", modeinfo[2]) — a String value never
                    // matches the Integer options, so this was always a no-op; preserved.
                    g.settings.set("scoretime", modeinfo[2].clone());
                }
            }
        } else {
            mode = modedata.parse().unwrap();
            if *self.wv() <= Version::new("2.0.3") {
                mode -= 1; // we changed the min mode idx from 1 to 0.
            }

            if mode == 3 {
                g.score_time = 300;
            }
        }

        g.settings.set_idx("mode", mode);
    }

    /// Java `loadPrefs(filename)`.
    fn load_prefs(&mut self, g: &mut Game, filename: &str) {
        let file = format!("{}{}{}", self.location, filename, EXTENSION);
        self.load_from_file(g, &file);

        // the default, b/c this doesn't really matter much being specific past this if
        // it's not set below.
        let mut pref_ver = Version::new("2.0.2");

        if !self.data[2].contains(';') {
            // signifies that this file was last written to by a version after 2.0.2.
            pref_ver = Version::new(&self.data.remove(0));
        }

        g.settings.set("sound", parse_bool(&self.data.remove(0)));
        g.settings.set("autosave", parse_bool(&self.data.remove(0)));

        if pref_ver >= Version::new("2.0.4-dev2") {
            g.settings
                .set("fps", self.data.remove(0).parse::<i32>().unwrap());
        }

        let subdata: Vec<String> = if pref_ver < Version::new("2.0.3-dev1") {
            self.data.clone()
        } else {
            // JAVA: MultiplayerDisplay.savedIP — multiplayer display is stubbed; discarded.
            let _saved_ip = self.data.remove(0);
            if pref_ver > Version::new("2.0.3-dev3") {
                // JAVA: MultiplayerDisplay.savedUUID / savedUsername.
                let _saved_uuid = self.data.remove(0);
                let _saved_username = self.data.remove(0);
            }

            if pref_ver >= Version::new("2.0.4-dev3") {
                let lang = self.data.remove(0);
                g.settings.set("language", lang.clone());
                g.localization.change_language(&lang);
            }

            let key_data = self.data[0].clone();
            java_split(&key_data, ':')
        };

        for keymap in &subdata {
            let map = java_split(keymap, ';');
            g.input.set_key(&map[0], &map[1], g.debug);
        }
    }

    /// Java `loadWorld(filename)`.
    fn load_world(&mut self, g: &mut Game, filename: &str) {
        for l in (crate::level::MIN_LEVEL_DEPTH..=crate::level::MAX_LEVEL_DEPTH).rev() {
            g.loading_message = crate::level::get_depth_string(l); // LoadingDisplay.setMessage
            let lvlidx = crate::level::lvl_idx(l);
            let file = format!("{}{}{}{}", self.location, filename, lvlidx, EXTENSION);
            self.load_from_file(g, &file);

            let lvlw: i32 = self.data[0].parse().unwrap();
            let lvlh: i32 = self.data[1].parse().unwrap();

            let mut tiles = vec![0u8; (lvlw * lvlh) as usize];
            let mut tdata = vec![0u8; (lvlw * lvlh) as usize];

            for x in 0..lvlw {
                for y in 0..lvlh {
                    let tile_arr_idx = (y + x * lvlw) as usize;
                    // the tiles are saved with x outer loop, and y inner loop, meaning that
                    // the list reads down, then right one, rather than right, then down one.
                    let tileidx = (x + y * lvlw) as usize;
                    let mut tilename = self.data[tileidx + 3].clone();
                    if *self.wv() < Version::new("1.9.4-dev6") {
                        // they were id numbers, not names, at this point
                        let tile_id: i32 = tilename.parse().unwrap();
                        match old_id(tile_id) {
                            Some(name) => tilename = name.to_string(),
                            None => {
                                println!("tile list doesn't contain tile {tile_id}");
                                tilename = "grass".to_string();
                            }
                        }
                    }
                    if l == crate::level::MIN_LEVEL_DEPTH + 1
                        && tilename.eq_ignore_ascii_case("LAPIS")
                        && *self.wv() < Version::new("2.0.3-dev6")
                    {
                        // JAVA: Math.random() — incidental randomness, uses g.random here.
                        if g.random.next_double() < 0.8 {
                            // don't replace *all* the lapis
                            tilename = "Gem Ore".to_string();
                        }
                    }
                    tiles[tile_arr_idx] = g.tiles.get(&tilename).id;
                    // JAVA: Byte.parseByte — values above 127 threw in Java too.
                    tdata[tile_arr_idx] = self.extradata[tileidx].parse::<i8>().unwrap() as u8;
                }
            }

            let parent_idx = crate::level::lvl_idx(l + 1);
            let parent_exists = g.levels[parent_idx].is_some();

            // Java `new Level(lvlw, lvlh, l, parent, false)`.
            let mut cur_level = Level::empty(lvlw, lvlh, l, g.settings.get_idx("diff"));
            cur_level.tiles = tiles;
            cur_level.data = tdata;
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
        let mut player = g.entities.take(g.player_id).expect("player entity missing");

        player.c.x = data.remove(0).parse().unwrap();
        player.c.y = data.remove(0).parse().unwrap();
        {
            let pd = player.player_mut();
            pd.spawnx = data.remove(0).parse().unwrap();
            pd.spawny = data.remove(0).parse().unwrap();
            pd.mob.health = data.remove(0).parse().unwrap();
        }
        if *self.wv() >= Version::new("2.0.4-dev7") {
            player.player_mut().hunger = data.remove(0).parse().unwrap();
        }
        player.player_mut().armor = data.remove(0).parse().unwrap();

        if player.player().armor > 0 {
            if *self.wv() < Version::new("2.0.4-dev7") {
                // reverse order b/c we are taking from the end
                let idx = data.len() - 1;
                let cur_armor = crate::item::registry::get(g, &data.remove(idx));
                player.player_mut().cur_armor = Some(cur_armor);
                let idx = data.len() - 1;
                player.player_mut().armor_damage_buffer = data.remove(idx).parse().unwrap();
            } else {
                player.player_mut().armor_damage_buffer = data.remove(0).parse().unwrap();
                let cur_armor = crate::item::registry::get(g, &data.remove(0));
                player.player_mut().cur_armor = Some(cur_armor);
            }
        }
        player
            .player_mut()
            .set_score(data.remove(0).parse().unwrap());

        if *self.wv() < Version::new("2.0.4-dev7") {
            let arrow_count: i32 = data.remove(0).parse().unwrap();
            if *self.wv() < Version::new("2.0.1-dev1") {
                let arrow = crate::item::registry::get(g, "arrow");
                player.player_mut().inventory.add_num(arrow, arrow_count);
            }
        }

        g.current_level = data.remove(0).parse::<i32>().unwrap() as usize;
        // removes the user player from the level, in case they would be added twice.
        if !player.c.removed {
            crate::entity::behavior::remove_entity(g, &mut player);
        }
        // JAVA: the level.add(player) happens here; in this port the player entity is
        // moved into the level queue at the end of this function, after the remaining
        // fields are set (Java kept mutating its shared reference — same final state).

        if *self.wv() < Version::new("2.0.4-dev8") {
            let modedata = data.remove(0);
            // JAVA: only load if you're loading the main player — always true here.
            self.load_mode(g, &modedata);
        }

        let potioneffects = data.remove(0);
        if potioneffects != "PotionEffects[]" {
            let effects = potioneffects.replace("PotionEffects[", "").replace(']', "");
            for effect in java_split(&effects, ':') {
                let effect = java_split(&effect, ';');
                let p_name = potion_type_from_name(&effect[0]);
                let time: i32 = effect[1].parse().unwrap();
                // Java PotionItem.applyPotion(player, pName, time).
                crate::item::interact::apply_potion_time(g, &mut player, p_name, time);
            }
        }

        if *self.wv() < Version::new("1.9.4-dev4") {
            let colors = data.remove(0).replace(['[', ']'], "");
            let color = java_split(&colors, ';');
            let cols: Vec<i32> = color
                .iter()
                .map(|c| c.parse::<i32>().unwrap() / 50)
                .collect();
            let col = format!("{}{}{}", cols[0], cols[1], cols[2]);
            println!("getting color as {col}");
            player.player_mut().shirt_color = col.parse().unwrap();
        } else {
            player.player_mut().shirt_color = data.remove(0).parse().unwrap();
        }

        player.player_mut().skinon = parse_bool(&data.remove(0));

        // JAVA: `if(!Game.isValidServer() || player != Game.player)` — always true here.
        let cur = g.current_level;
        if g.levels[cur].is_some() {
            g.level_mut(cur).add(player, cur);
        } else {
            if g.debug {
                // JAVA: Network.onlinePrefix() + "game level to add player ... is null."
                println!("game level to add player Player to is null.");
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
        let mut inventory = std::mem::take(&mut player.player_mut().inventory);
        self.load_inventory(g, &mut inventory, &data);
        player.player_mut().inventory = inventory;
        g.entities.put_back(player);
    }

    /// Java `loadInventory(Inventory inventory, List<String> data)`.
    pub fn load_inventory(&self, g: &Game, inventory: &mut Inventory, data: &[String]) {
        inventory.clear_inv();

        for item in data {
            let mut item = item.clone();
            if item.is_empty() {
                eprintln!("loadInventory: item in data list is \"\", skipping item");
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
                let item_name = &cur_data[0];

                let mut new_item = crate::item::registry::get(g, item_name);

                let count: i32 = cur_data[1].parse().unwrap();

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
            crate::core::world::check_air_wizard(g, i, true);
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

    let bracket_open = entity_data.find('[').expect("malformed entity data");
    let bracket_close = entity_data.find(']').expect("malformed entity data");
    // this gets everything inside the "[...]" after the entity name.
    let mut info: Vec<String> = java_split(&entity_data[bracket_open + 1..bracket_close], ':');

    // this gets the text before "[", which is the entity name.
    let entity_name = &entity_data[..bracket_open];

    // JAVA: a CLIENT WARNING debug print for "Player" — isValidClient() is false here.

    let x: i32 = info[0].parse().unwrap();
    let y: i32 = info[1].parse().unwrap();

    let mut eid = -1;
    if !is_local_save {
        eid = info.remove(2).parse().unwrap();
        // JAVA: the multiplayer entity-sync path — Network.getEntity(eid) replacement of
        // stale entities, and the RemotePlayer shouldTrack/dummy handling. The network
        // layer is stubbed (isValidClient() is false, no remote entities exist).
    }

    let new_entity: Option<Entity> = if entity_name == "RemotePlayer" {
        if is_local_save {
            eprintln!("remote player found in local save file.");
            return None; // don't load them; in fact, they shouldn't be here.
        }
        // JAVA: constructs a RemotePlayer from username/ip/port — multiplayer stub.
        return None;
    } else if entity_name == "Spark" && !is_local_save {
        let aw_id: i32 = info[2].parse().unwrap();
        let spark_owner = g
            .entities
            .get(aw_id)
            .filter(|e| matches!(e.kind, EntityKind::AirWizard(_)))
            .map(|e| (e.c.x, e.c.y));
        match spark_owner {
            Some((ox, oy)) => {
                // JAVA: new Spark((AirWizard)sparkOwner, x, y) — x and y land in the
                // (double xa, double ya) constructor parameters; preserved quirk.
                let mut rnd = g.random.clone();
                let e = crate::entity::projectile::new_spark(
                    aw_id, ox, oy, x as f64, y as f64, &mut rnd,
                );
                g.random = rnd;
                Some(e)
            }
            None => {
                eprintln!("failed to load spark; owner id doesn't point to a correct entity");
                return None;
            }
        }
    } else {
        let mut mob_lvl = 1;
        // JAVA: Class.forName("fdoom.entity.mob."+entityName) EnemyMob check, guarded by
        // !Crafter.names.contains(entityName).
        let is_crafter_name = crate::entity::furniture::crafter::CrafterType::VALUES
            .iter()
            .any(|t| t.name() == entity_name);
        let is_enemy_mob_class = matches!(
            entity_name,
            "Zombie" | "Slime" | "Creeper" | "Skeleton" | "Knight" | "Snake" | "AirWizard"
        );
        if !is_crafter_name && is_enemy_mob_class {
            mob_lvl = info[info.len() - 2].parse().unwrap();
        }

        if mob_lvl == 0 {
            if g.debug {
                println!("level 0 mob: {entity_name}");
            }
            mob_lvl = 1;
        }

        // Java entityName.substring(entityName.lastIndexOf(".")+1).
        let simple_name = entity_name.rsplit('.').next().unwrap_or(entity_name);
        get_entity(g, simple_name, mob_lvl)
    };

    let mut new_entity = new_entity?;

    if new_entity.is_mob() {
        // JAVA: `&& !(newEntity instanceof RemotePlayer)` — no RemotePlayers here.
        new_entity.mob_mut().unwrap().health = info[2].parse().unwrap();
    } else if new_entity.is_chest() {
        let is_death_chest = matches!(new_entity.kind, EntityKind::DeathChest(_));
        let is_dungeon_chest = matches!(new_entity.kind, EntityKind::DungeonChest(_));
        let chest_info: Vec<String> = info[2..info.len() - 1].to_vec();

        let end_idx = chest_info.len()
            - if is_death_chest || is_dungeon_chest {
                1
            } else {
                0
            };
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
                let mut stack = crate::item::registry::get(g, &aitem_data[0]);
                if !matches!(stack.kind, crate::item::ItemKind::Unknown { .. }) {
                    stack.set_count(aitem_data[1].parse().unwrap());
                    new_entity.chest_mut().unwrap().inventory.add(stack);
                } else {
                    eprintln!(
                        "LOAD ERROR: encountered invalid item name, expected to be stackable: {}; stack trace:",
                        aitem_data[0]
                    );
                }
            } else {
                let item = crate::item::registry::get(g, &item_data);
                new_entity.chest_mut().unwrap().inventory.add(item);
            }
        }

        if is_death_chest {
            if let EntityKind::DeathChest(dc) = &mut new_entity.kind {
                dc.time = chest_info[chest_info.len() - 1].parse().unwrap();
            }
        } else if is_dungeon_chest {
            let is_locked = parse_bool(&chest_info[chest_info.len() - 1]);
            if let EntityKind::DungeonChest(dc) = &mut new_entity.kind {
                dc.is_locked = is_locked;
            }
            if is_locked {
                let lvl: usize = info[info.len() - 1].parse().unwrap();
                g.level_mut(lvl).chest_count += 1;
            }
        }
    } else if matches!(new_entity.kind, EntityKind::Spawner(_)) {
        let mob_name = info[2].rsplit('.').next().unwrap_or(&info[2]).to_string();
        let mob = get_entity(g, &mob_name, info[3].parse().unwrap());
        if let Some(mob) = mob {
            // JAVA: `(MobAi) getEntity(...)` — the cast is implicit in Spawner::new here.
            let mut rnd = g.random.clone();
            new_entity = crate::entity::furniture::spawner::new(mob, &mut rnd);
            g.random = rnd;
        }
    } else if matches!(new_entity.kind, EntityKind::Lantern(_))
        && *world_ver >= Version::new("1.9.4")
        && info.len() > 3
    {
        let t: usize = info[2].parse().unwrap();
        new_entity = crate::entity::furniture::lantern::new(
            crate::entity::furniture::lantern::LanternType::VALUES[t],
        );
    }

    if !is_local_save {
        // JAVA: these branches only run for entities received over the network.
        if matches!(new_entity.kind, EntityKind::Arrow(_)) {
            let owner_id: i32 = info[2].parse().unwrap();
            // JAVA: (Mob)Network.getEntity(ownerID) — arena lookup here.
            let owner_is_mob = g
                .entities
                .get(owner_id)
                .map(|e| e.is_mob())
                .unwrap_or(false);
            if owner_is_mob {
                let dir = crate::entity::Direction::VALUES[info[3].parse::<usize>().unwrap()];
                let dmg: i32 = info[5].parse().unwrap();
                new_entity = crate::entity::projectile::new_arrow(owner_id, x, y, dir, dmg);
            }
        }
        if matches!(new_entity.kind, EntityKind::ItemEntity(_)) {
            let item = crate::item::registry::get(g, &info[2]);
            let zz: f64 = info[3].parse().unwrap();
            let lifetime: i32 = info[4].parse().unwrap();
            let timeleft: i32 = info[5].parse().unwrap();
            let xa: f64 = info[6].parse().unwrap();
            let ya: f64 = info[7].parse().unwrap();
            let za: f64 = info[8].parse().unwrap();
            let mut rnd = g.random.clone();
            new_entity = crate::entity::item_entity::with_motion(
                item, x, y, zz, lifetime, timeleft, xa, ya, za, &mut rnd,
            );
            g.random = rnd;
        }
        if matches!(new_entity.kind, EntityKind::TextParticle(_)) {
            let textcol: i32 = info[3].parse().unwrap();
            let mut rnd = g.random.clone();
            new_entity =
                crate::entity::particle::new_text_particle(&info[2], x, y, textcol, &mut rnd);
            g.random = rnd;
        }
    }

    // this will be -1 unless set earlier, so a new one will be generated when adding it
    // to the level.
    new_entity.c.eid = eid;
    if matches!(new_entity.kind, EntityKind::ItemEntity(_)) && eid == -1 {
        println!("Warning: item entity was loaded with no eid");
    }

    let cur_level: usize = info[info.len() - 1].parse().unwrap();
    if g.levels[cur_level].is_some() {
        g.level_mut(cur_level)
            .add_at(new_entity, x, y, false, cur_level);
    }
    // JAVA: else prints for RemotePlayers on null levels — multiplayer stub.

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
        "Sheep" => Some(mob::sheep::new(g)),
        "Pig" => Some(mob::pig::new(g)),
        "Zombie" => Some(mob::zombie::new(g, moblvl)),
        "Slime" => Some(mob::slime::new(g, moblvl)),
        "GlowWorm" => Some(mob::glow_worm::new(g)),
        "Creeper" => Some(mob::creeper::new(g, moblvl)),
        "Skeleton" => Some(mob::skeleton::new(g, moblvl)),
        "Knight" => Some(mob::knight::new(g, moblvl)),
        "Snake" => Some(mob::snake::new(g, moblvl)),
        "AirWizard" => Some(mob::air_wizard::new(g, moblvl > 1)),
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
        "Bed" => Some(furniture::bed::new()),
        "Tnt" => Some(furniture::tnt::new()),
        "Lantern" => Some(furniture::lantern::new(
            furniture::lantern::LanternType::Norm,
        )),
        "Arrow" => {
            // JAVA: new Arrow(new Skeleton(0), 0, 0, Direction.NONE, 0) — the throwaway
            // Skeleton owner maps to owner eid -1 here.
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
        // JAVA: case "Spark" is commented out.
        "FireParticle" => Some(crate::entity::particle::new_fire_particle(0, 0)),
        "SmashParticle" => Some(crate::entity::particle::new_smash_particle(0, 0)),
        "TextParticle" => {
            let mut rnd = g.random.clone();
            let e = crate::entity::particle::new_text_particle("", 0, 0, 0, &mut rnd);
            g.random = rnd;
            Some(e)
        }
        _ => {
            eprintln!("LOAD ERROR: unknown or outdated entity requested: {string}");
            None
        }
    }
}
