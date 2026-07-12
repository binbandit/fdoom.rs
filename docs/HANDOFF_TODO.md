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

## 1. PROGRAM STATE (2026-07-12) — original port+evolution program COMPLETE

Everything in the old sections 1-3 landed. Since then the project runs as a
PM-directed wave board (this session's tag: product taste rewritten in CLAUDE.md —
DayZ/7DtD north star + Minecraft approachability + world-coherence rules).

### Landed waves (commit refs)
- Creative-director top-10 (see PLAYTEST.md): combat juice e1427d4, notifications
  4dacdd9/9a05e5d, saves fix 0a460f3, cave-ins 3e38de0, map e525614, village light +
  flora grounding 94bae2d, first-day thread 660f09f, onboarding bugs fd32989.
- Biome coherence: climate-gated adjacency 17af522 (snow never borders sand),
  heath highlands + marsh identity 3194b4f + 494000d, snow accumulation/thaw 24b700b.
- Fishing feedback + spawn-light floor 88c9bbc; organic ragged holes 5364a72;
  excavation (merged digs, flooding, base-in-hole) a628082.
- Towns & scavenge (age axis, hamlets, containers, can/bottle/tin) 5df1fec.
- Temperature (7 bands, mitigations, 3-heart floor) 2e837da.
- Art 2d follow-ups 3ff1fe5 (tiny mushrooms, flower species, wet sand, icons);
  pixel studio v3 ff86b95 (real canvas mode; DEV_GUIDE manual).
- Oddity audit e1c6c00 (docs/ODDITIES.md, 27 findings) + O1-O3 fixes be7b198.
- UI redesign doc 634dbbd (docs/UI_REDESIGN.md: frameless HUD, E survival screen,
  THE BENCH) — implementation lanes in flight.

### In flight at handoff
- Fog (morning mist / afternoon haze / regional banks) — weather.rs + gfx.
- Farming & cooking merge (worktree agent-ab88391e82abab248: 4 crops, campfire/oven
  cooking, Queasy; rebasing onto main tip — tile ids renumber past heath's 67).
- (all in-flight lanes above have since LANDED)

### In flight (product-owner expansion, 4 worktree lanes + 1 deferred)
- Content wave 2: hot springs, abandoned mine shafts, bees/honey, badlands.
- Rivers: winding pure-field waterways, pannable banks, trail bridges.
- Hunting + field notes: deer (stalk via tall grass), venison/hide->leather,
  NOTES tab (days/biomes/places/events soft goals, tolerant-append save marker).
- Severe weather: blizzards (+cold band, fast settle, campfire sanctuary) and
  thunderstorms (telegraphed lightning, self-limiting fires, 8-tile player floor).
- DEFERRED until the above merge: structures_gen -> structures/ module split
  (spec in scratchpad/cleanup_spec.md section 2a; part 1 landed b26031c).
Merge order: by completion; I arbitrate shared files (weather/lighting/registry/
recipe/survival_display); registry+recipe edits are appended blocks by convention.

### Queue (dispatch order)
1. ODDITIES.md noticeable tier (14 items), then nitpicks (O1-O5 + waterlines DONE).
2. UI L6: gentle thirst (water bottles exist; HUD slot reserved at y=182).
3. Field-notes recipe VARIANTS layer (additive only — never gates progress).
4. Armor free-repair loophole (re-equip refreshes hits; needs item-data change —
   flagged by L3).
5. Bench nice-to-haves: dedicated bench/module art (TODO(art) marks), fit-from-
   screen (hold-to-fit shipped), in-game sprite reload key (studio v3 note).

### UI redesign program: COMPLETE (design 634dbbd; L1 5fb146c; L2 2e553cd;
### L3 4ba7f3e; L4 deb066c incl. text-overflow rule; L5 14dfa5c THE BENCH).
### README + hero shots: b1609fe. Fog: bbc59ed. Farming: 8aec45c.

## 4. GOTCHAS
- Never two agents on one file; registry.rs is the classic collision.
- Agents die on infra stalls/quota: resume with a one-line "you stalled, continue from
  <last state>" message; transcripts survive.
- Old prefs/saves in ~/fdoom can mask keymap/settings changes during testing — rm -rf
  ~/fdoom for clean runs (it's the game dir on macOS).
- FDOOM_DEMO menu navigation depends on current menu layouts (title: Continue only
  when saves exist; worldgen: name -> seed -> create with blanks between).
- The user reads screenshots: show, don't tell.
