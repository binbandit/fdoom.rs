# Request Audit — every user request this project cycle

Status: ✅ done & verified · 🔄 in flight (agent working now) · 📋 planned (spec'd task)

## Foundation
- ✅ Full Java→Rust 1:1 port, tagged `v0.1.0` (byte-identical worldgen vs JVM at the tag)
- ✅ Commit everything in logical conventional commits; .gitignore; version tag
- ✅ Remove JavaRandom (xoshiro256++, deterministic per seed)
- ✅ Codebase cleanup: Settings-as-widgets anti-pattern, =debug keymap hack, dead code
- ✅ Bug/stability passes (wall dmg, invisible fences, gravestone state, soak tests…)
- ✅ Better default controls (E inventory, Space attack, sane text entry)
- ✅ Easier content addition (docs/ADDING_CONTENT.md + registries + artgen)
- ✅ DX overhaul: README, docs/, justfile ("fun to build in")
- ✅ Document EVERYTHING (TERRAIN/ENTITIES/ITEMS_AND_CRAFTING/RENDERING_AND_UI/
     CORE_AND_SAVES + ROADMAP; sonnet agents)

## World
- ✅ Infinite chunked worlds (dig-descent between layers; no pre-placed stairs)
- ✅ Large Minecraft-scale biomes + more natural biomes (10 incl. DeepOcean)
- ✅ Patchy biome edges (domain-warped transitions; snow freckles, sand fades)
- 🔄 Per-biome ground tint (lighting agent, code written, verifying)
- ✅ Multi-level terrain: dig-down pits→chasm→ladder; deep water + raft
- 📋 Rock elevation upward (mountain tiers, mirror of dig-down) — in mining task
- ✅ Structures: ruins, cemeteries (aging graves→zombies), standing stones, camps,
     old trails, destroyed villages, boulders
- ✅ Mud tile + mud pits + inland ponds; ocean waves; marsh mud rims
- ✅ Tall grass: paddock-core blocking only; meadows; slow growth (days/stage)
- ✅ Tree species per biome (pine/dead/willow/palm/flat-crown; snow pine existing)
- ✅ Dried dead bushes (desert/savanna)
- ✅ Seaweed/coral shallows; reeds; snow-capped mountains
- ✅ Forests denser + more common (user report fixed)
- ✅ Natural food: berries (regrow), mushrooms (surface+mines), apples, cactus fruit,
     coconuts; pumpkins drop items
- ✅ Jack-o-lanterns: craftable (pumpkin+torch) + spawn in cemeteries/villages
- ✅ Rare world events: Hollow Night, Aurora, Ember Rain, Whisper Fog, The Caravan
- ✅ Seasonal events: Halloween & Christmas windows (real calendar, mockable)
- ✅ New worlds spawn at seed-random time of day
- ✅ Day-cycle option: Classic / Long / Realtime (24h)
- ✅ Survival-only (Creative = --debug tool); no world size/type/theme options
- ✅ Sandbox pivot: sky world, Air Wizard, win condition, Score mode all removed

## Game systems
- ✅ Crafting overhaul: gather chains (fiber/stick/cord/knapping → crude tools)
- ✅ Weapons: spear (+throw), crossbow (part assembly), throwing knives, slingshot
- ✅ More recipes: cooking chain, bandages, fruit medley, rafts, jack-o-lantern
- ✅ Mob roster: creeper/slime/skeleton/wizard REMOVED (incl. their art cells +
     bossdeath.wav); snake kept & fixed; 4 originals (lurker, hound pack, golem, wisp)
- 📋 Mob-life wave (task #2): snake family (harmless/venomous/rattler coil/cave serpent),
     ghost (graves at night, stamina drain, Hollow Night swarms), grass stealth rendering
     + glowing night eyes, per-mob movement personalities
- 📋 Mining "fossicking" overhaul (task #1): prospector's pan, vein-chasing sparkles,
     rock hardness character, cave-ins + timber props, + upward rock tiers
- ✅ QOL: durability bar, death screen (time/score, Respawn/Main Menu), inventory
     info strip, readable notifications, Load World fixed (was completely broken),
     R-save crash fixed
- 📋 Save toast small in bottom-right corner (renderer frees when lighting lands)

## Presentation
- ✅ Hybrid RGB pipeline → fresh original art + generator (`artgen`)
- ✅ Player art: pixel-traced from the original (after 3 redesign attempts failed)
- 🔄 Classic mob art (cow/pig/etc): pixel-trace directive issued (art agent)
- 🔄 Terrain textures (grass/sand/dirt/stone/mud/snow): full pattern-language redesign
     in flight ("still looks like minicraft" fix)
- ✅ Full sheet-art logo: FOSSICKERS + DOOM wordmarks (verified on title)
- 🔄 Item icon audit; flora readability (mushroom/coral/seaweed legibility);
     growth-stage clarity; footprint redraw; grave shape variety incl. wooden crosses;
     connected tree-cluster canopies per species; asset consolidation (one sheet,
     delete icons/icons_ale/logo.png) — all queued in the running art agent
- ✅ Lighting system: sunsets, real cave darkness, dithered warm torchlight, aurora
     curtains, ~96µs/frame, always-on (no Classic toggle per user)
- ✅ Title: drone-flyover world (shared splash/title), smooth pan, gradient text bands
- ✅ Splash modernized; dead menu entries removed
- ✅ Smoked-glass menus; slate frames; hints only on title
- ✅ World creation: name+seed only, floats over flyover, seed hint
- ✅ Title flow: Continue (most recent world) / New World / Load World flat menu
- ✅ Instructions/book pages readable (opaque paper under glass system)
- ✅ Options screen matches UI style (floats, no wear-suit row)
- ✅ Snow/sand footprints color-matched (snow prints blue-gray)
- ✅ Tile edge blending (rock halo, grass dirt-rim fixes; corner nubs with art agent)

## Meta
- ✅ Sub-agents used aggressively with correct sequencing/file ownership
- ✅ Creative roadmap committed (docs/ROADMAP.md): mystical layer (leylines, shrines),
     bountiful layer (hives, fishing schools) — future waves
