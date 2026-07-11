# HANDOFF TODO — precise continuation plan

Purpose: any agent/session can finish this program of work with no ambiguity.
Every item states WHAT, HOW (method + files), and DONE-WHEN (acceptance criteria).

## Standing rules (apply to every item)
- Verify before commit: `just check` (fmt --check, clippy -D warnings, full suite).
- Visual claims require screenshots you have LOOKED at: FDOOM_DEMO recipes are in
  docs/DEV_GUIDE.md; headless frames via testutil::TestWorld::screenshot; upscale with
  `sips -Z 1152 -s format png in.png --out out_6x.png` and Read the image.
- One lane = one conventional commit; stage the lane's explicit file list, never
  `git add -A` while anything else is uncommitted.
- Art taste rules (user-enforced, repeatedly): player + classic mob anatomy is
  pixel-traced from the original — NEVER redesign it; terrain texture = calm base +
  sparse clustered detail (no uniform dither); every effect must read as deliberate
  pixel art at 1x; when in doubt, A/B screenshot against the previous commit.
- Track closure in docs/REQUESTS_AUDIT.md (+ evidence style of docs/AUDIT_RESULTS.md).

## 1. IN-FLIGHT LANES (uncommitted edits exist — finish, verify, commit)

### 1a. JAVA-comment sweep + tile tool-use dedup — DONE
Landed across 802a352 (level/entity/item/saveload + tool_use dedup) and the follow-up
sweep commit (screen/core/platform/renderer — 2c folded in). `grep -rn "JAVA:" src/`
is 0 repo-wide. Note: ~720 `Java \`X\`` API-mapping doc-comments remain by design
(they map fns to their v0.1.0 origins); revisit only if the user asks.
Original spec (for reference):
Scope files: src/{level,entity,item,saveload,screen}/**, src/core/** EXCEPT renderer.rs.
Method:
  1. `grep -rn "JAVA:" <scope>` — for each site: DELETE if it only cites Java origin;
     REWRITE in plain terms if it explains quirk semantics, formula rationale,
     ordering/reentrancy hazards, or wire formats. No "Java" in rewrites. Module docs
     may keep one line: "Originally ported from the Java version (tag v0.1.0)".
  2. Dedup: ~15 tile interact fns repeat pay_stamina -> pay_durability -> set_tile ->
     drop_item -> play_sound. Create ONE helper in src/level/tile/mod.rs or dispatch.rs
     (suggested: `pub fn tool_use(g, lvl, xt, yt, player, item, spec: ToolUseSpec) ->
     bool` where ToolUseSpec { tool: ToolType, stamina: i32, result_tile: &str,
     drops: &[(&str, RangeInclusive<i32>)], sound: Sound }). Migrate the clear cases
     (dirt, grass turf, sand, snow, cloud, mud, tree-family harvest cores); leave
     divergent ones (fossick pan, pumpkin, depth pits) alone.
  3. Delete commented-out dead code + comments describing removed features (sky,
     score, pre-placed stairs).
Done-when: grep count of "JAVA:" in scope == 0; net LOC negative; suite green;
  commit `refactor: retire JAVA comments; extract tile tool_use helper`.
State at handoff: mid-migration; lib may not compile (E0061 half-updated call sites).
  Finish the migration — do NOT band-aid individual call sites.

### 1b. Sprite decomposition + artgen deletion
Scope files: assets/sprites/** (new), assets/golden_atlas.png (renamed fixture),
  src/gfx/sprite_sheet.rs (stitcher — ALREADY WRITTEN at handoff, review it),
  src/assets.rs, tests/sprite_atlas.rs, docs/ART_GUIDE.md, justfile; DELETE
  src/bin/artgen.rs + tests/artgen_sheet.rs.
Method:
  1. Export: decode assets/sprites.png; cut every non-empty cell region into files per
     the artgen header inventory (read src/bin/artgen.rs header comments — it lists
     every cell owner). Naming: assets/sprites/{tiles,mobs/<mob>,items,ui,font,logo,fx}/
     <snake_name>.png. Multi-cell sprites exported whole (tiles 16x16; mob frames
     16x16 each as <pose>_<frame>.png; connector sets one strip per set, piece order
     documented in ART_GUIDE; logo strips whole; font one strip).
  2. Manifest assets/sprites/manifest.txt: `<path> <cx> <cy> <w> <h> <pal|rgb>` per
     file (mode from the header's palette/true-color notes).
  3. Stitcher already in sprite_sheet.rs: pinned files -> exact cells; unpinned ->
     shelf-packed rows >= 32; name lookup via SpriteSheet::cell("items/berry").
     Runtime: dev reads the folder if present; release embeds via a generated
     include_bytes table (build.rs or a checked-in generated module).
  4. Golden test (tests/sprite_atlas.rs): stitch(manifest, all files) produces
     pixels IDENTICAL to decoding assets/golden_atlas.png. Plus manifest integrity:
     all files exist, sizes match, no pin overlaps, `pal` files contain only
     {0,85,170,255}+transparent.
  5. Delete artgen + its test; move its cell-inventory knowledge into manifest
     comments; scrub artgen mentions from justfile/docs (leave pixel_studio docs to 1d).
  6. docs/ART_GUIDE.md: folder map, manifest format, pal-vs-rgb rules (gray ladder
     0/85/170/255), pixel budgets, add-an-item walkthrough (draw in studio -> save to
     items/foo.png -> registry one-liner; no manifest edit needed), acceptance
     checklist, atlas rationale (flat-array perf, ~0.5MB).
Done-when: golden test green; game screenshot byte-comparable to pre-change build;
  `grep -rn artgen` only in git history/docs-as-history; suite green. Commit
  `feat(art)!: per-sprite PNG sources, stitched atlas; artgen removed`.

### 1c. Visual excellence (gfx)
Scope files: src/gfx/lighting.rs (+ optional new gfx modules), renderer.rs hooks,
  tests/visuals.rs, docs/RENDERING_AND_UI.md.
PRIORITY 0 — real boundary blending (user: "I don't see the blending"):
  Where ground FAMILY changes across a tile edge (families: grass/sand/snow/mud/dirt),
  overlay a 4-6px color-carry strip on BOTH sides: pixels lerp toward the NEIGHBOR
  family's representative color (hardcoded table sampled from base art), masked by
  Bayer dither ramping 100%->0% over the strip. Corners blend both axes. Keep the
  existing corner-multiplier blend underneath.
  Done-when: on a tundra-fringe scene (snow freckles in grass, e.g. seed BRD from
  target/verify/blend_check_6x.png), a 6x screenshot shows snow bleeding white
  speckle into grass and vice versa — obvious at a glance, art still crisp.
Then the effect menu, in order, each A/B-screenshot-judged, ship-or-cut:
  golden-hour long shadows (1-tile dithered strips E/W from blocks_light tiles+trees,
  direction flips am/pm); entity contact shadows (2px dithered ellipse, skip
  swimmers/ghost/wisp); night emitter halo (one extra quantized warm dither band);
  sun/moon glitter path on water (world-anchored band along sun azimuth); heat shimmer
  (lava always, desert noon: per-row 1px offset oscillation, never on UI rows);
  falling leaves/pollen motes (3-6 on screen, forest/plains, disabled in rain via
  weather API); torch breathing (±0.5 tile radius oscillation, lanterns steady);
  mine depth fog (blue-noise dithered darkness bands beyond lit area).
Budget: whole pass < 400us release worst-case (assert in tests/visuals.rs; currently
  ~163us before these).
Done-when: per-effect verdicts reported, hero shots exist (sunset lakeshore, night
  torch scene), suite green. Commit per bundle:
  `feat(gfx): boundary color-carry blending` then `feat(gfx): atmosphere effects`.

### 1d. Pixel studio v2 — LANDED (committed); REMAINING DEBT: extended tests only
Remaining: canvas-mode multi-file save roundtrip test + nudge/copy-paste correctness
tests + odd-origin selection regression test (features shipped; tests were being
written when the agent was stopped). Original spec below for reference.
Scope files: src/bin/pixel_studio.rs, tests/pixel_studio.rs, DEV_GUIDE section.
Features (ALL user-requested; in priority order):
  1. Palette-applied preview: cycle P through None / player (get4(-1,100,<shirt>,532))
     / zombie LVLCOLS 1-4 / tool tiers (TOOL_LEVEL_COLORS) — canvas + preview strip
     render palette-mode pixels through the chosen palette exactly like
     Screen::render does.
  2. In-context preview: composite the sprite over grass-day / sand / night-graded
     grass backdrops in the preview strip.
  3. Animation: A toggles playing sibling frames (mobs/<mob>/*) at game walk cadence.
  4. Onion skin: O toggles a 30% ghost of a reference file (B to pick).
  5. Tools: line (L+drag), rect (R / Shift-R fill), copy/paste (Ctrl-C/V, drag-place),
     nudge whole image (Shift+arrows, wrap), mirror-draw (M), shade-shift ([ / ]:
     grays step the 4-shade ladder, colors ±16/channel).
  6. UX: wheel zoom at cursor, middle-drag pan, '?' help overlay, recent-colors row,
     Ctrl-S alias, unsaved-exit warning.
  7. BUG (user-reported): sheet-mode multi-cell selection must have FREE per-cell
     origin (no even-snap — sprites live at odd cells; "half tree half pumpkin");
     add sprite-origin map + G = snap-to-sprite-under-cursor.
  8. ATLAS CANVAS MODE (user-required): all split files stitched into one editable
     canvas (layout pluggable: folder-grouped now, manifest order once 1b lands);
     paints route to owning file; per-file dirty tracking; S saves only dirty files
     (+session .bak each); eyedrop/copy-paste across sprite boundaries.
  9. NAMES (user-requested): selected sprite name FIRST and largest in the header
     ("Berry — items/berry.png" / "Pumpkin (2x2)"); hovered name in a secondary slot.
Done-when: all 9 shipped, round-trip tests extended (canvas-mode edit spanning two
  files saves both, untouched files bit-identical), bin builds clippy-clean, DEV_GUIDE
  updated. Commit `feat(tools): pixel_studio v2 — canvas mode, previews, real tools`.

## 2. QUEUED WAVES (start after the above)
### 2a. Creative-director pass (user grant: "see how it can be improved and extended, then do it")
  Method: scripted play-sessions (FDOOM_DEMO) across: fresh spawn day + night, each
  biome, mining trip incl. cave-in + props, rainy night at a campfire, Hollow Night
  cemetery, ocean raft + tides + fishing hotspot, village+trail loot run, window-lit
  house at night. Screenshot everything; write docs/PLAYTEST.md (what confuses, what
  lacks feedback, what's flat); rank fixes; implement the top items (expect: hit
  feedback juice, notification polish, pacing tweaks); also close audit debt:
  natural-spawn tests for marsh_lurker/feral_hound/stone_golem; one CONTINUOUS
  bare-hands->fibers->cord->knap->crude-axe test with zero tw.give; screenshot tests
  or manual verification for title/worldgen/options/death/book screens.
  Done-when: PLAYTEST.md committed with before/afters; improvements committed; audit
  docs updated.
### 2b. --debug dev console — DONE (6f2e6aa: F4 overlay + / command line;
  give/tp/time/heal; tests/dev_console.rs; documented in DEV_GUIDE)
  Overlay (toggle via a --debug key): tile name+data under player, biome, seed, day/
  clock, fps; commands: give <item> <n>, tp <x> <y>, time <morning|noon|dusk|night>,
  heal. Method: small module + renderer hook + input capture in debug only.
  Done-when: demoable via --debug run; documented in DEV_GUIDE.
### 2c. gfx/renderer JAVA comments — DONE (folded into the 1a completion commit).
### 2d. Art follow-ups on the split files (after 1b; edit PNGs via studio or scripts):
  tiny mushrooms (3-4px buttons, several per tile — user asked twice), flower variety
  (2-3 species/colors — either data-variant render or separate tiles), dedicated
  icons: prospector's pan, timber prop, window, big fish (fatter), cave eel (long,
  pale), wet-sand tidal cells. All TODO(art) comments in code mark exact sites.

## 3. VERIFICATION DEBT (cheap, do opportunistically)
- Swim render: screenshot a swimming player; user said it "looks really weird" once —
  rule fixed + audited, render never re-judged since sprite tracing. If ugly: the swim
  clip draws top-half only + splash ring — check MobSprite frame choice vs traced cells.
- Boundary blend re-check with the user after 1c P0.
- Hero screenshots for README after everything lands.

## 4. GOTCHAS
- Never two agents on one file; registry.rs is the classic collision.
- Agents die on infra stalls/quota: resume with a one-line "you stalled, continue from
  <last state>" message; transcripts survive.
- Old prefs/saves in ~/fdoom can mask keymap/settings changes during testing — rm -rf
  ~/fdoom for clean runs (it's the game dir on macOS).
- FDOOM_DEMO menu navigation depends on current menu layouts (title: Continue only
  when saves exist; worldgen: name -> seed -> create with blanks between).
- The user reads screenshots: show, don't tell.
