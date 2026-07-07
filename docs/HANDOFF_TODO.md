# HANDOFF TODO — continuation state for the next session/agent

Context: a long multi-agent build session. Everything committed is verified green.
Four agent lanes were IN FLIGHT at handoff with uncommitted work in the tree; their
scopes and how to finish each are below. Conventions: conventional commits, one lane
per commit, run `just check` before every commit, verify visuals by screenshot
(FDOOM_DEMO or TestWorld — see docs/DEV_GUIDE.md), never `git add -A` while other
work is in flight (stage by lane's file list).

## 1. IN-FLIGHT LANES (uncommitted work in the tree — finish, verify, commit)

### 1a. Comment sweep + tile dedup  (src/level, src/entity, src/item, src/saveload, src/screen, src/core minus renderer)
- Goal: remove ALL `// JAVA:` comments (delete pure provenance; rewrite real knowledge
  as plain maintainer comments), extract the repeated pay_stamina+pay_durability+
  set_tile+drop+sound tile-interact shape into one helper, delete dead/wrong comments.
- If the tree doesn't compile here, it's this lane's half-migrated helper. Finish the
  migration, `grep -rn "JAVA:" <its files>` must be 0, suite green, commit
  "refactor: retire JAVA comments; dedupe tile tool-use".
- NOT in scope: src/gfx/** + src/core/renderer.rs comments (~few sites) — sweep them
  after 1c lands.

### 1b. Sprite decomposition + artgen deletion  (assets/sprites/**, src/gfx/sprite_sheet.rs, src/assets.rs, docs/ART_GUIDE.md, tests/sprite_atlas.rs)
- Goal: cut assets/sprites.png into per-sprite PNGs under assets/sprites/{tiles,mobs/
  <mob>,items,ui,font,logo,fx}; manifest.txt pins legacy cells; stitcher composes at
  load (growable atlas, name lookup SpriteSheet::cell("items/berry")); GOLDEN TEST:
  stitched == old sheet byte-identical; DELETE src/bin/artgen.rs + tests/artgen_sheet.rs
  (replace with manifest-integrity test); write docs/ART_GUIDE.md.
- The stitcher in src/gfx/sprite_sheet.rs looked complete at handoff; remaining work
  was likely the file export + manifest + deletion + docs. Verify golden test passes,
  the game runs identically (screenshot A/B vs a pre-change build), commit.

### 1c. Visual excellence  (src/gfx/lighting.rs + siblings, src/core/renderer.rs hooks, tests/visuals.rs)
- PRIORITY 0 (user-reported): REAL boundary blending — dithered color-carry strips
  (~4-6px, Bayer-ramped) where ground families meet (snow bleeds into grass etc.);
  the shipped multiplier blend is invisible (see target/verify/blend_check_6x.png).
  Verify on a grass/snow freckle field at 6x.
- Then the effect menu (ship only what passes taste + <400us budget): golden-hour long
  shadows, entity contact shadows, night emitter halo, sun glitter on water, heat
  shimmer (lava/desert noon), falling leaves/pollen, torch breathing, mine depth fog.
- A/B screenshots per effect; commit per taste-approved bundle.

### 1d. Pixel studio v2  (src/bin/pixel_studio.rs, tests/pixel_studio.rs, DEV_GUIDE)
- Round-2 features (all user-requested): palette-applied preview (real game palettes:
  player get4(-1,100,shirt,532), zombie LVLCOLS, TOOL_LEVEL_COLORS), in-context
  backdrop previews, animation playback of sibling frames, onion skin, line/rect/
  copy-paste/nudge/mirror-draw/shade-shift tools, wheel zoom + pan + help overlay.
- BUG FIX (user-reported): sheet-mode 16x16 selection snaps to even cells and splits
  sprites (half tree/half pumpkin) — free per-cell origin + sprite-origin map + G snap.
- ATLAS CANVAS MODE (user-required): stitch ALL split files into one editable canvas
  (edits route to owning files, per-file dirty tracking, save writes only dirty files).
- SPRITE NAMES (user-requested): selected sprite's name first/large in the header,
  hovered sprite's name in a secondary slot, all modes.

## 2. QUEUED WAVES (not started)
- CREATIVE-DIRECTOR PASS: play via FDOOM_DEMO across ~12 scenarios, screenshot, write
  PLAYTEST.md critique, implement top improvements. Also owns audit debt: natural-spawn
  tests for marsh_lurker/feral_hound/stone_golem; one continuous bare-hands->crude-axe
  test; screenshot coverage for title/worldgen/options/death/book screens.
- DEV CONSOLE (--debug): tile/biome/seed inspector overlay, give-item, teleport,
  set-time. (Part of the DX phase-2 plan; renderer must be free.)
- gfx/renderer JAVA-comment cleanup (after 1c).
- Art follow-ups queued for after 1b lands (edit the split PNGs): tiny mushrooms
  (several 3-4px buttons per tile, user-requested), 2-3 flower species/colors,
  dedicated Prospector's Pan/Timber Prop/fish (fatter fish, pale eel)/window icons,
  wet-sand tidal cells (TODO(art) comments mark all sites).

## 3. VERIFICATION DEBT
- Swim visual: user said "swimming looks really weird" — reef-swim RULE was fixed +
  audit passed code checks, but nobody has visually confirmed the swim RENDER since
  the traced player sprites landed. Screenshot a swimming player and judge.
- Boundary blend re-check after 1c priority-0 lands (the user must SEE it this time).
- Whole-game hero screenshots after everything lands (rainy night campfire, Hollow
  Night cemetery, sunset lakeshore) — good README material.

## 4. KNOWN QUIRKS / GOTCHAS FOR THE NEXT DRIVER
- Session/user history: user cares MOST about art quality (player sprite had to be
  pixel-traced from the original after 3 failed redesigns — NEVER freestyle the player
  or classic mob anatomy), terrain texture taste = "calm base, sparse clustered
  detail", and visible results (screenshot everything).
- Agents stall on infra sometimes — their transcripts survive; resume by re-stating
  where they were. Never let two agents own one file; registry.rs is the usual
  collision point.
- docs/REQUESTS_AUDIT.md + docs/AUDIT_RESULTS.md track every user request and its
  evidence; update both when closing items.
- The tree at handoff may not compile (lane 1a mid-refactor). That lane's completion
  fixes it; don't "fix" half-migrated call sites independently.
