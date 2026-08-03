# Grumpy code review — 2026-07-15

Structural review of the whole codebase (51k lines src, 19k lines tests, 16 docs),
written after a QA pass turned up a hard crash the test suite was happy about.
Ordered by how much damage each one does. Every claim has a number behind it.

**The meta-problem:** 435 green tests coexisted with "resize the window → instant
crash". The suite tests what was built, at the size it was built for, through the
APIs it was built with. It does not test what a player does. Coverage of
*mechanisms* is good; coverage of *the product* is thin.

---

## G1. Stringly-typed everything — 332 + 131 runtime lookups
`g.tiles.get("hole")`, `registry::get(g, "Leather Armor")` — 332 tile lookups and
131 item lookups resolved from string literals **at runtime**, including inside
per-tile neighbour checks in tick and render paths.

Each call `to_uppercase()`s into a fresh `String`, then **linear-scans up to 256
entries**. A typo is not a compile error: `Tiles::get` prints `TILES.GET: invalid
tile requested` to stdout and silently returns **tile 0 (grass)**; `registry::get`
returns a NULL item. That is a content bug that ships looking like a gameplay bug.

**Fix:** intern once. `TileId`/`ItemId` newtypes + consts for the ~80 known tiles,
resolved at startup; keep string lookup only at the save/load and dev-console
boundary, where it belongs, and make it return `Result`/`Option` there.

## G2. `Game` is a 58-public-field god object
Every subsystem reaches into every other through `g.`. No invariants, no
encapsulation, no way to reason about what a function touches from its signature.
It is also why `&mut Game` threads through 100+ functions.

**Fix:** group into cohesive sub-structs (`world`, `time`, `ui`, `audio`,
`session`) and privatize anything with no outside reader. Mechanical, wide, worth it.

## G3. 101 × `#[allow(clippy::too_many_arguments)]`
Not a lint problem — a design problem. Functions take 8–10 positional `i32`s
(`x, y, w, h, x_scroll, y_scroll, col, bits, …`). Positional integer soup is
exactly how the resize crash happened: two of those integers (`xo`, `yo`) went
negative and nothing in the type system cared.

**Fix:** small structs — `Rect`, `DrawCtx`, `TileCtx`, `BlitParams` — at the worst
offenders. The lint count is the to-do list.

## G4. 123 `unwrap()/expect()` + 72 `println!` in engine code
No logging layer and no error type. Diagnostics go to a stdout nobody reads while
playing; failures panic in the player's face. `expect("tile 0 must exist")`,
`expect("player entity missing")`, `unwrap()` on lookups that a bad save can miss.

**Fix:** a tiny `log` module (levels + one place to route), `Result` at the real
boundaries (save/load, asset load, world create), and the in-game notification
system for anything the player should actually know.

## G5. Monster functions — bugs hide here
`renderer::render_gui` **467 lines**, `registry::build_registry` 358,
`item::interact_on_tile` 356, `Game::tick` 249, `infinite_gen::surface_tile` 244,
`level::try_spawn_pass` 233, `Recipes::new` 226. The production pack crash lived in
this exact kind of function. Decompose by concern, not by line count.

## G6. Test suite: 99 copy-pasted helpers across 64 files
`deer_eid`, `pixel_at`, `count_*`, world-staging boilerplate — reinvented per file.
That is how the deer-flee flake got written four times over. Promote to
`testutil`; a shared helper gets fixed once.

## G7. The take-out tick pattern is a footgun
`with_entity` removes an entity from the arena while it ticks, so any lookup of the
ticking entity returns `None`, and any id held across a tick can dangle. It is
load-bearing and documented, but it is a permanent bug generator — every
"entity vanished mid-operation" bug traces here.

**Fix (cheap):** a guard type / accessor that makes "this entity is currently out"
explicit and impossible to mistake for "this entity is gone".

## G8. Java-port residue
`get_score()`/`set_score()` accessor pairs over what are effectively public fields;
`Rc<RefCell<…>>` (44 uses) where ownership is unambiguous; hand-rolled `Point`,
`Rectangle`, `Dimension`; inconsistent numeric types (tile ids `u8`, levels
`usize`, depths `i32`, coordinates `i32` with `i64` bolted on where overflow bit).

## G9. `pixel_studio.rs` is 3,783 lines in one file
This is the user's long-term independence tool — the thing they keep the game with
after the model that wrote it is gone. It deserves the best structure in the repo
and currently has the worst.

## G10. The resize migration was never swept
Dynamic resolution landed screen-by-screen. Left behind: `FontStyle` anchoring
every centered draw to the classic 288×192 centre, six screens positioning against
`screen::W/H` constants, and a screenshot dumper hardcoded to 288×192. Symptom the
owner reported: "text not fitting inside containers… text flowing out of the
screen". One sweep, one rule: **no layout math may read the classic constants.**

---

## Dispatch state (updated 2026-07-15)
- **G5 `render_gui`** — DONE (78fb5e1): 436 lines -> 40, 23 named units, proven
  pixel-identical over 296 hashed HUD frames; carried a live fix for the dungeon
  backdrop leaving stale pixels at large sizes.
- **G9 `pixel_studio`** — DONE (68597ed): one 3,783-line file -> 20 documented
  modules, zero behaviour change proven three ways, 53 new unit tests.
- **G10 classic-constant sweep** — DONE (35cd554, 5ff258d, 91f546d, 06e30d9):
  per-axis draw-time anchoring, seven screens, the scale policy, the unpainted
  viewport strip, and the black-on-black book pages.
- **G1 interning** — DONE (fa718be): 77 TileId consts + 230 checked ItemName
  consts, 348/353 in-lane sites converted, world generation proven byte-identical
  across 45 seed/layer hashes. Caught a live bug: `tiles.get("farm")` matched
  nothing and had been silently returning grass on every world gen.
- **G4 logging + unwrap audit** — DONE (ad26d9f): src/core/log.rs (4 levels, no
  deps, zero-cost when suppressed), 61 printlns converted, 97 unwrap/expect sites
  audited (76 made non-panicking), and 29 reachable panics + 1 infinite-loop hang
  fixed.
- **G3 param structs** — partly absorbed by G5/G9; the remaining ~95 suppressed
  lints stay on the list.
- **G2, G6, G7, G8** — queued; each needs files the in-flight lanes hold.

### Added by QA (2026-07-15)
- **G11. The test suite was not measuring the product.** 435 tests were green
  while the window could not be resized without crashing, mobs panicked the game
  every tick at distance, a corrupt prefs file made it unstartable, and book text
  rendered black on black. Every one was found by *playing it or fuzzing it*, not
  by unit tests. Standing rule going forward: a user-facing claim needs a
  screenshot or a scripted play session behind it, not a green suite.
- **G12. Wall-clock randomness** (fixed, 06e30d9): levels and the game seeded RNG
  from the clock, so the same save rolled differently every run and the suite
  flaked ~1 in 5. Runtime rolls now derive from the world seed.
- **G13** — FIXED (in 06e30d9 follow-up): sprites now sort by (y, eid).
  Original finding: sprites sorted by `y` alone, so equal-`y` entities draw in HashMap order —
  genuinely nondeterministic between processes. Makes PNG comparison unreliable as
  a regression signal. Needs a stable eid tiebreak. OPEN.
- **G14. `gfx/lighting.rs` holds a process-wide `static DISABLED_FX: AtomicU32`** —
  shared mutable state across parallel tests. OPEN.
