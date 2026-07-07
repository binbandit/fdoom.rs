# Implementation Audit Results — 2026-07-07

Independent verification of every request in `docs/REQUESTS_AUDIT.md`. Method per item:
locate the implementing code, confirm it is wired into live paths (not dead), run its
tests, and render/inspect visual evidence (headless test PNGs + FDOOM_DEMO screenshots
in `target/verify/`). Claims were **not** taken on trust from REQUESTS_AUDIT.md.

Statuses: **VERIFIED** (code + wiring + test/visual evidence) · **VERIFIED (code)**
(implementation confirmed by inspection; no dedicated automated test) · **PARTIAL**
(real but with a genuine gap) · **IN-FLIGHT** (agent actively working, expected pending)
· **PLANNED** (openly marked 📋, correctly not claimed done).

Full suite at audit end: **all green — 167 passed, 0 failed, 0 ignored** (25 lib unit
tests + 140 integration tests across 28 test binaries + 2 doctests; counts below).

## Foundation & controls

| Request | Status | Evidence | Notes |
|---|---|---|---|
| Full Java→Rust port, v0.1.0 tag | VERIFIED | `git tag` v0.1.0; suite green | Post-port era conventions followed |
| JavaRandom removed → xoshiro256++ | VERIFIED | `src/rng.rs`; no `rand` in Cargo.toml; zero `JavaRandom` hits in src/ | Deterministic per seed |
| E opens inventory | VERIFIED | `input_handler.rs:138` `("INVENTORY","E\|I")`; wired `player_behavior.rs:410`; tests `keymap_check::e_key_opens_inventory_mapping` + `e_opens_inventory_in_game` pass | End-to-end via TestWorld |
| SPACE attacks | VERIFIED | `input_handler.rs:136`; wired `player_behavior.rs:353,392`; lib test `multi_mapping_or_combines` pass | |
| R saves without crash | VERIFIED | Fix = deferred `g.pending_save` (`player_behavior.rs:441-448` → `game.rs:300-306`); test `save_hotkey::save_hotkey_saves_without_panicking` pass | Historical take-out-pattern panic documented in test header |
| Sane text entry | VERIFIED | `input_entry.rs:126-128`, `input_handler.rs:419-434`; unit test `typing_accumulates_and_backspaces` pass; menu-open guard blocks game actions (`player_behavior.rs:58-62`) | Gap: no integration test drives the world-name field; see defect #4 (same-tick chars overwrite) |
| Load World fixed | VERIFIED (code) | `world_select.rs:236-281` scans saves, version guard, copy/rename/delete; builds clean | No automated test for WorldSelectDisplay |

## World

| Request | Status | Evidence | Notes |
|---|---|---|---|
| Infinite worlds, dig-descent, no pre-placed stairs | VERIFIED | `tile/depth.rs:93-169` pit→chasm→ladder; `player_behavior.rs:106-137` + `world.rs:48-70` level change; lib test `no_preplaced_stairs_on_infinite_layers`; test `multi_level_terrain::dig_down_through_the_world` pass | Full descend + re-ascend loop exercised |
| …at negative coordinates | VERIFIED (code) | `chunk.rs:58-65` uses `>>`/`&` (floor-safe); `infinite_gen.rs` uses div_euclid/rem_euclid; `coords_round_trip` covers (-1,-1) | No integration test digs at negative coords (gap); one cosmetic `/16` bug found — defect #3 |
| Deep water + raft | VERIFIED | `depth.rs:47-61` gate wired via `dispatch.rs:259` → `behavior.rs:250` movement collision; recipe `recipe.rs:223`; test `deep_water_needs_a_raft` pass | Real collision gate, not cosmetic |
| Tides twice a day, tiles flip | VERIFIED | `tidal.rs:57-69` cosine 2 cycles/day; tests `tides` 5/5 incl. `fixed_tile_flips_submerged_and_exposed_across_the_day` | |
| Beachcombing at low tide | VERIFIED | `tidal.rs:120-168` exposed-only drops, radius-throttled; test `beachcombing_drops_are_throttled` pass | |
| Biomes (10, large, patchy edges) | VERIFIED | `infinite_gen.rs:221-276` (`biome_at`, `biome_at_blended` ±4-tile jitter); biome map viewed `target/verify/biome_map_42.png` | |
| Per-biome ground tint in lighting pass | VERIFIED | `gfx/lighting.rs:396-424` bilinear biome factor; `light_biome_desert_noon.png` shows patchy sand/grass interleave (viewed) | |
| Highland rock | VERIFIED | `infinite_gen.rs:121-124` + `rock.rs:53-57,212-226` (health 100 vs 50, +2 stone); test `highland_rock_takes_double_and_pays_extra` pass | |
| Summit snow any climate | VERIFIED (code) | `infinite_gen.rs:322-334`: `belt > 0.80 → snow` before temperature is sampled; Mountains gate is climate-free (`:250-254`) | No dedicated test for hot-climate summit |
| Mud/ponds/waves/marsh rims, tall grass, tree species, dried bushes, seaweed/reeds | VERIFIED | `flora_gen` 9/9 pass incl. `species_present_in_their_biomes` | |
| Natural food (berries regrow, mushrooms surface+mines, apples, cactus fruit, coconuts, pumpkins) | VERIFIED | `flora_gen::berry_bush_pick_and_regrow_cycle`, `mine_caves_grow_mushrooms`, `palm_drops_coconuts_when_felled`, `pumpkins_and_jack_o_lanterns_drop_their_items` all pass | Apples/cactus-fruit drop code inspected (probabilistic, untested) |
| Jack-o-lanterns craft + spawn | VERIFIED | Recipe `recipe.rs:192` (Pumpkin+Torch); spawn `structures_gen.rs:482-484,690-692`; tests `jack_o_lanterns_haunt_some_structures` + headless craft pass | |
| Rare events ×5 with real cues | VERIFIED | `core/events.rs`; Hollow Night (grave decay ×, quiet week), Aurora (spawn pause + `aurora_bands` render), Ember Rain (craters cool at dawn), Whisper Fog (marsh 2× pressure), Caravan (supply drops near trails); `world_events` 15/15 pass; `light_aurora_night.png` viewed | Aurora reads as green wash in a single static frame; banding is in code, sells in motion |
| Seasonal Halloween/Christmas, mockable date | VERIFIED | `events.rs:116-160` real Gregorian calendar; `date_override` + `FDOOM_DATE` mock; tests `mocked_date_drives_season_and_veil_cue`, `christmas_suppresses_hollow_night`, `halloween_doubles_night_events_only` pass | |
| Seed-random spawn time | VERIFIED | `world.rs:548-551` seed-hashed via xoshiro; empirically 8 seeds → 8.5%–99.8% of day | No repo test (external check only) |
| Day-cycle Classic/Long/Realtime changes pacing | PARTIAL | Divisors 1/4/80 applied `game.rs:326-336` + `weather.rs:246-256`; empirically 1000 ticks → 1000/250/12 | **Not persisted**: `save.rs write_prefs`/`load.rs load_prefs` never write/read `daycycle` — resets to Classic on restart. Zero test coverage. Defect #1 |
| Survival-only; sandbox/Air Wizard/score removed | VERIFIED | No AirWizard/Creeper/Slime/Skeleton in `EntityKind`; deleted mob files in git status; `load.rs:1197` skips removed kinds gracefully | |

## Structures

| Request | Status | Evidence | Notes |
|---|---|---|---|
| 13 layout variants | VERIFIED | `structures_gen.rs:78-91` sums exactly 13 (Ruins 3, Cemetery 3, Stones 3, Camp 2, Village 2); test `every_variant_occurs_with_its_signature_tiles` pass | Uniform hash selection, all reachable |
| Old trails | VERIFIED | `structures_gen.rs:735-860`; test `trails_link_nearby_structures_deterministically` pass | |
| Destroyed villages | VERIFIED | Blueprint `:594-707`; test `villages_are_ruined_clusters_with_plaza_well_and_chests` pass; 679 villages in 32k² window | |
| Boulders | VERIFIED | `boulder_at :867-879`; test `boulders_scatter_sparsely_and_straddle_chunks` pass | |
| Desert cemeteries | VERIFIED | `biome_ok :125-139` includes Desert; commit `fcfe1df` | Biome gate tested generically |
| +46% density | PARTIAL | Mechanism real + tested (`density_wave_raised_spawn_rates`) | Numbers disagree: commit says +46%, code comment `:112` and TERRAIN.md say ~+55%. Doc inconsistency only — defect #6 |
| Graves age → zombies | VERIFIED | `grave_stone.rs:83-127`; tests `hollow_night_greatly_accelerates_grave_decay`, `quiet_week_suppresses_grave_decay` pass | Zombie-spawn step code-verified, no isolated test |
| Worldview inspector (biome/tile, counts, R reroll) | VERIFIED | `src/bin/worldview.rs:444-531`; builds clean; prior session dumps in target/verify viewed | |

## Game systems

| Request | Status | Evidence | Notes |
|---|---|---|---|
| Bare-hands → crude-tools loop headless | PARTIAL | `crafting_chain` 4/4 pass; `crude_axe_outchops_fists_and_grass_yields_fibers` punches grass for real fiber drops via live dispatch; `early_loop_crafts_a_crude_axe` runs Stick→Cord→Sharp Stone→Crude Axe via real `Recipe::craft` | No single continuous gather→craft run through input/menu loop; material-gathering step uses `tw.give` in the craft test. Defect #5 |
| Workbench/anvil gating | VERIFIED (code) | Distance-bounded furniture interaction `player_behavior.rs:512-514,973-995` → `crafter_behavior.rs:8-29`; recipe-list separation tested (`personal_crafting_offers_the_survival_chain`) | No far-vs-near integration test |
| Weapons all fire + damage | VERIFIED | Spear melee reach+bonus & SHIFT-throw (`player_behavior.rs:589-690,777-788,1093`); crossbow needs anvil-forged Mechanism; all projectiles damage via shared `projectile_behavior::arrow_tick`; tests `projectile_weapons_damage_a_zombie`, `spear_throw_and_pickup_roundtrip` pass | |
| Recipes (cooking, bandages, fruit medley, raft, jack-o-lantern) | VERIFIED | `all_recipe_names_resolve_in_registry` sweeps all 7 stations; bandage/medley/jack-o-lantern/cooked-mushroom individually craft-simulated; `bandage_restores_health` pass | Raft covered by resolve sweep only |
| Mob roster: 4 removed, snake fixed, 4 originals | VERIFIED / PARTIAL (tests) | Removal total (enum, saves, sounds, art cells reused); snake `touched_by` dispatch fixed; all 4 originals wired into `try_spawn_pass` (`level/mod.rs:718-940`) | Gap: no natural-spawn test for marsh_lurker/feral_hound/stone_golem (night_wisp/ghost/snakes have them). Defect #2. Hound "pack" = grouped spawn + per-hound circle AI, no coordination (matches its own description) |
| Snake family + rattler warn→strike | VERIFIED | `snake.rs:71-122,209-297` two-phase coil→rattle→strike, 2× primed damage; test `rattler_warns_then_strikes` pass | |
| Ghost (grave rise, Hollow Night swarm, phase pulses, dawn banish) | VERIFIED | `ghost.rs:54-124`, `level/mod.rs:901-910`; 2 mob_life tests pass | |
| Fireflies roost + spook | VERIFIED | `fireflies.rs` Wander/Roost/Scatter machine; test `fireflies_spawn_at_dusk_and_spook_into_scatter` pass | |
| Grass stealth + night eye-glints | VERIFIED | `behavior.rs:948-980` renders 2-px glints only; pixel-sampling test `hostile_in_tall_grass_at_night_shows_only_eye_glints` pass | |
| Movement personalities alter movement | VERIFIED | `style_step` feeds real `mobai_move` (`behavior.rs:848-916`); Circle reroutes chase (`:1170-1222`); test `movement_personalities_wired` asserts distinct displacement | Read sites traced — not a dead field |
| Fossicking: pan + richness scaling | VERIFIED | `richness_at` (`infinite_gen.rs:101`) consulted in `fossick.rs:154`; bands widen with richness (`:101-120`); test asserts `finds(0.9) > finds(0.1)` | |
| Mineral-seep stains | VERIFIED (code) | `mineral_stain_at` (richness > 0.70) rendered `rock.rs:125-134`; same field raises underground ore density | No test references it |
| Vein sparkles | VERIFIED | `fossick.rs:264-276` ← `ore.rs:151`; test `vein_ping_marks_hidden_ore` pass | |
| Cracked/dense rock | VERIFIED | `fossick.rs:74-83` + `rock.rs:207-226` (30/50/80 HP); distribution + break-threshold tests pass | |
| Cave-ins + timber props prevent | VERIFIED | `collapse_check`/`fuse_tick` (`fossick.rs:198-259`), prop checked at arm AND fire time; test `collapse_triggers_without_prop_not_with` pass | |
| Skerries | VERIFIED | `skerry_at` (`infinite_gen.rs:131-133`) → `surface_tile :298-302`; lib test `ocean_has_skerries` pass | |
| Weather (deterministic, streaks, dimming, desert gate, growth boost, fish bubbles) | VERIFIED | `weather` 9/9 pass; `weather_rain_day.png` (diagonal streaks + dimming) and `weather_snow_tundra.png` (flecks) viewed; growth boost consulted in `farm.rs:65`, `berry_bush.rs:58` | |
| QOL: durability bar, death screen, info strip, notifications | VERIFIED | `hud_qol` pass; `hud_dur_full/low.png` viewed (green→red); death screen `player_death_display.rs:26-52` (time, score, Respawn/Main Menu — code-verified); info strip `inventory_menu.rs:60-76`; `hud_notification.png` viewed | Death screen has no screenshot test |
| Save toast bottom-right | PLANNED | No toast code in src/ | Correctly still 📋 |

## Presentation

| Request | Status | Evidence | Notes |
|---|---|---|---|
| Cave darkness | VERIFIED | `lighting.rs:145-147` (ambient 0.06); `light_cave_dark.png` viewed — near-black beyond player radius | |
| Dithered warm torchlight | VERIFIED | `light_torch_night.png` viewed — stippled warm edge, not gradient | |
| Beam through window; walls stop light; sealed rooms | VERIFIED | `light_shelter` 5/5 pass; `light_shelter_window_beam.png` viewed (lit tile past window, dark diagonal); `dispatch.rs:311-316` + occlusion mask `lighting.rs:616-686` | |
| Sunset keyframes | VERIFIED | `light_dawn/sunset_amber/sunset_violet/dusk.png` all viewed — distinct grades; ordering test pass | |
| Aurora curtains | VERIFIED | `aurora_bands` (`lighting.rs:811-832`); test + `light_aurora_night.png` viewed | Static frame reads as wash; banding needs motion |
| Sheet-art FOSSICKERS DOOM logo on title | VERIFIED | `target/verify/title.png` (fresh FDOOM_DEMO shot, viewed): red/orange wordmark lockup over flyover world | |
| Title flow: Continue/New World/Load World flat menu | VERIFIED | Same shot: `> CONTINUE (WOW) <` / NEW WORLD / LOAD WORLD / OPTIONS / HELP / QUIT; `most_recent_world` picks max mtime (`world_select.rs:45-61`); Continue label showed the actual newest save ("wow" in ~/fdoom/saves) | No automated test of mtime selection |
| Hints only on title | VERIFIED | Hints band visible in title.png; grep confirms no other display draws them | |
| World creation name+seed only, floats, seed hint | VERIFIED | `target/verify/gen_menu.png` viewed: NEW WORLD / name / seed / "(leave empty for a random seed)" / Create World over flyover; `world_gen_display.rs:121-131` | |
| Smoked-glass menus + slate frames | VERIFIED | `menu.rs:392-443` `darken_rect_screen(…,185)` default path used by pause/options/worldgen/death/select; visually confirmed via gen_menu.png + hud_notification.png band | |
| Readable books (opaque paper) | VERIFIED (code) | `book_display.rs:66-73` `set_frame_colors(554,…)` → opaque cream, black text; `menu.rs:400-402` explains the split | No screenshot test |
| Options: floats, no wear-suit row, no language row, daycycle present | VERIFIED (code) | `options_display.rs:19-38` rows = diff/daycycle/fps/sound/autosave + keybinds only | Language SettingEntry kept for sync() but never rendered |
| Multiplayer/temp displays removed | VERIFIED | Files deleted; zero references; builds clean | |
| Splash/title flyover, glass, footprint colors, edge blending, artgen sheet | VERIFIED | `artgen_sheet` 5/5, `biome_frames`, `headless_render` pass; title.png shows flyover; snow-print commit f5a58e7 | |

## DX / Meta

| Request | Status | Evidence | Notes |
|---|---|---|---|
| TestWorld harness used by tests | VERIFIED | `src/testutil.rs`; 21 of 28 test files use it | Non-users are non-gameplay tests |
| 1-line item declarations | VERIFIED | `registry.rs` `items.push(stackable(…))` one-liners; matches ADDING_CONTENT.md | |
| justfile verbs | VERIFIED | `just --list` clean; biome-map body run → `biome_map_42.png` viewed (plausible biome overview); `demo-title`/`demo-world` run → 3 screenshots captured & viewed | demo-world names the world "T" not "PIT" — defect #4 |
| pixel_studio | VERIFIED | bin exists; `pixel_studio` tests 2/2 pass | |
| Docs match reality | VERIFIED | TERRAIN.md (3 claims vs code: exact), ENTITIES.md (3 claims: exact), ADDING_CONTENT.md recipes accurate | ENTITIES.md missing Campfire variant = expected in-flight |
| FDOOM_DEMO scripted runs | VERIFIED | `src/platform/demo.rs` supports wait/key/type/shot/quit; exercised live this audit | See defect #4 for `type:` collapse |
| Fire & campfire wave | IN-FLIGHT | `cargo test --test fire` currently 9/9 pass (fuel/ember, cooking, stamina, spread/containment, rain extinguish, cold-camp embers, night smoke shot) | Agent still working; looks substantially complete already |
| Fishing wave | IN-FLIGHT | `cargo test --test fishing` currently 8/8 pass (all water kinds, deep table, bubbles > dead water, rain bite, tidal-flat gating, cook+heal) | Agent still working; looks substantially complete already |
| Beauty sweep (JAVA comments, interact dedup, dev console) | PLANNED | 289 `// JAVA:` comments remain; no dev console | Correctly still 📋 |
| Roadmap committed | VERIFIED | `docs/ROADMAP.md` exists (commit 7ef1d3f) | |

## Ranked genuine gaps / defects

1. **Day-cycle setting is not persisted** — `write_prefs`/`load_prefs` (`src/saveload/save.rs:98-123`, `load.rs:437-483`) never store `daycycle`; it silently resets to Classic every launch. The pacing itself works (verified empirically: divisors 1/4/80). Also zero test coverage. This is the only found defect a player would notice as "my setting didn't stick."
2. **No natural-spawn tests for 3 of the 4 original mobs** — marsh_lurker, feral_hound, stone_golem are correctly wired into `try_spawn_pass` (code-verified) but no test drives them through `level::try_spawn` the way snakes/ghost/wisp/fireflies are. A spawn-table regression would go undetected.
3. **Cosmetic negative-coordinate bug** — `src/entity/mob/player_behavior.rs:1143` uses truncating `/ 16` instead of `>> 4` for the swim-splash color lookup; at negative non-multiple-of-16 positions it samples the wrong tile (wrong splash tint only; movement/dig unaffected). Also: no integration test digs/descends at negative coords, though the chunk math itself is floor-safe.
4. **Typed-char buffer drops same-tick keystrokes** — `input_handler.rs:377-378` overwrites `key_typed_buffer` instead of queueing; consecutive FDOOM_DEMO `type:` steps collapse to the last char (the shipped `demo-world` recipe creates a world named "T", not "PIT"). Unreachable by human typing at 60fps, but it breaks scripted runs and would bite key-repeat edge cases.
5. **Crafting-chain test has a seam** — the gather half and the craft half are separate tests; the craft test injects materials via `tw.give`. No single continuous bare-hands→crude-axe run through the real input/menu loop.
6. **Doc inconsistency: structure density** — commit/REQUESTS_AUDIT say +46%, code comment (`structures_gen.rs:112`) and TERRAIN.md say ~+55%. Mechanism is real and tested; the two figures were never reconciled.
7. **Untested-by-automation visual/UI surfaces** — title/worldgen/options/death/book screens have no screenshot tests (verified manually this audit via FDOOM_DEMO); `most_recent_world` mtime logic, summit-snow-in-hot-climate, mineral-seep stains, and seed-random spawn time are code-verified only.

## Final full-suite run (`cargo test`, 2026-07-07 audit close)

**167 passed, 0 failed, 0 ignored.** Per binary:

| Binary | Passed | | Binary | Passed |
|---|---|---|---|---|
| lib unit tests | 25 | | lighting | 7 |
| artgen_sheet | 5 | | mining | 8 |
| attack_bounds | 1 | | mob_life | 11 |
| biome_frames | 2 | | multi_level_terrain | 2 |
| crafting_chain | 4 | | pixel_studio | 2 |
| display_flow | 1 | | save_hotkey | 1 |
| fire (in-flight wave) | 9 | | save_load_roundtrip | 2 |
| fishing (in-flight wave) | 8 | | structures_gen | 15 |
| flora_gen | 9 | | tides | 5 |
| gameplay_soak | 3 | | underground_gen | 2 |
| headless_render | 1 | | weapons_and_food | 5 |
| hud_qol | 2 | | weather | 9 |
| infinite_world | 1 | | world_events | 15 |
| keymap_check | 2 | | doctests | 2 |
| level_gen_determinism | 3 | | light_shelter | 5 |
