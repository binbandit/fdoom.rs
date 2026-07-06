# fdoom.rs

Rust port of the Java game "Fossickers Doom" (Fossicker repo). **Read `PORTING.md` first** —
it defines the architecture and the Java→Rust conventions. The Java source of truth lives in
a scratch clone outside this repo; when in doubt about behavior, defer to the Java code.

## Commands

- `cargo run` — run the game (windowed).
- `cargo test` — unit + headless render tests.
- `cargo clippy --all-targets -- -D warnings` — must stay clean.
- `cargo fmt` — rustfmt, default style.

## Porting conventions (mandatory)

1. **1:1 fidelity.** Port logic line-for-line where Rust allows. Keep constants, formulas,
   integer math (Java `int` = `i32`, `>>` = arithmetic shift on i32), string formats, and
   even quirky/buggy-looking behavior. Mark preserved quirks with `// JAVA:` comments.
2. **`g: &mut Game`** replaces all Java statics (`Game.*`, `Updater.*`, `World.*`,
   `Settings`, `Sound`). Renderer/Screens live outside `Game`; render fns take
   `(&mut Screen, &Game)`, tick fns take `&mut Game`.
3. **Entities** = `EntityCommon` + `EntityKind` enum; tick via the take-out pattern
   (`g.with_entity`). Java `instanceof` → matches!/predicates. Java `super.foo()` → call the
   parent layer's shared function.
4. **Naming**: Java camelCase → Rust snake_case; class names stay PascalCase. File layout
   mirrors the Java package layout (see PORTING.md module map).
5. **Randomness**: only `JavaRandom`. World gen uses its own seeded instances exactly as
   Java does; incidental randomness uses `g.random`. Never `rand` crate.
6. **No new dependencies** without updating PORTING.md. Platform code (winit/softbuffer/
   rodio) is confined to `src/platform/`.
7. Doc-comment each ported item with a short note of its Java origin only when the mapping
   is non-obvious (renames, merged classes). Don't restate the Java file for straight ports.
