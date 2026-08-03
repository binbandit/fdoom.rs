//! Adversarial save/load fuzzing.
//!
//! Every case runs inside `catch_unwind` with a thread-local quiet panic hook, so one
//! run reports *all* the ways a mangled/old/partial save can take the game down rather
//! than stopping at the first. Hard convention 10: a broken save warns and degrades,
//! it never panics.

use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Once;

use fdoom::core::game::Game;
use fdoom::saveload::save::EXTENSION;
use fdoom::saveload::{load, save};
use fdoom::testutil::TestWorld;

/* --------------------------------- panic capture --------------------------------- */

thread_local! {
    static QUIET: Cell<bool> = const { Cell::new(false) };
    static LOC: RefCell<Option<String>> = const { RefCell::new(None) };
}
static HOOK: Once = Once::new();

/// Silence + record panics on threads that opted in; every other thread keeps the
/// normal panic output (so an assertion failure elsewhere stays readable).
fn install_hook() {
    HOOK.call_once(|| {
        let default = std::panic::take_hook();
        // FDOOM_FUZZ_LOUD=1 keeps the real hook (and RUST_BACKTRACE) for pinpointing
        // which frame under a failing case actually blew up.
        let loud = std::env::var_os("FDOOM_FUZZ_LOUD").is_some();
        std::panic::set_hook(Box::new(move |info| {
            if QUIET.with(|q| q.get()) && !loud {
                let loc = info
                    .location()
                    .map(|l| format!("{}:{}", l.file(), l.line()))
                    .unwrap_or_default();
                LOC.with(|c| *c.borrow_mut() = Some(loc));
            } else {
                default(info);
            }
        }));
    });
}

/// Run `f`, returning the panic message (+ source location) if it blew up.
fn run_case(f: impl FnOnce()) -> Result<(), String> {
    install_hook();
    QUIET.with(|q| q.set(true));
    LOC.with(|c| *c.borrow_mut() = None);
    let r = catch_unwind(AssertUnwindSafe(f));
    QUIET.with(|q| q.set(false));
    match r {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = e
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| e.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "<non-string panic payload>".to_string());
            let loc = LOC.with(|c| c.borrow().clone()).unwrap_or_default();
            Err(format!("{msg}   [{loc}]"))
        }
    }
}

/// A named way to break a file on disk.
type Corruption = (&'static str, fn(&Path));
/// A named game state to save from.
type Moment = (&'static str, fn(&mut TestWorld));

/// Collects failures so a whole sweep reports at once.
#[derive(Default)]
struct Report {
    failures: Vec<String>,
    ran: usize,
}

impl Report {
    fn case(&mut self, name: &str, f: impl FnOnce()) {
        self.ran += 1;
        if let Err(e) = run_case(f) {
            self.failures.push(format!("{name}: {e}"));
        }
    }

    fn finish(self, what: &str) {
        if !self.failures.is_empty() {
            let list = self
                .failures
                .iter()
                .enumerate()
                .map(|(i, f)| format!("  {}. {f}", i + 1))
                .collect::<Vec<_>>()
                .join("\n");
            panic!(
                "{} of {} {what} cases panicked:\n{list}",
                self.failures.len(),
                self.ran
            );
        }
    }
}

/* --------------------------------- save fixtures --------------------------------- */

const WORLD: &str = "fuzzworld";

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fdoom_fuzz_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let to = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), to).unwrap();
        }
    }
}

/// A realistic, rich save: infinite world, worn gear, held item, stocked chest,
/// bench with modules, notes + variants + thirst, entities on several layers.
fn build_template(tag: &str) -> PathBuf {
    // one TestWorld dir per template, or concurrently running sweeps fight over it
    let mut tw = TestWorld::infinite().seed(4242).name(tag).build();

    {
        let g = &mut tw.g;
        let hat = fdoom::item::registry::get(g, "Straw Hat");
        let armor = fdoom::item::registry::get(g, "Iron Armor");
        let pick = fdoom::item::registry::get(g, "Wood Pickaxe");
        let wood = fdoom::item::registry::get(g, "Wood_25");
        let pd = g.player_mut().player_mut();
        pd.worn_head = Some(hat);
        pd.cur_armor = Some(armor);
        pd.armor = 40;
        pd.armor_damage_buffer = 5;
        pd.worn_meter = Some(("Iron Armor".to_string(), 3, 2));
        pd.active_item = Some(pick);
        pd.inventory.add(wood);
        pd.thirst = 7;
        pd.variants_learned = 3;
    }

    {
        let g = &mut tw.g;
        let mut bench = fdoom::entity::furniture::crafter::new(
            fdoom::entity::furniture::crafter::CrafterType::Bench,
        );
        if let fdoom::entity::EntityKind::Crafter(c) = &mut bench.kind {
            for m in fdoom::entity::furniture::crafter::Module::VALUES {
                c.modules.push(m);
            }
        }
        let (px, py) = (g.player().c.x, g.player().c.y);
        let lvl = g.current_level;
        g.level_mut(lvl).add_at(bench, px + 32, py, false, lvl);

        let mut chest = fdoom::entity::furniture::chest::new();
        let apple = fdoom::item::registry::get(g, "apple_5");
        chest.chest_mut().unwrap().inventory.add(apple);
        g.level_mut(lvl).add_at(chest, px, py + 32, false, lvl);
    }

    tw.g.world_name = WORLD.to_string();
    save::save_world_named(&mut tw.g, WORLD);

    let template = scratch(tag);
    copy_dir(&tw.g.game_dir, &template);
    template
}

/// A game dir that is a fresh copy of `template`, with `mutate` applied to the
/// world folder.
fn staged(template: &Path, tag: &str, mutate: impl FnOnce(&Path)) -> PathBuf {
    let dir = scratch(tag);
    copy_dir(template, &dir);
    mutate(&dir.join("saves").join(WORLD));
    dir
}

fn fresh_game(dir: PathBuf) -> Game {
    let mut g = Game::new(false, false, dir);
    let mut player = fdoom::entity::mob::player::new(&g, None);
    player.c.eid = 0;
    g.entities.put_back(player);
    g.world_name = WORLD.to_string();
    g
}

/// Load `dir`'s world into a fresh game (the thing under test).
fn load_world_at(dir: PathBuf) {
    let mut g = fresh_game(dir);
    load::load_world_named(&mut g, WORLD);
}

fn file(world_dir: &Path, name: &str) -> PathBuf {
    world_dir.join(format!("{name}{EXTENSION}"))
}

/* ------------------------------- 1. broken save files ------------------------------ */

/// Every save file, every way it can be broken on disk (crash mid-write, disk full,
/// hand edit, filesystem weirdness).
#[test]
fn broken_save_files_never_panic() {
    let template = build_template("tmpl_files");
    let mut r = Report::default();

    let names = [
        "Game",
        "Player",
        "Inventory",
        "Entities",
        "Level4",
        "Level4data",
        "WorldMeta",
    ];
    let corruptions: [Corruption; 7] = [
        ("delete", |p| {
            let _ = std::fs::remove_file(p);
        }),
        ("empty", |p| {
            let _ = std::fs::write(p, b"");
        }),
        ("truncate-half", |p| {
            if let Ok(s) = std::fs::read(p) {
                let half = s.len() / 2;
                let _ = std::fs::write(p, &s[..half]);
            }
        }),
        ("truncate-1byte", |p| {
            if let Ok(s) = std::fs::read(p) {
                let n = s.len().min(1);
                let _ = std::fs::write(p, &s[..n]);
            }
        }),
        ("garbage-text", |p| {
            let _ = std::fs::write(p, b"this is not a save file at all, sorry");
        }),
        ("non-utf8", |p| {
            let _ = std::fs::write(p, [0xffu8, 0xfe, 0x00, 0x80, 0x9f]);
        }),
        ("dir-in-place-of-file", |p| {
            let _ = std::fs::remove_file(p);
            let _ = std::fs::create_dir_all(p);
        }),
    ];

    for name in names {
        for (label, mutate) in corruptions {
            let tag = format!("f_{name}_{label}").replace('-', "_");
            r.case(&format!("{name}/{label}"), || {
                let dir = staged(&template, &tag, |wd| mutate(&file(wd, name)));
                load_world_at(dir);
            });
        }
    }

    r.case("world-dir-is-a-file", || {
        let dir = scratch("f_worlddir_file");
        copy_dir(&template, &dir);
        let wd = dir.join("saves").join(WORLD);
        std::fs::remove_dir_all(&wd).unwrap();
        std::fs::write(&wd, b"nope").unwrap();
        load_world_at(dir);
    });
    r.case("world-dir-missing", || {
        let dir = scratch("f_worlddir_gone");
        copy_dir(&template, &dir);
        std::fs::remove_dir_all(dir.join("saves").join(WORLD)).unwrap();
        load_world_at(dir);
    });

    r.finish("broken-save-file");
}

/* ---------------------------------- 2. prefs file ---------------------------------- */

#[test]
fn broken_prefs_never_panic() {
    let mut r = Report::default();
    let payloads: [(&str, &[u8]); 10] = [
        ("empty", b""),
        ("one-field", b"3.0.0,"),
        ("two-fields", b"3.0.0,true,"),
        ("three-fields", b"3.0.0,true,true,"),
        ("garbage", b"hello world"),
        ("no-version", b"true,true,60,,,,english,UP;W:,"),
        ("bad-fps", b"3.0.0,true,true,notanumber,,,,english,UP;W:,"),
        ("keymap-no-semicolon", b"3.0.0,true,true,60,,,,english,UPW,"),
        ("keymap-empty", b"3.0.0,true,true,60,,,,english,,"),
        ("commas-only", b",,,,,,,,,,"),
    ];
    for (label, bytes) in payloads {
        r.case(label, || {
            let dir = scratch(&format!("prefs_{label}").replace('-', "_"));
            std::fs::write(dir.join(format!("Preferences{EXTENSION}")), bytes).unwrap();
            let mut g = fresh_game(dir);
            load::load_prefs(&mut g);
        });
    }
    r.finish("broken-prefs");
}

/* --------------------------------- 3. Player file --------------------------------- */

/// The tolerant trailing markers (Held/WornHead/ArmorMeter/Notes/Thirst/Variants):
/// out of order, duplicated, empty, separator-laden, unknown payloads — plus
/// corruption of the fixed-arity prefix.
#[test]
fn player_file_fuzz_never_panics() {
    let template = build_template("tmpl_player");
    let base =
        std::fs::read_to_string(file(&template.join("saves").join(WORLD), "Player")).unwrap();
    let fields: Vec<&str> = base.trim_end_matches(',').split(',').collect();
    let classic = fields
        .iter()
        .copied()
        .take_while(|f| {
            !f.starts_with("WornHead:")
                && !f.starts_with("ArmorMeter:")
                && !f.starts_with("Notes:")
                && !f.starts_with("Thirst:")
                && !f.starts_with("Variants:")
        })
        .collect::<Vec<_>>()
        .join(",");

    let tails: [(&str, &str); 18] = [
        ("no-markers", ""),
        (
            "reordered",
            ",Thirst:5,WornHead:Straw Hat,Notes:v1:,Variants:v1:1",
        ),
        ("duplicated", ",Thirst:5,Thirst:9,Thirst:2"),
        ("empty-worn-head", ",WornHead:"),
        ("empty-armor-meter", ",ArmorMeter:"),
        ("empty-notes", ",Notes:v1:"),
        ("empty-thirst", ",Thirst:"),
        ("empty-variants", ",Variants:v1:"),
        ("unknown-head-item", ",WornHead:Sombrero Of Doom"),
        ("non-head-item", ",WornHead:Wood Pickaxe"),
        ("armor-meter-partial", ",ArmorMeter:Iron Armor"),
        ("armor-meter-junk", ",ArmorMeter:;;;;;"),
        ("armor-meter-colons", ",ArmorMeter:Iron:Armor;a;b"),
        ("thirst-negative", ",Thirst:-99999"),
        ("thirst-huge", ",Thirst:99999999999999999999"),
        ("variants-negative", ",Variants:v1:-1"),
        (
            "variants-huge",
            ",Variants:v1:340282366920938463463374607431768211456",
        ),
        ("notes-garbage", ",Notes:v1:!!!!;;;::::"),
    ];
    let mut r = Report::default();
    for (label, tail) in tails {
        r.case(label, || {
            let tag = format!("pl_{label}").replace('-', "_");
            let body = format!("{classic}{tail},");
            let dir = staged(&template, &tag, |wd| {
                std::fs::write(file(wd, "Player"), &body).unwrap();
            });
            load_world_at(dir);
        });
    }

    let fixed: [(&str, &str); 12] = [
        ("no-fields", ""),
        ("x-only", "100"),
        ("x-y-only", "100,100"),
        ("through-health", "100,100,5,5,10"),
        (
            "non-numeric-x",
            "abc,100,5,5,10,9,0,0,3,PotionEffects[],0,false",
        ),
        ("empty-x", ",100,5,5,10,9,0,0,3,PotionEffects[],0,false"),
        (
            "huge-x",
            "999999999999,100,5,5,10,9,0,0,3,PotionEffects[],0,false",
        ),
        (
            "negative-health",
            "100,100,5,5,-99,9,0,0,3,PotionEffects[],0,false",
        ),
        (
            "level-out-of-range",
            "100,100,5,5,10,9,0,0,99,PotionEffects[],0,false",
        ),
        (
            "level-negative",
            "100,100,5,5,10,9,0,0,-3,PotionEffects[],0,false",
        ),
        (
            "unknown-potion",
            "100,100,5,5,10,9,0,0,3,PotionEffects[Nonsense;100],0,false",
        ),
        (
            "potion-no-duration",
            "100,100,5,5,10,9,0,0,3,PotionEffects[Regen],0,false",
        ),
    ];
    for (label, body) in fixed {
        r.case(&format!("fixed/{label}"), || {
            let tag = format!("plf_{label}").replace('-', "_");
            let body = format!("{body},");
            let dir = staged(&template, &tag, |wd| {
                std::fs::write(file(wd, "Player"), &body).unwrap();
            });
            load_world_at(dir);
        });
    }
    r.finish("player-file");
}

/* -------------------------------- 4. Inventory file -------------------------------- */

#[test]
fn inventory_file_fuzz_never_panics() {
    let template = build_template("tmpl_inv");
    let bodies: [(&str, &str); 12] = [
        ("empty", ""),
        ("held-unknown", "Held:Excalibur Of Nonsense,Wood_5,"),
        ("held-empty", "Held:,Wood_5,"),
        ("held-only", "Held:Wood Pickaxe,"),
        ("unknown-item", "Bogus Item,Wood_5,"),
        ("empty-entries", ",,,,,"),
        ("count-not-a-number", "Wood_notanumber,"),
        ("count-negative", "Wood_-5,"),
        ("count-huge", "Wood_99999999999999999,"),
        ("semicolon-legacy", "Wood;5,"),
        ("semicolon-no-count", "Wood;,"),
        ("colons-in-name", "Wood:Pickaxe:5,"),
    ];
    let mut r = Report::default();
    for (label, body) in bodies {
        r.case(label, || {
            let tag = format!("inv_{label}").replace('-', "_");
            let dir = staged(&template, &tag, |wd| {
                std::fs::write(file(wd, "Inventory"), body).unwrap();
            });
            load_world_at(dir);
        });
    }
    r.finish("inventory-file");
}

/* -------------------------------- 5. Entities file -------------------------------- */

#[test]
fn entities_file_fuzz_never_panics() {
    let template = build_template("tmpl_ent");
    let bodies: [(&str, &str); 21] = [
        ("empty", ""),
        ("unknown-entity", "Creeper[100:100:5:1:3],"),
        ("no-brackets", "Zombie 100 100 5 1 3,"),
        ("open-bracket-only", "Zombie[100:100:5:1:3,"),
        ("close-bracket-only", "Zombie]100:100,"),
        ("reversed-brackets", "Zombie]100:100[,"),
        ("empty-brackets", "Zombie[],"),
        ("one-field", "Zombie[100],"),
        ("two-fields", "Zombie[100:100],"),
        ("non-numeric-coords", "Zombie[x:y:5:1:3],"),
        ("level-out-of-range", "Zombie[100:100:5:1:77],"),
        ("level-negative", "Zombie[100:100:5:1:-1],"),
        ("chest-no-level", "Chest[100:100],"),
        ("chest-unknown-item", "Chest[100:100:Bogus Item:3],"),
        ("chest-bad-count", "Chest[100:100:Wood;x:3],"),
        ("deathchest-no-time", "DeathChest[100:100:3],"),
        ("dungeonchest-short", "DungeonChest[100:100:4],"),
        ("scav-short", "ScavContainer[100:100:4],"),
        ("spawner-short", "Spawner[100:100:3],"),
        ("bench-bad-modules", "Bench[100:100:a;b;999999:3],"),
        ("lantern-bad-ordinal", "Lantern[100:100:99:0:3],"),
    ];
    let mut r = Report::default();
    for (label, body) in bodies {
        r.case(label, || {
            let tag = format!("ent_{label}").replace('-', "_");
            let dir = staged(&template, &tag, |wd| {
                std::fs::write(file(wd, "Entities"), body).unwrap();
            });
            load_world_at(dir);
        });
    }
    r.finish("entities-file");
}

/* ---------------------------------- 6. Game file ---------------------------------- */

#[test]
fn game_file_fuzz_never_panics() {
    let template = build_template("tmpl_game");
    let bodies: [(&str, &str); 12] = [
        ("version-only", "3.0.0,"),
        ("no-version", "0,3600,70000,1,true,"),
        ("bad-version", "not.a.version,0,3600,70000,1,true,"),
        ("old-version", "1.9.2,0,3600,70000,1,true,"),
        ("future-version", "99.9.9,0,3600,70000,1,true,"),
        ("missing-tail", "3.0.0,0,3600,"),
        ("non-numeric-time", "3.0.0,0,abc,70000,1,true,"),
        ("non-numeric-mode", "3.0.0,x,3600,70000,1,true,"),
        ("huge-diff", "3.0.0,0,3600,70000,99999,true,"),
        ("negative-diff", "3.0.0,0,3600,70000,-5,true,"),
        ("huge-time", "3.0.0,0,99999999999999999,70000,1,true,"),
        ("mode-out-of-range", "3.0.0,42,3600,70000,1,true,"),
    ];
    let mut r = Report::default();
    for (label, body) in bodies {
        r.case(label, || {
            let tag = format!("gm_{label}").replace('-', "_");
            let dir = staged(&template, &tag, |wd| {
                std::fs::write(file(wd, "Game"), body).unwrap();
            });
            load_world_at(dir);
        });
    }
    r.finish("game-file");
}

/* --------------------------------- 7. Level file ---------------------------------- */

#[test]
fn level_file_fuzz_never_panics() {
    let template = build_template("tmpl_level");
    let bodies: [(&str, &str); 9] = [
        ("dims-only", "128,128,-4,"),
        ("short-tile-list", "128,128,-4,grass,grass,grass,"),
        ("zero-dims", "0,0,-4,"),
        ("negative-dims", "-5,-5,-4,"),
        ("huge-dims", "100000,100000,-4,"),
        ("overflowing-dims", "2147483647,2147483647,-4,"),
        ("non-numeric-dims", "a,b,-4,"),
        ("unknown-tile-name", "1,1,-4,Nonsense Tile,"),
        ("empty", ""),
    ];
    let mut r = Report::default();
    for (label, body) in bodies {
        r.case(label, || {
            let tag = format!("lv_{label}").replace('-', "_");
            let dir = staged(&template, &tag, |wd| {
                std::fs::write(file(wd, "Level4"), body).unwrap();
            });
            load_world_at(dir);
        });
    }

    // matching tile file but a broken/short companion data file
    let datas: [(&str, &str); 4] = [
        ("data-empty", ""),
        ("data-short", "0,0,0,"),
        ("data-non-numeric", "x,"),
        ("data-out-of-i8-range", "999,"),
    ];
    for (label, body) in datas {
        r.case(label, || {
            let tag = format!("lvd_{label}").replace('-', "_");
            let dir = staged(&template, &tag, |wd| {
                std::fs::write(file(wd, "Level4"), "1,1,-4,grass,").unwrap();
                std::fs::write(file(wd, "Level4data"), body).unwrap();
            });
            load_world_at(dir);
        });
    }
    r.finish("level-file");
}

/* ------------------------------- 7b. chunk files ---------------------------------- */

/// The streamed chunk files: truncated by a crash mid-flush, zero-byte, or bloated.
#[test]
fn broken_chunk_files_never_panic() {
    let template = build_template("tmpl_chunk");
    let mut r = Report::default();

    let mutations: [Corruption; 6] = [
        ("zero-byte", |p| {
            let _ = std::fs::write(p, b"");
        }),
        ("one-byte", |p| {
            let _ = std::fs::write(p, b"x");
        }),
        ("half", |p| {
            if let Ok(b) = std::fs::read(p) {
                let half = b.len() / 2;
                let _ = std::fs::write(p, &b[..half]);
            }
        }),
        ("tiles-only", |p| {
            if let Ok(b) = std::fs::read(p) {
                let n = b.len().min(4096);
                let _ = std::fs::write(p, &b[..n]);
            }
        }),
        ("one-byte-short-of-complete", |p| {
            if let Ok(b) = std::fs::read(p) {
                let n = b.len().saturating_sub(1);
                let _ = std::fs::write(p, &b[..n]);
            }
        }),
        ("giant", |p| {
            let _ = std::fs::write(p, vec![0xabu8; 1 << 16]);
        }),
    ];

    for (label, mutate) in mutations {
        r.case(label, || {
            let tag = format!("ck_{label}").replace('-', "_");
            let dir = scratch(&tag);
            copy_dir(&template, &dir);
            let chunks = dir.join("saves").join(WORLD).join("chunks");
            assert!(chunks.is_dir(), "template has no chunk data to corrupt");
            let mut touched = 0;
            for depth in std::fs::read_dir(&chunks).unwrap().flatten() {
                for f in std::fs::read_dir(depth.path()).unwrap().flatten() {
                    mutate(&f.path());
                    touched += 1;
                }
            }
            assert!(touched > 0, "no chunk files were corrupted");

            let mut g = fresh_game(dir);
            load::load_world_named(&mut g, WORLD);
            for lvl in 0..g.levels.len() {
                fdoom::level::tick_level(&mut g, lvl, false);
            }
            // stream the (broken) chunks back in around the player, then play on
            let lvl = g.current_level;
            fdoom::level::ensure_chunks(&mut g, lvl);
            for _ in 0..10 {
                g.tick();
            }
        });
    }
    r.finish("chunk-file");
}

/* ------------------ 8. a world that refuses to load must not crash ----------------- */

/// A save the loader cannot read used to leave `g.levels` empty and then drop the
/// player into gameplay, where the panicking `Game::level()` accessor took the game
/// down on the next tick.
#[test]
fn refused_load_leaves_a_running_game() {
    let template = build_template("tmpl_refused");
    let mut r = Report::default();

    let breakages: [Corruption; 3] = [
        ("game-file-empty", |wd| {
            std::fs::write(file(wd, "Game"), b"").unwrap();
        }),
        ("game-file-gone", |wd| {
            std::fs::remove_file(file(wd, "Game")).unwrap();
        }),
        ("pre-3.0-world", |wd| {
            std::fs::write(file(wd, "Game"), "2.0.7,0,3600,70000,1,false,").unwrap();
        }),
    ];

    for (label, breakage) in breakages {
        r.case(label, || {
            let tag = format!("rf_{label}").replace('-', "_");
            let dir = staged(&template, &tag, breakage);
            let mut g = fresh_game(dir);
            fdoom::screen::world_select::set_world_name(&mut g, WORLD, true);
            fdoom::core::world::init_world(&mut g);
            assert!(
                g.world_load_failed,
                "{label}: load should have been refused"
            );
            // every layer must still be a real level, and the game must keep ticking
            for i in 0..g.levels.len() {
                assert!(g.levels[i].is_some(), "{label}: layer {i} left empty");
            }
            for _ in 0..30 {
                g.tick();
            }
        });
    }
    r.finish("refused-load");
}

/* ---------------------------- 9. extreme tile coordinates -------------------------- */

/// i32 overflow hunting: a player parked at the far edge of the coordinate space
/// (reachable from a hand-edited save, and the natural end of a very long walk).
#[test]
fn extreme_coordinates_never_panic() {
    let mut r = Report::default();

    let spots: [(&str, i32, i32); 10] = [
        ("i32-max", i32::MAX, i32::MAX),
        ("i32-min", i32::MIN, i32::MIN),
        ("i32-max-minus-8", i32::MAX - 8, i32::MAX - 8),
        ("i32-min-plus-8", i32::MIN + 8, i32::MIN + 8),
        ("mixed-extremes", i32::MAX, i32::MIN),
        ("2e9", 2_000_000_000, 2_000_000_000),
        ("neg-2e9", -2_000_000_000, -2_000_000_000),
        ("chunk-edge", 1 << 30, -(1 << 30)),
        ("far-x-only", i32::MAX - 1, 0),
        ("far-y-only", 0, i32::MIN + 1),
    ];

    for (label, px, py) in spots {
        // chunk streaming around a player parked at the edge of the coordinate space
        r.case(&format!("stream-chunks/{label}"), || {
            let mut tw = TestWorld::infinite().seed(9).build();
            {
                let p = tw.g.player_mut();
                p.c.x = px;
                p.c.y = py;
            }
            let lvl = tw.g.current_level;
            fdoom::level::ensure_chunks(&mut tw.g, lvl);
            // closest-player distance math over a span no i32 difference can hold
            let _ = fdoom::level::get_closest_player(&tw.g, lvl, 0, 0);
            let _ = fdoom::level::get_closest_player(&tw.g, lvl, i32::MIN, i32::MAX);
        });

        r.case(&format!("level-change/{label}"), || {
            let mut tw = TestWorld::infinite().seed(9).build();
            {
                let p = tw.g.player_mut();
                p.c.x = px;
                p.c.y = py;
            }
            fdoom::core::world::change_level(&mut tw.g, -1);
            fdoom::core::world::change_level(&mut tw.g, 1);
        });
    }

    // chunk streaming asked for chunks at the very edge of the coordinate space
    let tiles: [(&str, i32, i32); 6] = [
        ("tile-i32-max", i32::MAX, i32::MAX),
        ("tile-i32-min", i32::MIN, i32::MIN),
        ("tile-max-chunk", i32::MAX >> 4, i32::MAX >> 4),
        ("tile-min-chunk", i32::MIN >> 4, i32::MIN >> 4),
        ("tile-mixed", i32::MAX, i32::MIN),
        ("tile-2e9", 2_000_000_000, -2_000_000_000),
    ];
    for (label, tx, ty) in tiles {
        r.case(&format!("ensure-chunks-at/{label}"), || {
            let mut tw = TestWorld::infinite().seed(9).build();
            let lvl = tw.g.current_level;
            fdoom::level::ensure_chunks_at(&mut tw.g, lvl, tx, ty, false);
        });
    }

    // a hand-edited Player file that parks the player past the edge of the world
    let template = build_template("tmpl_extreme");
    for (label, px, py) in spots {
        r.case(&format!("save-load/{label}"), || {
            let tag = format!("ex_{label}").replace('-', "_");
            let body = format!("{px},{py},5,5,10,9,0,0,3,PotionEffects[],0,false,");
            let dir = staged(&template, &tag, |wd| {
                std::fs::write(file(wd, "Player"), &body).unwrap();
            });
            let mut g = fresh_game(dir);
            assert!(load::load_world_named(&mut g, WORLD));
            for i in 0..g.levels.len() {
                fdoom::level::tick_level(&mut g, i, false);
            }
            let lvl = g.current_level;
            fdoom::level::ensure_chunks(&mut g, lvl);
            fdoom::core::world::change_level(&mut g, -1);
        });
    }
    r.finish("extreme-coordinate");
}

/// Cross-lane findings, recorded rather than fixed: a *full game tick* with the player
/// parked at the edge of the coordinate space still overflows, but every remaining
/// site is owned by another lane (entity + gfx), so this lane leaves them alone.
///
/// Un-ignore once those lanes widen their coordinate math:
///   - `src/entity/behavior.rs:518-519` — the fire-corner check does `e.c.x ± e.c.xr`
///     in i32 (`attempt to add/subtract with overflow`).
///   - `src/entity/behavior.rs:194` — `is_within` does `e.c.x - other.c.x` in i32
///     before widening to f64.
///   - `src/gfx/rectangle.rs:61` — rectangle construction adds without widening.
#[test]
#[ignore = "known overflows owned by the entity/gfx lanes; see doc comment"]
fn extreme_coordinates_full_tick_entity_and_gfx_lanes() {
    let mut r = Report::default();
    for (label, px, py) in [
        ("i32-max", i32::MAX, i32::MAX),
        ("i32-min", i32::MIN, i32::MIN),
        ("mixed-extremes", i32::MAX, i32::MIN),
    ] {
        r.case(&format!("full-tick/{label}"), || {
            let mut tw = TestWorld::infinite().seed(9).build();
            {
                let p = tw.g.player_mut();
                p.c.x = px;
                p.c.y = py;
            }
            let lvl = tw.g.current_level;
            fdoom::level::ensure_chunks(&mut tw.g, lvl);
            fdoom::level::tick_level(&mut tw.g, lvl, true);
            tw.tick_recover();
        });
    }
    r.finish("extreme-coordinate-full-tick");
}

/* ------------------------------ 10. level transitions ------------------------------ */

#[test]
fn level_transitions_never_panic() {
    let mut r = Report::default();

    r.case("dig-down-and-back-repeatedly", || {
        let mut tw = TestWorld::infinite().seed(31).name("transit").build();
        for _ in 0..3 {
            for _ in 0..4 {
                fdoom::core::world::change_level(&mut tw.g, -1);
                tw.tick_recover();
            }
            for _ in 0..4 {
                fdoom::core::world::change_level(&mut tw.g, 1);
                tw.tick_recover();
            }
        }
    });

    r.case("save-load-at-every-depth", || {
        for step in 0..5 {
            let name = format!("depth{step}");
            let mut tw = TestWorld::infinite().seed(31).name(&name).build();
            for _ in 0..step {
                fdoom::core::world::change_level(&mut tw.g, -1);
                tw.tick_recover();
            }
            save::save_world_named(&mut tw.g, &name);

            let mut g = Game::new(false, false, tw.g.game_dir.clone());
            let mut player = fdoom::entity::mob::player::new(&g, None);
            player.c.eid = 0;
            g.entities.put_back(player);
            g.world_name = name.clone();
            assert!(load::load_world_named(&mut g, &name), "depth {step}");
            for i in 0..g.levels.len() {
                fdoom::level::tick_level(&mut g, i, false);
            }
            for _ in 0..5 {
                g.tick();
            }
        }
    });

    r.case("transition-past-both-ends", || {
        let mut tw = TestWorld::infinite().seed(31).build();
        // walk off both ends of the level stack (the wrap-around guards)
        for _ in 0..8 {
            fdoom::core::world::change_level(&mut tw.g, -1);
        }
        for _ in 0..8 {
            fdoom::core::world::change_level(&mut tw.g, 1);
        }
        tw.tick_recover();
    });

    r.case("transition-with-a-menu-open", || {
        let mut tw = TestWorld::infinite().seed(31).build();
        tw.press("MENU");
        fdoom::core::world::change_level(&mut tw.g, -1);
        tw.tick_recover();
    });

    r.case("transition-while-dead", || {
        let mut tw = TestWorld::infinite().seed(31).build();
        tw.g.player_mut().player_mut().mob.health = 0;
        tw.g.tick();
        fdoom::core::world::change_level(&mut tw.g, -1);
        tw.g.tick();
    });

    r.case("scheduled-change-through-the-tick-loop", || {
        let mut tw = TestWorld::infinite().seed(31).build();
        for _ in 0..6 {
            fdoom::core::world::schedule_level_change(&mut tw.g, -1);
            tw.tick_recover();
            tw.tick_recover();
        }
    });

    r.finish("level-transition");
}

/* ------------------------------- 11. world creation -------------------------------- */

#[test]
fn world_names_never_panic() {
    let mut r = Report::default();
    let long_name = "w".repeat(200);
    let names: [(&str, &str); 9] = [
        ("empty", ""),
        ("spaces", "   "),
        ("dot-dot", ".."),
        ("slash", "a/b"),
        ("backslash", "a\\b"),
        ("unicode", "wörld-日本語-🌍"),
        ("200-chars", &long_name),
        ("mixed-case", "MiXeDcAsE"),
        ("trailing-dot", "world."),
    ];
    for (label, name) in names {
        r.case(&format!("create/{label}"), || {
            let dir = scratch(&format!("wn_{label}").replace('-', "_"));
            let mut g = fresh_game(dir);
            g.world_name = name.to_string();
            fdoom::screen::world_select::set_world_name(&mut g, name, false);
            fdoom::core::world::init_world(&mut g);
            save::save_world_named(&mut g, name);
            let _ = fdoom::screen::world_select::get_world_names(&g);
        });
    }

    r.case("duplicate-name-reloads-cleanly", || {
        let dir = scratch("wn_duplicate");
        for _ in 0..2 {
            let mut g = fresh_game(dir.clone());
            g.world_name = "dupe".to_string();
            fdoom::screen::world_select::set_world_name(&mut g, "dupe", false);
            fdoom::core::world::init_world(&mut g);
            save::save_world_named(&mut g, "dupe");
        }
        let mut g = fresh_game(dir);
        fdoom::screen::world_select::set_world_name(&mut g, "dupe", true);
        fdoom::core::world::init_world(&mut g);
        assert!(!g.world_load_failed);
    });

    r.case("world-list-with-junk-in-the-saves-dir", || {
        let dir = scratch("wn_junk");
        let saves = dir.join("saves");
        std::fs::create_dir_all(&saves).unwrap();
        std::fs::write(saves.join("a-file-not-a-world"), b"junk").unwrap();
        std::fs::create_dir_all(saves.join("emptyworld")).unwrap();
        let g = fresh_game(dir);
        let _ = fdoom::screen::world_select::get_world_names(&g);
    });

    r.finish("world-name");
}

/* ---------------------------- 12. saving in odd moments ---------------------------- */

#[test]
fn saving_mid_state_never_panics() {
    let mut r = Report::default();

    let moments: [Moment; 6] = [
        ("menuopen", |tw| {
            tw.press("MENU");
        }),
        ("dead", |tw| {
            tw.g.player_mut().player_mut().mob.health = 0;
            tw.g.tick();
        }),
        ("fullinventory", |tw| {
            for item in ["Wood", "Stone", "Sand", "Dirt", "Coal", "Apple"] {
                tw.give(item, 99);
            }
        }),
        ("midtransition", |tw| {
            fdoom::core::world::schedule_level_change(&mut tw.g, -1);
        }),
        ("deepunderground", |tw| {
            for _ in 0..3 {
                fdoom::core::world::change_level(&mut tw.g, -1);
                tw.tick_recover();
            }
        }),
        ("noithelditem", |tw| {
            tw.g.player_mut().player_mut().active_item = None;
        }),
    ];

    for (label, setup) in moments {
        r.case(label, || {
            let name = format!("moment{label}");
            let mut tw = TestWorld::infinite().seed(77).name(&name).build();
            setup(&mut tw);
            save::save_world_named(&mut tw.g, &name);

            let mut g = Game::new(false, false, tw.g.game_dir.clone());
            let mut player = fdoom::entity::mob::player::new(&g, None);
            player.c.eid = 0;
            g.entities.put_back(player);
            g.world_name = name.clone();
            assert!(
                load::load_world_named(&mut g, &name),
                "{label}: load refused"
            );
            for i in 0..g.levels.len() {
                fdoom::level::tick_level(&mut g, i, false);
            }
            for _ in 0..10 {
                g.tick();
            }
        });
    }
    r.finish("save-moment");
}

/* --------------- 13. degrading gracefully, not just failing to panic --------------- */

/// A damaged layer file costs the player that one layer, not the whole world.
#[test]
fn damaged_layer_is_rebuilt_not_dropped() {
    let template = build_template("tmpl_relayer");
    let dir = staged(&template, "beh_relayer", |wd| {
        std::fs::write(file(wd, "Level4"), "128,128,-4,grass,grass,").unwrap();
    });
    let mut g = fresh_game(dir);
    assert!(
        load::load_world_named(&mut g, WORLD),
        "a damaged dungeon file must not fail the whole world"
    );
    // the dungeon came back as a real, generated layer...
    let dungeon = g.level(4);
    assert!(dungeon.w > 0 && dungeon.h > 0, "dungeon has no size");
    assert!(
        dungeon.tiles.iter().any(|&t| t != 0),
        "rebuilt dungeon is blank"
    );
    // ...with the landing gate arrivals need
    let stairs_up = g.tiles.get("Stairs Up").id;
    assert!(
        (0..dungeon.w).any(|x| (0..dungeon.h).any(|y| g.tile_at(4, x, y).id == stairs_up)),
        "rebuilt dungeon has no stairs up to arrive on"
    );
    // ...and the four infinite layers are untouched
    for idx in 0..4 {
        assert!(g.level(idx).is_infinite(), "layer {idx} lost its chunks");
    }
}

/// Losing WorldMeta used to send an infinite world down the finite path, where the
/// never-written Level0..3 files took the load down.
#[test]
fn missing_worldmeta_still_loads_as_infinite() {
    let template = build_template("tmpl_meta");
    let dir = staged(&template, "beh_meta", |wd| {
        std::fs::remove_file(file(wd, "WorldMeta")).unwrap();
    });
    let mut g = fresh_game(dir);
    assert!(load::load_world_named(&mut g, WORLD));
    for idx in 0..4 {
        assert!(
            g.level(idx).is_infinite(),
            "layer {idx} should still be chunked"
        );
    }
}

/// A player saved on a level this world does not have lands on the surface instead
/// of indexing off the end of `g.levels`.
#[test]
fn player_on_a_missing_level_lands_on_the_surface() {
    let template = build_template("tmpl_lvl");
    for bad in ["99", "-3"] {
        let dir = staged(
            &template,
            &format!("beh_lvl{bad}").replace('-', "n"),
            |wd| {
                let body = format!("100,100,5,5,10,9,0,0,{bad},PotionEffects[],0,false,");
                std::fs::write(file(wd, "Player"), body).unwrap();
            },
        );
        let mut g = fresh_game(dir);
        assert!(load::load_world_named(&mut g, WORLD));
        assert!(
            g.current_level < g.levels.len(),
            "level {bad} was not clamped"
        );
        assert_eq!(g.current_level, 3, "should have landed on the surface");
    }
}

/// The day-cycle setting used to be read out of the keymap slot, so it never came
/// back from a Preferences file.
#[test]
fn daycycle_pref_round_trips() {
    let dir = scratch("beh_daycycle");
    let mut g1 = fresh_game(dir.clone());
    g1.settings.set("daycycle", "Long");
    g1.settings.set("fps", 90);
    save::save_prefs(&mut g1);

    let mut g2 = fresh_game(dir);
    load::load_prefs(&mut g2);
    assert_eq!(g2.settings.get("daycycle").as_str(), "Long");
    assert_eq!(g2.settings.get("fps").as_int(), 90);
}

/// `Level::remove` drops queued entities, so reaching for the player with
/// remove-then-take destroyed a player still sitting in a level's add-queue.
#[test]
fn back_to_back_level_changes_keep_the_player() {
    let mut tw = TestWorld::infinite().seed(5).build();
    let start = tw.g.current_level;
    // no tick in between: the player is still in the destination level's add-queue
    fdoom::core::world::change_level(&mut tw.g, -1);
    fdoom::core::world::change_level(&mut tw.g, -1);
    assert!(
        tw.g.try_player().is_some(),
        "the player was destroyed by the second transition"
    );
    fdoom::core::world::change_level(&mut tw.g, 1);
    fdoom::core::world::change_level(&mut tw.g, 1);
    assert_eq!(tw.g.current_level, start);
    tw.tick_recover();
    assert!(tw.g.try_player().is_some());
}
