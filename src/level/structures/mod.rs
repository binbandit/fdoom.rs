//! Deterministic surface structures for infinite worlds: ruins, cemeteries, standing
//! stones, abandoned camps, and the towns — little hamlets and full villages — plus
//! the connective tissue between them: worn trails, and boulder scatter in the open
//! biomes. Towns additionally roll an AGE ([`town_age`]): Overgrown (walls down,
//! floors reclaimed by flora, lamps out, time-capsule loot), the classic Weathered
//! look, or Settled (sound walls, tended garden, every lamp lit — just nobody home).
//!
//! Placement follows the same hash-grid pattern as `infinite_gen::gate_in_cell`: each
//! structure type gets its own coarse cell grid, and each cell holds at most one
//! structure at a jittered, biome-gated position — a pure function of
//! `(world seed, structure kind, cell)`. Each kind also rolls a layout variant from
//! the placement hash ([`variant_of`]): ruins come as square rooms, L-shaped two-room
//! builds, or round towers; cemeteries are fenced, overgrown, or stone-walled; standing
//! stones form rings, straight avenues, or dolmen clusters; camps pitch a lean-to or go
//! cold (fire ring + bedroll); hamlets come as a crossroads, a ring around a green,
//! or a straggle along a lane; villages center on a round plaza or a crossroads.
//! Chunks stamp every structure whose footprint
//! could overlap them (rect query padded by [`MAX_RADIUS`]), so a structure straddling a
//! chunk border comes out identical from both sides.
//!
//! Three stamping passes run per chunk, all pure, in a fixed order so overlaps resolve
//! identically everywhere:
//!
//! 1. **Boulders** ([`boulder_at`]): sparse per-tile hash scatter of 1x1/2x2 rock
//!    outcrops in Plains/Savanna/Tundra. Breakable like any rock tile.
//! 2. **Trails** ([`trails_in_rect`], [`trail_writes`]): each trail-worthy structure
//!    (ruins/cemetery/camp) links to its nearest neighbor within [`TRAIL_RANGE`] tiles
//!    with a winding worn-dirt path — hash-jittered waypoint chains with occasional
//!    worn-away gaps and a torch stump where the trail meets the site. Trails only
//!    replace soft ground (grass/sand/snow/trees/...), never water or rock, so they
//!    fade out at fords and outcrops like real old routes.
//! 3. **Structures** ([`structure_writes`]): the blueprints proper, stamped last so
//!    their footprints always win. The towns come last in [`ALL_KINDS`] so a rare
//!    single-structure overlap resolves in the town's favor (villages over hamlets).
//!
//! Tiles are stamped during `infinite_gen::generate_chunk` (before the gate set-pieces,
//! so a rare overlap always leaves the gate intact). Loot chests, scavenge containers
//! (crates/barrels/cupboards — one-time searchable, [`container_positions`]), ember
//! campfires and house lanterns are entities and can't live in the pure tile pass;
//! they are spawned by [`spawn_chunk_entities`] when `level::ensure_chunks_at`
//! generates a chunk *fresh* (not loaded from disk), and the chunk is marked dirty so
//! it persists and the entities never duplicate.

mod loot;
mod placement;
mod ruins_camps;
mod towns;

pub use loot::*;
pub use placement::*;
pub use ruins_camps::*;
pub use towns::*;
