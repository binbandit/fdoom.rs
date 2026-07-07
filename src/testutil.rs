//! Test support for headless integration tests (and nothing else — the game never
//! calls in here).
//!
//! The entry point is [`TestWorld`]: a builder that boots a real `Game` with a world
//! generated and the first tick done, plus the helpers every test used to copy-paste
//! (staging tiles, hitting/interacting, biome hunting, PNG dumps into `target/verify`).
//!
//! ```no_run
//! use fdoom::testutil::TestWorld;
//!
//! let mut tw = TestWorld::infinite().seed(42).creative().build();
//! tw.place("tall grass", 1, 0);       // one tile right of the player
//! assert!(tw.hit(1, 0, 1));           // bare-handed hit breaks it
//! tw.press("E");                      // tap a key for one tick
//! assert!(tw.display.menu_active());  // TestWorld derefs to Game
//! ```

use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::core::game::Game;
use crate::core::renderer::Renderer;
use crate::core::world;
use crate::entity::{Direction, EntityKind};
use crate::gfx::screen;
use crate::item::{Recipe, registry};
use crate::level::infinite_gen::{Biome, biome_at};
use crate::level::tile::dispatch;

/// A headless `Game` plus the helpers tests keep needing. Derefs to [`Game`], so
/// anything not covered here is one `tw.` away (`tw.tick()`, `tw.player()`, ...).
pub struct TestWorld {
    pub g: Game,
    renderer: Option<Renderer>,
}

/// Configures and boots a [`TestWorld`]; see [`TestWorld::infinite`].
pub struct TestWorldBuilder {
    seed: i64,
    name: Option<String>,
    creative: bool,
    debug: bool,
}

static NEXT_WORLD: AtomicUsize = AtomicUsize::new(0);

impl TestWorld {
    /// A fresh world (all new worlds are infinite/chunked; the finite sky/dungeon
    /// set-piece levels exist alongside as always).
    pub fn infinite() -> TestWorldBuilder {
        TestWorldBuilder {
            seed: 20260707,
            name: None,
            creative: false,
            debug: false,
        }
    }

    /* ------------------------------- ticking ------------------------------- */

    /// Tick `n` times.
    pub fn tick_n(&mut self, n: usize) {
        for _ in 0..n {
            self.g.tick();
        }
    }

    /// Tick once, then recover from whatever the tick produced: close any menu that
    /// opened (pause/death/level transition) so the level keeps ticking headlessly,
    /// and respawn the player if it died (what the death display's "respawn" does).
    pub fn tick_recover(&mut self) {
        self.g.tick();
        if self.g.display.menu_active() {
            self.g.clear_menu();
        }
        let player_gone = self.g.try_player().map(|p| p.c.removed).unwrap_or(true);
        if player_gone {
            world::reset_game(&mut self.g, true);
        }
    }

    /// Tap a key like the platform layer would: press + tick, release + tick.
    pub fn press(&mut self, key: &str) {
        self.g.input.key_toggled(key, true);
        self.g.tick();
        self.g.input.key_toggled(key, false);
        self.g.tick();
    }

    /// Hold a key down (until [`release`](Self::release)); takes effect on the next tick.
    pub fn hold(&mut self, key: &str) {
        self.g.input.key_toggled(key, true);
    }

    /// Release a key held with [`hold`](Self::hold).
    pub fn release(&mut self, key: &str) {
        self.g.input.key_toggled(key, false);
    }

    /* ------------------------------- the player ------------------------------- */

    /// Player position in pixels.
    pub fn player_pos(&self) -> (i32, i32) {
        let p = self.g.player();
        (p.c.x, p.c.y)
    }

    /// Player position in tile coordinates.
    pub fn player_tile(&self) -> (i32, i32) {
        let (x, y) = self.player_pos();
        (x >> 4, y >> 4)
    }

    /// Move the player to the center of tile `(tx, ty)` on the current level.
    pub fn teleport(&mut self, tx: i32, ty: i32) {
        let p = self.g.player_mut();
        p.c.x = tx * 16 + 8;
        p.c.y = ty * 16 + 8;
    }

    /// Teleport to the nearest tile of `want` (infinite worlds) and settle a few ticks
    /// so the chunks around it stream in. Returns the tile reached.
    pub fn goto_biome(&mut self, want: Biome) -> (i32, i32) {
        let (tx, ty) = find_biome(self.g.world_seed, want);
        self.teleport(tx, ty);
        self.tick_n(8);
        (tx, ty)
    }

    /// Add `n` of a registry item to the player's inventory.
    pub fn give(&mut self, item: &str, n: i32) {
        let item = registry::get(&self.g, &format!("{item}_{n}"));
        self.g
            .with_entity(self.g.player_id, |e, _g| e.player_mut().inventory.add(item))
            .expect("player entity missing");
    }

    /* ------------------------------- tiles ------------------------------- */

    /// Set the tile at player + `(dx, dy)` (tile coords) to `tile` with its default
    /// data, and return that position.
    pub fn place(&mut self, tile: &str, dx: i32, dy: i32) -> (i32, i32) {
        let (px, py) = self.player_tile();
        self.place_at(tile, px + dx, py + dy);
        (px + dx, py + dy)
    }

    /// Set the tile at `(tx, ty)` on the current level to `tile` with its default data.
    pub fn place_at(&mut self, tile: &str, tx: i32, ty: i32) {
        let def = self.g.tiles.get(tile);
        let lvl = self.g.current_level;
        self.g.set_tile_default(lvl, tx, ty, &def);
    }

    /// Hit the tile at player + `(dx, dy)` the way a bare-handed player attack does
    /// (`dispatch::hurt_by`). Returns whether the tile reacted.
    pub fn hit(&mut self, dx: i32, dy: i32, dmg: i32) -> bool {
        let (px, py) = self.player_tile();
        let (tx, ty) = (px + dx, py + dy);
        let lvl = self.g.current_level;
        let def = self.g.tile_at(lvl, tx, ty);
        let mut player = self.g.entities.take(self.g.player_id).expect("player");
        let hit = dispatch::hurt_by(
            &mut self.g,
            &def,
            lvl,
            tx,
            ty,
            &mut player,
            dmg,
            Direction::Down,
        );
        self.g.entities.put_back(player);
        hit
    }

    /// Use a fresh registry item on the tile at player + `(dx, dy)` (the tool-interact
    /// path: pays stamina/durability). Returns whether the item was used.
    pub fn interact_with(&mut self, item: &str, dx: i32, dy: i32) -> bool {
        let mut item = registry::get(&self.g, item);
        self.interact_item(&mut item, dx, dy)
    }

    /// Like [`interact_with`](Self::interact_with), but with a caller-owned item so the
    /// test can inspect it afterwards (durability paid, count consumed, ...).
    pub fn interact_item(&mut self, item: &mut crate::item::Item, dx: i32, dy: i32) -> bool {
        let (px, py) = self.player_tile();
        let (tx, ty) = (px + dx, py + dy);
        let lvl = self.g.current_level;
        let def = self.g.tile_at(lvl, tx, ty);
        let mut player = self.g.entities.take(self.g.player_id).expect("player");
        let used = dispatch::interact(
            &mut self.g,
            &def,
            lvl,
            tx,
            ty,
            &mut player,
            item,
            Direction::Down,
        );
        self.g.entities.put_back(player);
        used
    }

    /// Names of every item currently dropped on the current level (queued or live).
    pub fn dropped_items(&self) -> Vec<String> {
        let lvl = self.g.current_level;
        let mut names: Vec<String> = self
            .g
            .level(lvl)
            .entities_to_add
            .iter()
            .filter_map(|e| match &e.kind {
                EntityKind::ItemEntity(d) => Some(d.item.get_name().to_string()),
                _ => None,
            })
            .collect();
        for eid in self.g.entities.ids_on_level(lvl) {
            if let Some(EntityKind::ItemEntity(d)) = self.g.entities.get(eid).map(|e| &e.kind) {
                names.push(d.item.get_name().to_string());
            }
        }
        names
    }

    /* ------------------------------- rendering ------------------------------- */

    /// Render one frame headlessly and return the framebuffer pixels (XRGB `i32`s,
    /// `screen::W` x `screen::H`).
    pub fn render(&mut self) -> Vec<i32> {
        self.g.has_gui = true; // let the renderer draw in headless mode
        let r = self.renderer.get_or_insert_with(renderer);
        r.render(&mut self.g);
        r.screen.pixels.clone()
    }

    /// Render a frame and write it to `target/verify/<name>` (1x). Returns the path.
    pub fn screenshot(&mut self, name: &str) -> PathBuf {
        let pixels = self.render();
        let path = verify_path(name);
        save_png(&path, &pixels, screen::W as usize, screen::H as usize, 1);
        path
    }
}

impl Deref for TestWorld {
    type Target = Game;
    fn deref(&self) -> &Game {
        &self.g
    }
}

impl DerefMut for TestWorld {
    fn deref_mut(&mut self) -> &mut Game {
        &mut self.g
    }
}

impl TestWorldBuilder {
    /// World seed (default `20260707`).
    pub fn seed(mut self, seed: i64) -> Self {
        self.seed = seed;
        self
    }

    /// World + save-dir name. Defaults to a unique throwaway name; set one when the
    /// test inspects save files on disk.
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_string());
        self
    }

    /// Creative mode (fills the creative inventory).
    pub fn creative(mut self) -> Self {
        self.creative = true;
        self
    }

    /// Enable the debug-gated key bindings (`--debug`).
    pub fn debug(mut self) -> Self {
        self.debug = true;
        self
    }

    /// Boot the game: fresh temp save dir, world generated, first tick done (so the
    /// player is live in the entity arena, not the level's add-queue).
    pub fn build(self) -> TestWorld {
        let name = self.name.unwrap_or_else(|| {
            format!(
                "tw{}-{}",
                std::process::id(),
                NEXT_WORLD.fetch_add(1, Ordering::Relaxed)
            )
        });
        let mut g = bare_game(&format!("world_{name}"));
        g.debug = self.debug;
        world::reset_game(&mut g, true);
        g.settings.set("autosave", false);
        if self.creative {
            g.settings.set("mode", "Creative");
        }
        g.world_name = name;
        g.world_seed = self.seed;
        world::init_world(&mut g);
        // Live worlds spawn at a seed-random time of day; pin tests to morning-0 so
        // jumping the clock forward never reads as a midnight wrap to the scheduler.
        g.change_time_of_day(crate::core::updater::Time::Morning);
        g.tick();
        TestWorld { g, renderer: None }
    }
}

/* --------------------------------- free helpers --------------------------------- */

/// A headless [`Renderer`] with the real sprite sheet, for tests that drive rendering
/// directly (most tests want [`TestWorld::render`] / [`TestWorld::screenshot`] instead).
pub fn renderer() -> Renderer {
    Renderer::new(Arc::new(crate::assets::sprite_sheet()))
}

/// A headless `Game` with the main player created (eid 0) but **no world** — for
/// registry/recipe checks and save/load tests that fabricate their own levels.
/// The save dir is `$TMPDIR/fdoom_test_<name>`, wiped first.
pub fn bare_game(name: &str) -> Game {
    let dir = std::env::temp_dir().join(format!("fdoom_test_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut g = Game::new(false, false, dir);
    let mut player = crate::entity::mob::player::new(&g, None);
    player.c.eid = 0; // Java main() gives the main player eid 0
    g.entities.put_back(player);
    g
}

/// Nearest tile whose biome is `want` (outward ring search from the origin).
/// Panics if none is found within ~4800 tiles.
pub fn find_biome(seed: i64, want: Biome) -> (i32, i32) {
    for r in 0i32..600 {
        let ring = r * 8;
        for dy in (-ring..=ring).step_by(8) {
            for dx in (-ring..=ring).step_by(8) {
                if (dx.abs() == ring || dy.abs() == ring) && biome_at(seed, dx, dy) == want {
                    return (dx, dy);
                }
            }
        }
    }
    panic!("no {want:?} biome within range for seed {seed:#x}");
}

/// The recipe for `product` (case-insensitive) in a station list, or panic.
pub fn find_recipe<'a>(recipes: &'a [Recipe], product: &str) -> &'a Recipe {
    recipes
        .iter()
        .find(|r| r.product_name().eq_ignore_ascii_case(product))
        .unwrap_or_else(|| panic!("recipe for {product:?} not found"))
}

/// `target/verify/<name>`, with the directory created. All visual test output goes
/// here (`just shots` upscales everything in it).
pub fn verify_path(name: &str) -> PathBuf {
    let dir = Path::new("target/verify");
    std::fs::create_dir_all(dir).unwrap();
    dir.join(name)
}

/// Write XRGB `i32` pixels as an RGB PNG, nearest-neighbor upscaled by `scale`.
pub fn save_png(path: impl AsRef<Path>, pixels: &[i32], w: usize, h: usize, scale: usize) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let file = std::fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(
        std::io::BufWriter::new(file),
        (w * scale) as u32,
        (h * scale) as u32,
    );
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().unwrap();
    let mut data = vec![0u8; w * scale * h * scale * 3];
    for y in 0..h {
        for x in 0..w {
            let p = pixels[x + y * w];
            let rgb = [
                ((p >> 16) & 0xff) as u8,
                ((p >> 8) & 0xff) as u8,
                (p & 0xff) as u8,
            ];
            for sy in 0..scale {
                for sx in 0..scale {
                    let o = ((y * scale + sy) * w * scale + x * scale + sx) * 3;
                    data[o..o + 3].copy_from_slice(&rgb);
                }
            }
        }
    }
    writer.write_image_data(&data).unwrap();
}
