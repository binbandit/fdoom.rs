//! Port of `fdoom.screen.WorldSelectDisplay` + `fdoom.screen.WorldEditDisplay`.
//!
//! The Java statics (`worldName`, `loadedWorld`) live on `Game`. `WorldEditDisplay`
//! (rename/copy/delete, a separate Java file) lives here as a second struct.
//! Java cached the scanned world list in a static (`refreshWorldNames`); the Rust port
//! rescans the saves folder on each query — same results, no stale-cache invalidation.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use crate::core::file_handler;
use crate::core::game::{self, Game};
use crate::gfx::{Screen, color, font};
use crate::saveload::save::EXTENSION as SAVE_EXTENSION;
use crate::saveload::version::Version;

use super::display::{Display, DisplayBase, display_render_default, display_tick_default};
use super::entry::input_entry::InputEntry;
use super::entry::{
    COL_SLCT, COL_UNSLCT, EntryFlags, EntryHandle, ListEntry, SelectEntry, StringEntry, handle,
};
use super::loading_display::LoadingDisplay;
use super::menu::MenuBuilder;
use super::rel_pos::RelPos;
use super::title_display::TitleDisplay;

/// Java `WorldSelectDisplay.getWorldName()`.
pub fn get_world_name(g: &Game) -> String {
    g.world_name.clone()
}

/// Java `WorldSelectDisplay.setWorldName(world, loaded)`.
pub fn set_world_name(g: &mut Game, name: &str, load: bool) {
    g.world_name = name.to_string();
    g.loaded_world = load;
}

/// Java `WorldSelectDisplay.loadedWorld()`.
pub fn loaded_world(g: &Game) -> bool {
    g.loaded_world
}

/// The most recently saved world, for the title's Continue entry (newest Game file).
pub fn most_recent_world(g: &Game) -> Option<String> {
    let saves = g.game_dir.join("saves");
    let mut best: Option<(String, std::time::SystemTime)> = None;
    for name in get_world_names(g) {
        let game_file = saves
            .join(name.to_lowercase())
            .join(format!("Game{}", crate::saveload::save::EXTENSION));
        if let Ok(meta) = std::fs::metadata(&game_file) {
            if let Ok(modified) = meta.modified() {
                if best.as_ref().map(|(_, t)| modified > *t).unwrap_or(true) {
                    best = Some((name, modified));
                }
            }
        }
    }
    best.map(|(name, _)| name)
}

/// Java `WorldSelectDisplay.getWorldNames()`.
pub fn get_world_names(g: &Game) -> Vec<String> {
    scan_worlds(g).into_iter().map(|(name, _)| name).collect()
}

/// The find-worlds init step of Java `getWorldNames(recalc)` — scans `gameDir/saves/`.
fn scan_worlds(g: &Game) -> Vec<(String, Version)> {
    let worlds_dir = g.game_dir.join("saves");
    let _ = std::fs::create_dir_all(&worlds_dir); // JAVA: folder.mkdirs()

    let mut out = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(&worlds_dir) else {
        eprintln!("ERROR: Game location file folder is null, somehow...");
        return out;
    };

    // JAVA: File.listFiles() order is filesystem-dependent; sort for determinism.
    let mut paths: Vec<std::path::PathBuf> = read_dir.flatten().map(|e| e.path()).collect();
    paths.sort();

    for path in paths {
        if path.is_dir() {
            let mut files: Vec<String> = std::fs::read_dir(&path)
                .map(|rd| {
                    rd.flatten()
                        .map(|e| e.file_name().to_string_lossy().into_owned())
                        .collect()
                })
                .unwrap_or_default();
            files.sort();
            // JAVA: only the first file's extension is checked.
            if files
                .first()
                .map(|f| f.ends_with(SAVE_EXTENSION))
                .unwrap_or(false)
            {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if g.debug {
                    println!("World found: {name}");
                }
                out.push((name, load_world_version(&path)));
            }
        }
    }

    out
}

/// Java `new Load(name, false).getWorldVersion()` — reads the first CSV token of the
/// world's `Game` save file. TODO(port:saveload): fold into `Load` once it's ported.
fn load_world_version(world_dir: &Path) -> Version {
    let content = std::fs::read_to_string(world_dir.join(format!("Game{SAVE_EXTENSION}")))
        .unwrap_or_default();
    // JAVA: Load concatenates the file's lines with no separator, then splits on ",".
    let content: String = content.lines().collect();
    let first = content.split(',').next().unwrap_or("");
    if first.contains('.') {
        Version::new(first)
    } else {
        Version::new("1.8")
    }
}

/// Java `WorldSelectDisplay.Action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Copy,
    Rename,
    Delete,
}

impl Action {
    pub const VALUES: [Action; 3] = [Action::Copy, Action::Rename, Action::Delete];

    pub fn key(self) -> &'static str {
        match self {
            Action::Copy => "C",
            Action::Rename => "R",
            Action::Delete => "D",
        }
    }

    pub fn color(self) -> i32 {
        match self {
            Action::Copy => color::get(-1, 5),
            Action::Rename => color::get(-1, 50),
            Action::Delete => color::get(-1, 500),
        }
    }

    /// Java `Action.toString()` (the enum constant's name).
    pub fn name(self) -> &'static str {
        match self {
            Action::Copy => "Copy",
            Action::Rename => "Rename",
            Action::Delete => "Delete",
        }
    }
}

/// Java's anonymous `SelectEntry` override in `WorldSelectDisplay.init`: a world row that
/// takes the pending action's color while selected.
struct WorldEntry {
    inner: SelectEntry,
    cur_action: Rc<RefCell<Option<Action>>>,
}

impl ListEntry for WorldEntry {
    fn flags(&self) -> EntryFlags {
        self.inner.flags()
    }

    fn flags_mut(&mut self) -> &mut EntryFlags {
        self.inner.flags_mut()
    }

    fn tick(&mut self, g: &mut Game) {
        self.inner.tick(g);
    }

    fn to_display_string(&self, g: &Game) -> String {
        self.inner.to_display_string(g)
    }

    fn get_color(&self, is_selected: bool) -> i32 {
        match (*self.cur_action.borrow(), is_selected) {
            (Some(action), true) => action.color(),
            _ => {
                if is_selected {
                    COL_SLCT
                } else {
                    COL_UNSLCT
                }
            }
        }
    }
}

/// NOTE (JAVA): "this will only be responsible for the world load selection screen."
pub struct WorldSelectDisplay {
    base: DisplayBase,
    cur_action: Rc<RefCell<Option<Action>>>,
    world_versions: Vec<Version>,
}

impl Default for WorldSelectDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldSelectDisplay {
    pub fn new() -> WorldSelectDisplay {
        WorldSelectDisplay {
            base: DisplayBase::new(true, true, Vec::new()),
            cur_action: Rc::new(RefCell::new(None)),
            world_versions: Vec::new(),
        }
    }
}

impl Display for WorldSelectDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn init(&mut self, g: &mut Game) {
        // JAVA: super.init(parent instanceof TitleDisplay ? parent : new TitleDisplay());
        // with the explicit display stack, exiting returns to whatever opened this screen.
        g.world_name = String::new();
        g.loaded_world = true;

        let worlds = scan_worlds(g);
        self.world_versions = worlds.iter().map(|(_, v)| v.clone()).collect();

        let mut entries: Vec<EntryHandle> = Vec::new();
        for (name, version) in worlds {
            let cur_action = self.cur_action.clone();
            let entry_name = name.clone();
            entries.push(handle(WorldEntry {
                cur_action: self.cur_action.clone(),
                inner: SelectEntry::with_localize(
                    &name,
                    move |g: &mut Game| {
                        let action = *cur_action.borrow();
                        match action {
                            None => {
                                if version > game::version() {
                                    return; // cannot load a game saved by a higher version!
                                }
                                g.world_name = entry_name.clone();
                                g.set_menu(LoadingDisplay::new());
                            }
                            Some(action) => {
                                g.set_menu(WorldEditDisplay::new(g, action, &entry_name));
                                *cur_action.borrow_mut() = None;
                            }
                        }
                    },
                    false,
                ),
            }));
        }

        let menu = MenuBuilder::new(false, 0, RelPos::Center, entries)
            .set_display_length(5)
            .set_scroll_policies(1.0, true)
            .create_menu(g);

        self.base.menus = vec![menu];
        self.base.selection = 0;
    }

    fn tick(&mut self, g: &mut Game) {
        display_tick_default(&mut self.base, g);

        if self.cur_action.borrow().is_some() {
            return;
        }

        for action in Action::VALUES {
            if g.input.get_key(action.key()).clicked {
                *self.cur_action.borrow_mut() = Some(action);
                break;
            }
        }
    }

    fn render(&mut self, screen: &mut Screen, g: &mut Game) {
        display_render_default(&mut self.base, screen, g);

        let sel = self.base.menus[0].get_selection();
        if sel >= 0 && (sel as usize) < self.world_versions.len() {
            let version = &self.world_versions[sel as usize];
            let mut col = color::WHITE;
            if *version > game::version() {
                col = color::RED;
                font::draw_centered(
                    "Higher version, cannot load world",
                    screen,
                    font::text_height() * 5,
                    col,
                );
            }
            font::draw_centered(
                &format!(
                    "World Version: {}{}",
                    if *version <= Version::new("1.9.2") {
                        "~"
                    } else {
                        ""
                    },
                    version
                ),
                screen,
                font::text_height() * 7 / 2,
                col,
            );
        }

        font::draw_centered(
            &format!("{} to confirm", g.input.get_mapping("select")),
            screen,
            crate::gfx::screen::H - 60,
            color::GRAY,
        );
        font::draw_centered(
            &format!("{} to return", g.input.get_mapping("exit")),
            screen,
            crate::gfx::screen::H - 40,
            color::GRAY,
        );

        let mut title = "Select World".to_string();
        let mut title_color = color::WHITE;

        match *self.cur_action.borrow() {
            None => {
                let mut y =
                    crate::gfx::screen::H - font::text_height() * Action::VALUES.len() as i32;

                for action in Action::VALUES {
                    font::draw_centered(
                        &format!("{} to {}", action.key(), action.name()),
                        screen,
                        y,
                        action.color(),
                    );
                    y += font::text_height();
                }
            }
            Some(action) => {
                title = format!("Select a World to {}", action.name());
                title_color = action.color();
            }
        }

        font::draw_centered(&title, screen, 0, title_color);
    }
}

/// Port of `fdoom.screen.WorldEditDisplay` — "will be used to enact the extra actions
/// (copy, delete, rename) that you can do for worlds in the WorldSelectMenu."
pub struct WorldEditDisplay {
    base: DisplayBase,
    action: Action,
    world_name: String,
    /// The name-input row (Java downcast `menus[0].getCurEntry()` to reach it).
    input_entry: Option<Rc<RefCell<InputEntry>>>,
}

impl WorldEditDisplay {
    pub fn new(g: &Game, action: Action, world_name: &str) -> WorldEditDisplay {
        let mut entries: Vec<EntryHandle> = Vec::new();
        let mut input_entry = None;

        if action != Action::Delete {
            let mut names = get_world_names(g);
            if action == Action::Rename {
                names.retain(|n| n != world_name);
            }
            entries.push(handle(StringEntry::with_color(
                "New World Name:",
                action.color(),
            )));
            let entry: Rc<RefCell<InputEntry>> = Rc::new(RefCell::new(
                super::world_gen_display::make_world_name_input("", names, world_name),
            ));
            entries.push(entry.clone() as EntryHandle);
            input_entry = Some(entry);
        } else {
            entries.push(handle(StringEntry::with_color(
                "Are you sure you want to delete",
                action.color(),
            )));
            entries.push(handle(StringEntry::with_color(
                &format!("\"{world_name}\"?"),
                color::tint(action.color(), 1, true),
            )));
            entries.push(handle(StringEntry::with_color(
                "This can not be undone!",
                action.color(),
            )));
        }

        entries.extend(StringEntry::use_lines_color(
            color::WHITE,
            &[
                String::new(),
                format!("{} to confirm", g.input.get_mapping("select")),
                format!("{} to cancel", g.input.get_mapping("exit")),
            ],
        ));

        let menu = MenuBuilder::new(false, 8, RelPos::Center, entries).create_menu(g);

        WorldEditDisplay {
            base: DisplayBase::new(true, true, vec![menu]),
            action,
            world_name: world_name.to_string(),
            input_entry,
        }
    }
}

impl Display for WorldEditDisplay {
    fn base(&self) -> &DisplayBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut DisplayBase {
        &mut self.base
    }

    fn tick(&mut self, g: &mut Game) {
        display_tick_default(&mut self.base, g);

        if g.input.get_key("select").clicked {
            // do action
            let worlds_dir = g.game_dir.join("saves");
            let world = worlds_dir.join(&self.world_name);
            match self.action {
                Action::Delete => {
                    if g.debug {
                        println!("deleting world: {}", world.display());
                    }
                    if let Err(e) = std::fs::remove_dir_all(&world) {
                        eprintln!("could not delete world {}: {e}", world.display());
                    }

                    if !get_world_names(g).is_empty() {
                        g.set_menu(WorldSelectDisplay::new());
                    } else {
                        g.set_menu(TitleDisplay::new(g));
                    }
                }

                Action::Copy => {
                    let entry = self
                        .input_entry
                        .as_ref()
                        .expect("copy action has an input entry");
                    if !entry.borrow().is_valid() {
                        return;
                    }
                    // user hits enter with a valid new name; copy is created here.
                    let newname = entry.borrow().get_user_input();
                    let newworld = worlds_dir.join(&newname);
                    let _ = std::fs::create_dir_all(&newworld);
                    if g.debug {
                        println!(
                            "copying world {} to world {}",
                            world.display(),
                            newworld.display()
                        );
                    }
                    // walk file tree
                    if let Err(e) = file_handler::copy_folder_contents(
                        &world,
                        &newworld,
                        file_handler::REPLACE_EXISTING,
                        false,
                        g.debug,
                    ) {
                        eprintln!("{e}");
                    }

                    g.set_menu(WorldSelectDisplay::new());
                }

                Action::Rename => {
                    let entry = self
                        .input_entry
                        .as_ref()
                        .expect("rename action has an input entry");
                    if !entry.borrow().is_valid() {
                        return;
                    }
                    // user hits enter with a valid new name; name is set here:
                    let name = entry.borrow().get_user_input();
                    if g.debug {
                        println!("renaming world {} to new name: {name}", world.display());
                    }
                    if let Err(e) = std::fs::rename(&world, worlds_dir.join(&name)) {
                        eprintln!("could not rename world {}: {e}", world.display());
                    }
                    g.set_menu(WorldSelectDisplay::new());
                }
            }
        }
        // Display class will take care of exiting
    }
}
