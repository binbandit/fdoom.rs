//! pixel_studio — the game's pixel-art studio. The split sprite PNGs under
//! `assets/sprites/**` are the art source of truth (docs/ART_GUIDE.md); this tool
//! browses, previews, and edits them in place.
//!
//! Three views, one editor:
//!
//! - **Directory mode** (primary): point it at a folder (default `assets/sprites`)
//!   and the left pane is a file browser over every `*.png` under it (`/` finds by
//!   name, `N` creates a new sprite). Opening a file sizes the editor to the image;
//!   bigger strips are edited one window at a time. When the folder has a
//!   `manifest.txt` (the atlas manifest), each file's declared `pal`/`rgb` mode
//!   drives precise wrong-mode warnings.
//! - **Canvas mode** (`W` from directory mode, or `--canvas`): every file stitched
//!   into one editable canvas via the game's own stitcher, i.e. the real atlas
//!   layout. Paints route to their owning file (per-file dirty tracking, red
//!   outlines), `S` saves only the dirty files, `G` snaps the window to the file
//!   under the cursor, Shift+arrows nudges just that file, and copy/paste/eyedrop
//!   work across file boundaries.
//! - **Sheet mode** (fallback, for `assets/golden_atlas.png` or any stitched atlas):
//!   the left pane shows the whole sheet at 2x. The editing window sits at any 8px
//!   cell (no even-cell snapping); `G` jumps it to the sprite under the cursor with
//!   its true footprint via a built-in sprite map, and the header names that sprite.
//!
//! ```sh
//! cargo run --bin pixel_studio                                  # assets/sprites
//! cargo run --bin pixel_studio -- assets/sprites --canvas       # whole-sheet view
//! cargo run --bin pixel_studio -- --sheet assets/golden_atlas.png --cell 15 26
//! cargo run --bin pixel_studio -- <png> --set X Y RRGGBB        # headless batch edit
//! cargo run --bin pixel_studio -- <dir> --canvas --set X Y c    # canvas-coord edits
//! cargo run --bin pixel_studio -- <dir> --new items/moonfruit 8x8
//! cargo run --bin pixel_studio -- <target> --snap CX CY         # report G-snap, exit
//! cargo run --bin pixel_studio -- <target> --shot out.png       # headless UI frame
//! ```
//!
//! Press `?` in-app for the full key list. Highlights: palette-applied preview (`P`
//! cycles real game palettes so grayscale sprites show as the game draws them),
//! in-context previews over the real terrain textures (`D` cycles grass/sand/snow/
//! water/night, sampled from the loaded sheet through the tiles' actual palettes),
//! animation preview (`A`), onion skin (`B` capture / `O` toggle), line/rect tools
//! (`L`/`R`/Shift+`R`), mirror-draw (`M`), copy/paste (Ctrl+C/V), shade-shift
//! (`[`/`]`), image nudge (Shift+arrows, wraps), undo/redo (`U`/`Y`), wheel zoom at
//! the cursor, middle-drag pan.
//!
//! Pixel semantics mirror `src/gfx/sprite_sheet.rs`: opaque grays (`r==g==b`) are
//! palette pixels recolored at draw time (legal shades are exactly 0/85/170/255),
//! any saturated color draws literally, alpha < 128 is transparent. Mixing the two
//! modes in one 8x8 cell (or violating a file's manifest mode) gets a warning.
//!
//! The window shell mirrors `worldview` (winit 0.30 + softbuffer, scaled blit); the
//! UI is drawn rects + the game font. No `Game`, no new dependencies.
//!
//! # Layout
//!
//! | module     | owns |
//! |------------|------|
//! | `cli`      | the command-line grammar |
//! | `batch`    | headless `--new`/`--set`/`--blit`/`--nudge` |
//! | `layout`   | the 960x720 frame's fixed geometry and chrome colours |
//! | `color`    | pixel semantics, colour maths, preview palettes, backdrops |
//! | `atlas`    | the 8px cell grid and the classic atlas sprite map |
//! | `image`    | the pixel buffer and its PNG round-trip |
//! | `shapes`   | line and rectangle brush geometry |
//! | `library`  | the sprite tree on disk and the manifest |
//! | `canvas`   | the stitched all-files canvas and edit routing |
//! | `studio`   | editor state and everything it does (see `studio::mod`) |
//! | `app`      | the winit window shell and pointer hit-testing |
//! | `keymap`   | every keyboard binding |

mod app;
mod atlas;
mod batch;
mod canvas;
mod cli;
mod color;
mod image;
mod keymap;
mod layout;
mod library;
mod shapes;
mod studio;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use winit::event_loop::{ControlFlow, EventLoop};

use app::App;
use atlas::CELL;
use canvas::{Stitched, build_canvas};
use cli::Args;
use image::{Image, write_png};
use layout::{VIEW_H, VIEW_W};
use library::{Entry, load_manifest_modes, walk};
use studio::{NewSprite, Source, Studio};

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = Args::parse(&argv);

    let target = resolve_target(args.target.clone());
    let dir_mode = target.is_dir() && !args.force_sheet;
    if (args.canvas_mode || args.new_sprite.is_some()) && !dir_mode {
        eprintln!("--canvas / --new require a directory target");
        std::process::exit(2);
    }

    // headless new-sprite: create a blank PNG in the tree and exit
    if let Some((rel, size)) = &args.new_sprite {
        batch::run_new_sprite(&target, rel, size);
        return;
    }

    // headless canvas batch: edits in stitched-canvas coordinates route to their
    // owning files; only the touched files are rewritten (each with a .bak).
    if args.canvas_mode && !args.batch.is_empty() {
        batch::run_canvas_batch(&target, &args.batch);
        return;
    }

    let (entries, open_idx, path) = resolve_open_file(&target, dir_mode, &args);
    let mut img = batch::open_or_exit(&path);

    // headless batch mode: apply edits in argument order, back up, save, exit
    if !args.batch.is_empty() {
        batch::run_file_batch(&path, &mut img, &args.batch);
        return;
    }

    let mut st = build_studio(&args, &target, dir_mode, entries, open_idx, path, img);

    // `--snap CX CY`: report which sprite a G-snap at that cell selects, then exit
    // (with `--shot` the frame is rendered first). This is the odd-origin regression
    // hook: the selection must cover the whole sprite, wherever it starts.
    if let Some((cx, cy)) = args.snap {
        st.sheet_hover = Some((cx * CELL, cy * CELL));
        st.snap_to_sprite();
        println!("{}", st.title());
        println!("{}", st.status);
        if args.shot.is_none() {
            return;
        }
    }

    if let Some(out) = &args.shot {
        run_shot(&mut st, out);
        return;
    }

    println!("{}", st.title());
    println!("controls: press ? in-app for the full key list");
    run_gui(st);
}

/// With no target named, prefer the split-sprite tree; fall back to the atlas.
fn resolve_target(explicit: Option<PathBuf>) -> PathBuf {
    explicit.unwrap_or_else(|| {
        let dir = PathBuf::from("assets/sprites");
        if dir.is_dir() {
            dir
        } else {
            PathBuf::from("assets/golden_atlas.png")
        }
    })
}

/// The PNG to open, plus — in dir mode — the walked entry list it came from and the
/// index of the opened file inside it.
fn resolve_open_file(
    target: &Path,
    dir_mode: bool,
    args: &Args,
) -> (Option<Vec<Entry>>, usize, PathBuf) {
    if !dir_mode {
        if args.file_rel.is_some() {
            eprintln!("--file only applies to directory mode");
            std::process::exit(2);
        }
        return (None, 0, target.to_path_buf());
    }
    let entries = walk(target);
    let idx = match &args.file_rel {
        Some(rel) => entries
            .iter()
            .position(|e| !e.is_dir && e.rel == *rel)
            .unwrap_or_else(|| {
                eprintln!("--file {rel}: not found under {}", target.display());
                std::process::exit(2);
            }),
        None => match entries.iter().position(|e| !e.is_dir) {
            Some(i) => i,
            None => {
                eprintln!("no *.png files under {}", target.display());
                std::process::exit(2);
            }
        },
    };
    let path = entries[idx].path.clone();
    (Some(entries), idx, path)
}

/// Assemble the editor: pick the source view, then apply the startup flags.
fn build_studio(
    args: &Args,
    target: &Path,
    dir_mode: bool,
    entries: Option<Vec<Entry>>,
    open_idx: usize,
    path: PathBuf,
    img: Image,
) -> Studio {
    let manifest = if dir_mode {
        load_manifest_modes(target)
    } else {
        HashMap::new()
    };
    let (source, img) = if args.canvas_mode {
        match build_canvas(target) {
            Ok(Stitched {
                img,
                placements,
                owner,
            }) => (Source::Canvas { placements, owner }, img),
            Err(e) => {
                eprintln!("pixel_studio --canvas: {e}");
                std::process::exit(1);
            }
        }
    } else {
        match entries {
            Some(entries) => (
                Source::Tree {
                    entries,
                    sel: open_idx,
                    scroll: 0,
                },
                img,
            ),
            None => (Source::Sheet, img),
        }
    };
    let mut st = Studio::new(source, path, img, args.size);
    st.manifest = manifest;
    st.pal_idx = args.pal;
    st.backdrop_idx = args.backdrop;
    if dir_mode {
        st.root = Some(target.to_path_buf());
    }
    if let Some((cx, cy)) = args.cell {
        // any 8px cell is a legal origin — no even-cell snapping
        st.set_origin(cx * CELL, cy * CELL);
    }
    if args.demo_new {
        // screenshot/test hook: open the new-sprite modal prefilled
        st.new_sprite = Some(NewSprite {
            name: "items/moonfruit".into(),
            preset: 0,
            w: 8,
            h: 8,
            pal: false,
        });
    }
    st
}

/// `--shot`: render one UI frame headlessly, write it, print the title, exit.
fn run_shot(st: &mut Studio, out: &Path) {
    st.render();
    let img = Image {
        w: VIEW_W,
        h: VIEW_H,
        px: st
            .frame
            .iter()
            .map(|&p| [(p >> 16) as u8, (p >> 8) as u8, p as u8, 255])
            .collect(),
    };
    if let Err(e) = write_png(out, &img) {
        eprintln!("shot failed: {e}");
        std::process::exit(1);
    }
    println!("{}", st.title());
    println!("wrote {}", out.display());
}

fn run_gui(st: Studio) {
    let event_loop = EventLoop::new().expect("could not create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(st);
    event_loop.run_app(&mut app).expect("event loop error");
}
