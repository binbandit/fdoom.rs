# fdoom.rs

See also: `docs/` — ARCHITECTURE.md (codebase tour), DEV_GUIDE.md (commands, FDOOM_DEMO
scripted runs, headless testing, cheat keys), ADDING_CONTENT.md (item/tile/mob recipes).

Rust port of the Java game "Fossickers Doom" (Fossicker repo). **Read `PORTING.md` first** —
it defines the architecture and the Java→Rust conventions. The Java source of truth lives in
a scratch clone outside this repo; when in doubt about behavior, defer to the Java code.

## Commands

- `cargo run` — run the game (windowed).
- `cargo test` — unit + headless render tests.
- `cargo clippy --all-targets -- -D warnings` — must stay clean.
- `cargo fmt` — rustfmt, default style.

## Porting conventions (mandatory)

1. **Post-port era** (after tag `v0.1.0`): the codebase no longer preserves Java quirks
   for their own sake — prefer clear, idiomatic Rust and fix inherited bugs. `// JAVA:`
   comments remain as provenance notes. When porting any *remaining* Java behavior,
   match it first, then improve deliberately.
2. **`g: &mut Game`** replaces all Java statics (`Game.*`, `Updater.*`, `World.*`,
   `Settings`, `Sound`). Renderer/Screens live outside `Game`; render fns take
   `(&mut Screen, &Game)`, tick fns take `&mut Game`.
3. **Entities** = `EntityCommon` + `EntityKind` enum; tick via the take-out pattern
   (`g.with_entity`). Java `instanceof` → matches!/predicates. Java `super.foo()` → call the
   parent layer's shared function.
4. **Naming**: Java camelCase → Rust snake_case; class names stay PascalCase. File layout
   mirrors the Java package layout (see PORTING.md module map).
5. **Randomness**: only `crate::rng::Rng` (deterministic per seed). World gen seeds its
   own instances; incidental randomness uses `g.random`. No `rand` crate dependency.
6. **No new dependencies** without updating PORTING.md. Platform code (winit/softbuffer/
   rodio) is confined to `src/platform/`.
7. Doc-comment each ported item with a short note of its Java origin only when the mapping
   is non-obvious (renames, merged classes). Don't restate the Java file for straight ports.
