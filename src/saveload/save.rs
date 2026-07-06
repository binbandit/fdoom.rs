//! Port of `fdoom.saveload.Save`.
//!
//! Java's constructor overloads become free functions: `new Save(worldname)` →
//! [`save_world_named`], `new Save()` → [`save_prefs`], `new Save(player, writePlayer)` →
//! [`save_player`]. The `Save(worldname, GameServer)` server-config overload is not ported
//! (the network layer is a stub; see PORTING.md "Multiplayer").

use std::io::Write as _;
use std::path::PathBuf;

use crate::core::game::Game;
use crate::entity::{Entity, EntityKind};
use crate::screen::entry::array_entry::Value;

/// Java `Save.extension`.
pub const EXTENSION: &str = ".miniplussave";

/// Java `Save`'s instance state (`location` + the `data` list shared by the writers).
struct Save {
    location: String,
    data: Vec<String>,
}

impl Save {
    /// Java `private Save(File worldFolder)`.
    fn new_at(world_folder: PathBuf, debug: bool) -> Save {
        let mut world_folder = world_folder;

        // JAVA: worldFolder.getParent().equals("saves") — only true for a relative gameDir.
        if world_folder
            .parent()
            .map(|p| p.as_os_str() == "saves")
            .unwrap_or(false)
        {
            let world_name = world_folder
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if world_name.to_lowercase() != world_name {
                if debug {
                    println!("renaming world in {} to lowercase", world_folder.display());
                }
                let path = world_folder.to_string_lossy().to_string();
                let path = path[..path.rfind(&world_name).unwrap_or(0)].to_string();
                let new_folder = PathBuf::from(format!("{}{}", path, world_name.to_lowercase()));
                if std::fs::rename(&world_folder, &new_folder).is_ok() {
                    world_folder = new_folder;
                } else {
                    eprintln!(
                        "failed to rename world folder {} to {}",
                        world_folder.display(),
                        new_folder.display()
                    );
                }
            }
        }

        let location = format!("{}/", world_folder.display());
        let _ = std::fs::create_dir_all(&world_folder); // Java folder.mkdirs()

        Save {
            location,
            data: Vec::new(),
        }
    }

    /// Java `writeToFile(String filename, List<String> savedata)` (instance method).
    fn write_to_file(&mut self, g: &mut Game, filename: &str) {
        if let Err(ex) = write_to_file(filename, &self.data, true) {
            eprintln!("{ex}"); // Java ex.printStackTrace()
        }

        self.data.clear();

        // Java LoadingDisplay.progress(7) + clamp.
        g.loading_percentage = (g.loading_percentage + 7.0).min(100.0);
        if g.loading_percentage > 100.0 {
            g.loading_percentage = 100.0;
        }

        // JAVA: Renderer.render() — "AH HA!!! HERE'S AN IMPORTANT STATEMENT!!!!". The
        // renderer lives outside `Game` in this port; the save-progress frame is drawn by
        // the platform render loop instead.
    }

    /// Java `writeGame(filename)`.
    fn write_game(&mut self, g: &mut Game, filename: &str) {
        self.data.push(crate::core::game::version().to_string());
        self.data.push(format!(
            "{}{}",
            g.settings.get_idx("mode"),
            if g.is_mode("score") {
                format!(
                    ";{};{}",
                    g.score_time,
                    g.settings.get("scoretime").to_display()
                )
            } else {
                String::new()
            }
        ));
        self.data.push(g.tick_count.to_string());
        self.data.push(g.game_time.to_string());
        self.data.push(g.settings.get_idx("diff").to_string());
        self.data.push(g.air_wizard_beaten.to_string());
        let file = format!("{}{}{}", self.location, filename, EXTENSION);
        self.write_to_file(g, &file);
    }

    /// Java `writePrefs()`.
    fn write_prefs(&mut self, g: &mut Game) {
        self.data.push(crate::core::game::version().to_string());
        self.data.push(g.settings.get("sound").to_display());
        self.data.push(g.settings.get("autosave").to_display());
        self.data.push(g.settings.get("fps").to_display());
        // JAVA: MultiplayerDisplay.savedIP / savedUUID / savedUsername — the multiplayer
        // display is a stub in this build, so these statics are always "".
        self.data.push(String::new());
        self.data.push(String::new());
        self.data.push(String::new());
        self.data
            .push(g.localization.get_selected_language().to_string());

        let key_pairs = g.input.get_key_prefs(g.debug);
        self.data.push(key_pairs.join(":"));

        let file = format!("{}Preferences{}", self.location, EXTENSION);
        self.write_to_file(g, &file);

        if g.settings.get("unlockedskin").as_bool() {
            self.data.push("AirSkin".to_string());
        }

        if g.settings
            .get_entry("scoretime")
            .borrow()
            .get_value_visibility(&Value::Int(10))
        {
            self.data.push("10_ScoreTime".to_string());
        }
        if g.settings
            .get_entry("scoretime")
            .borrow()
            .get_value_visibility(&Value::Int(120))
        {
            self.data.push("120_ScoreTime".to_string());
        }

        let file = format!("{}Unlocks{}", self.location, EXTENSION);
        self.write_to_file(g, &file);
    }

    /// Java `writeWorld(filename)`.
    fn write_world(&mut self, g: &mut Game, filename: &str) {
        g.loading_message = "Levels".to_string(); // Java LoadingDisplay.setMessage
        for l in 0..g.levels.len() {
            // JAVA: writes the "size" *setting* for w and h, not the level's actual size.
            let world_size = g.settings.get("size").to_display();
            self.data.push(world_size.clone());
            self.data.push(world_size);
            self.data.push(g.level(l).depth.to_string());

            let (w, h) = (g.level(l).w, g.level(l).h);
            for x in 0..w {
                for y in 0..h {
                    self.data.push(g.tile_at(l, x, y).name.clone());
                }
            }

            let file = format!("{}{}{}{}", self.location, filename, l, EXTENSION);
            self.write_to_file(g, &file);
        }

        for l in 0..g.levels.len() {
            let (w, h) = (g.level(l).w, g.level(l).h);
            for x in 0..w {
                for y in 0..h {
                    self.data.push(g.level(l).get_data(x, y).to_string());
                }
            }

            let file = format!("{}{}{}data{}", self.location, filename, l, EXTENSION);
            self.write_to_file(g, &file);
        }
    }

    /// Java `writePlayer(String filename, Player player)`.
    fn write_player(&mut self, g: &mut Game, filename: &str) {
        g.loading_message = "Player".to_string(); // Java LoadingDisplay.setMessage
        {
            let player = g.player();
            write_player(g, player, &mut self.data);
        }
        let file = format!("{}{}{}", self.location, filename, EXTENSION);
        self.write_to_file(g, &file);
    }

    /// Java `writeInventory(String filename, Player player)`.
    fn write_inventory(&mut self, g: &mut Game, filename: &str) {
        write_inventory(g.player(), &mut self.data);
        let file = format!("{}{}{}", self.location, filename, EXTENSION);
        self.write_to_file(g, &file);
    }

    /// Java `writeEntities(filename)`.
    fn write_entities(&mut self, g: &mut Game, filename: &str) {
        g.loading_message = "Entities".to_string(); // Java LoadingDisplay.setMessage
        for l in 0..g.levels.len() {
            // Java Level.getEntitiesToSave(): current entities + entitiesToAdd.
            let g_ref: &Game = g;
            for e in g_ref.entities.entities_on_level(l) {
                let saved = write_entity(g_ref, e, true);
                if !saved.is_empty() {
                    self.data.push(saved);
                }
            }
            for e in &g_ref.level(l).entities_to_add {
                let saved = write_entity(g_ref, e, true);
                if !saved.is_empty() {
                    self.data.push(saved);
                }
            }
        }

        let file = format!("{}{}{}", self.location, filename, EXTENSION);
        self.write_to_file(g, &file);
    }
}

/// Java `new Save(worldname)` — saves the whole world.
pub fn save_world_named(g: &mut Game, world_name: &str) {
    let folder = PathBuf::from(format!("{}/saves/{}", g.game_dir.display(), world_name));
    let mut save = Save::new_at(folder, g.debug);

    if g.is_valid_client() {
        // clients are not allowed to save.
        g.saving = false;
        return;
    }

    save.write_game(g, "Game");
    save.write_world(g, "Level");
    if !g.is_valid_server() {
        // this must be waited for on a server.
        save.write_player(g, "Player");
        save.write_inventory(g, "Inventory");
    }
    save.write_entities(g, "Entities");

    // JAVA: WorldSelectDisplay.refreshWorldNames() — the world-select display re-reads the
    // saves directory when opened in this port.

    g.notify_all("World Saved!");
    g.as_tick = 0;
    g.saving = false;
}

/// Java `new Save()` — saves the global options (preferences + unlocks).
pub fn save_prefs(g: &mut Game) {
    let folder = g.game_dir.clone();
    let mut save = Save::new_at(folder, g.debug);
    if g.debug {
        println!("writing preferences and unlocks...");
    }
    save.write_prefs(g);
}

/// Java `new Save(player, writePlayer)` — saves the main player (and their inventory)
/// into the currently selected world.
pub fn save_player(g: &mut Game, write_player: bool) {
    let world_name = crate::screen::world_select::get_world_name(g);
    let folder = PathBuf::from(format!("{}/saves/{}", g.game_dir.display(), world_name));
    let mut save = Save::new_at(folder, g.debug);
    if write_player {
        save.write_player(g, "Player");
        save.write_inventory(g, "Inventory");
    }
}

/// Java static `Save.writeFile(filename, lines)` — newline-joined, no trailing separator.
pub fn write_file(filename: &str, lines: &[String]) -> std::io::Result<()> {
    let mut file = std::fs::File::create(filename)?;
    file.write_all(lines.join("\n").as_bytes())?; // Java System.lineSeparator()
    Ok(())
}

/// Java static `Save.writeToFile(filename, savedata, isWorldSave)`.
///
/// World saves are one long line of comma-terminated values (every value gets a trailing
/// `","`); non-world saves get a `"\n"` after every value instead.
pub fn write_to_file(
    filename: &str,
    savedata: &[String],
    is_world_save: bool,
) -> std::io::Result<()> {
    let mut out = String::new();
    for (i, value) in savedata.iter().enumerate() {
        out.push_str(value);
        if is_world_save {
            out.push(',');
            // JAVA: the Level5 files get one extra trailing comma.
            if filename.contains("Level5") && i == savedata.len() - 1 {
                out.push(',');
            }
        } else {
            out.push('\n');
        }
    }
    let mut file = std::fs::File::create(filename)?;
    file.write_all(out.as_bytes())?;
    Ok(())
}

/// Java static `Save.writePlayer(player, data)`.
pub fn write_player(g: &Game, player: &Entity, data: &mut Vec<String>) {
    let pd = player.player();
    data.clear();
    data.push(player.c.x.to_string());
    data.push(player.c.y.to_string());
    data.push(pd.spawnx.to_string());
    data.push(pd.spawny.to_string());
    data.push(pd.mob.health.to_string());
    data.push(pd.hunger.to_string());
    data.push(pd.armor.to_string());
    if let Some(cur_armor) = &pd.cur_armor {
        data.push(pd.armor_damage_buffer.to_string());
        data.push(cur_armor.get_name().to_string());
    }
    data.push(pd.get_score().to_string());
    data.push(g.current_level.to_string());

    let mut subdata = String::from("PotionEffects[");

    for (ptype, duration) in &pd.potioneffects {
        subdata.push_str(&format!("{ptype};{duration}:"));
    }

    if !pd.potioneffects.is_empty() {
        // cuts off extra ":" and appends "]"
        subdata.truncate(subdata.len() - 1);
        subdata.push(']');
    } else {
        subdata.push(']');
    }
    data.push(subdata);

    data.push(pd.shirt_color.to_string());
    data.push(pd.skinon.to_string());
}

/// Java static `Save.writeInventory(player, data)`.
pub fn write_inventory(player: &Entity, data: &mut Vec<String>) {
    let pd = player.player();
    data.clear();
    if let Some(active_item) = &pd.active_item {
        data.push(active_item.get_data());
    }

    let inventory = &pd.inventory;

    for i in 0..inventory.inv_size() {
        data.push(inventory.get(i).get_data());
    }
}

/// The Java class simple name for an entity (Java `e.getClass().getName()` tail).
fn entity_class_name(e: &Entity) -> &'static str {
    match &e.kind {
        EntityKind::Player(_) => "Player",
        EntityKind::Cow(_) => "Cow",
        EntityKind::Pig(_) => "Pig",
        EntityKind::Sheep(_) => "Sheep",
        EntityKind::GlowWorm(_) => "GlowWorm",
        EntityKind::Zombie(_) => "Zombie",
        EntityKind::Slime(_) => "Slime",
        EntityKind::Creeper(_) => "Creeper",
        EntityKind::Skeleton(_) => "Skeleton",
        EntityKind::Snake(_) => "Snake",
        EntityKind::Knight(_) => "Knight",
        EntityKind::AirWizard(_) => "AirWizard",
        EntityKind::ItemEntity(_) => "ItemEntity",
        EntityKind::Arrow(_) => "Arrow",
        EntityKind::Spark(_) => "Spark",
        // JAVA: FireParticle/SmashParticle are separate classes; both map to the merged
        // Particle kind here. Particles are never written to local saves.
        EntityKind::Particle(_) => "Particle",
        EntityKind::TextParticle(_) => "TextParticle",
        EntityKind::Furniture(_) => "Furniture",
        EntityKind::Chest(_) => "Chest",
        EntityKind::DeathChest(_) => "DeathChest",
        EntityKind::DungeonChest(_) => "DungeonChest",
        EntityKind::Bed(_) => "Bed",
        EntityKind::Crafter(_) => "Crafter",
        EntityKind::Lantern(_) => "Lantern",
        EntityKind::Spawner(_) => "Spawner",
        EntityKind::Tnt(_) => "Tnt",
    }
}

/// Java static `Save.writeEntity(e, isLocalSave)`.
pub fn write_entity(g: &Game, e: &Entity, is_local_save: bool) -> String {
    let mut name = entity_class_name(e).to_string();
    let mut extradata = String::new();

    // don't even write ItemEntities or particle effects; Spark... will probably is saved,
    // eventually; it presents an unfair cheat to remove the sparks by reloading the Game.

    if is_local_save
        && matches!(
            e.kind,
            EntityKind::ItemEntity(_)
                | EntityKind::Arrow(_)
                | EntityKind::Spark(_)
                | EntityKind::Particle(_)
                | EntityKind::TextParticle(_)
        )
    {
        // JAVA: RemotePlayer is also skipped here; no such entity exists in this build.
        return String::new();
    }

    if !is_local_save {
        extradata.push_str(&format!(":{}", e.c.eid));
    }

    // JAVA: `!isLocalSave && e instanceof RemotePlayer` writes rp.getData() — multiplayer
    // stub, unreachable. The "else" is so that RemotePlayer doesn't get the health thing.
    if let Some(m) = e.mob() {
        extradata.push_str(&format!(":{}", m.health));
        if let Some(em) = e.enemy_mob() {
            extradata.push_str(&format!(":{}", em.lvl));
        }
    }

    if let Some(chest) = e.chest() {
        for ii in 0..chest.inventory.inv_size() {
            let item = chest.inventory.get(ii);
            extradata.push_str(&format!(":{}", item.get_name()));
            if item.is_stackable() {
                extradata.push_str(&format!(";{}", chest.inventory.count(item)));
            }
        }

        if let EntityKind::DeathChest(dc) = &e.kind {
            extradata.push_str(&format!(":{}", dc.time));
        }
        if let EntityKind::DungeonChest(dc) = &e.kind {
            extradata.push_str(&format!(":{}", dc.is_locked));
        }
    }

    if let EntityKind::Spawner(egg) = &e.kind {
        let mobname = entity_class_name(&egg.mob);
        extradata.push_str(&format!(
            ":{}:{}",
            mobname,
            match egg.mob.enemy_mob() {
                Some(em) => em.lvl,
                None => 1,
            }
        ));
    }

    if let EntityKind::Lantern(l) = &e.kind {
        // Java Lantern.Type ordinal.
        let ordinal = crate::entity::furniture::lantern::LanternType::VALUES
            .iter()
            .position(|t| *t == l.lantern_type)
            .unwrap_or(0);
        extradata.push_str(&format!(":{ordinal}"));
    }

    if let EntityKind::Crafter(c) = &e.kind {
        name = c.crafter_type.name().to_string();
    }

    if !is_local_save {
        // JAVA: these are only written when *sending* a world (multiplayer), not saving it.
        if let EntityKind::ItemEntity(ie) = &e.kind {
            extradata.push_str(&format!(":{}", crate::entity::item_entity::get_data(ie)));
        }
        if let EntityKind::Arrow(a) = &e.kind {
            // Java Arrow.getData(): owner.eid + ":" + dir.ordinal() + ":" + damage.
            extradata.push_str(&format!(":{}:{}:{}", a.owner, a.dir.ordinal(), a.damage));
        }
        if let EntityKind::Spark(s) = &e.kind {
            // Java Spark.getData(): owner.eid.
            extradata.push_str(&format!(":{}", s.owner));
        }
        if let EntityKind::TextParticle(tp) = &e.kind {
            // JAVA: TextParticle.getData() is msg + ":" + style.getColor(); FontStyle does
            // not expose its color in this port and this branch is multiplayer-only.
            extradata.push_str(&format!(":{}", tp.msg));
        }
    }

    let depth = match e.c.level {
        None => {
            println!(
                "WARNING: saving entity with no level reference: {name}; setting level to surface"
            );
            0
        }
        Some(lvl) => g.level(lvl).depth,
    };

    extradata.push_str(&format!(":{}", crate::level::lvl_idx(depth)));

    format!("{}[{}:{}{}]", name, e.c.x, e.c.y, extradata)
}
