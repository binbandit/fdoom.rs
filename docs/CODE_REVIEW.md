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

## Dispatch state
- G10 text anchoring — lane in flight.
- G5 (`render_gui`) — lane in flight.
- G9 (`pixel_studio`) — lane in flight.
- G1, G2, G3, G4, G6, G7, G8 — queued behind the crash-hunt lanes that currently
  own those files (one agent per file, hard rule).
