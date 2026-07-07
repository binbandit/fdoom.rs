//! Window tile (light & shelter wave, no Java counterpart): a wall segment with a
//! glass pane. Solid to movement exactly like a wall, but `blocks_light` stays
//! `false`, so the occlusion-aware emitter stamping in `gfx::lighting` shines
//! straight through — a torch-lit room spills a beam onto the ground outside.
//!
//! Crafted at the workbench (Glass*2 + Wood*2), placed on the wall-floor family
//! (Wood Planks / Stone Bricks / Obsidian). Glass shatters at a single hit: the
//! pane is gone, the wooden frame's planks remain, and half the time a Glass pane
//! survives to pick back up.
//!
//! TODO(art): dedicated window cells (frame, pane, mullion cross). Until the art
//! agent lands them, this reuses the stone-wall connector cells for the frame and
//! inlays the pane as a pale solid-fill cell with a darkened mullion cross.

use super::{ConnectorSprite, TileDef, TileKind};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::{Direction, Entity};
use crate::gfx::{Screen, Sprite, color};

pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Window);
    // The stone wall's connector cells in a paler mortar palette: adjacent walls and
    // windows merge into one run of masonry, and a lone window still reads as a
    // wall-thick segment. blocks_light deliberately stays false.
    def.csprite = Some(ConnectorSprite::new(
        Sprite::new(4, 25, 3, 3, color::get4(111, 344, 455, 455), 3),
        Sprite::new(7, 24, 2, 2, color::get(111, 455), 3),
        Sprite::blank(2, 2, 455),
    ));
    def
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    // The wall-segment frame first (connector pieces vs. neighboring walls/windows).
    super::dispatch::default_render(g, screen, def, lvl, x, y);

    // The pane: a pale sky-glass fill inset into the segment, with a darkened
    // mullion cross and a top shadow line so it reads as leaded glass, not a hole.
    let (px, py) = (x << 4, y << 4);
    Sprite::blank(1, 1, 445).render(screen, px + 4, py + 4);
    screen.darken_rect(px + 4, py + 4, 8, 1, 70); // frame shadow along the top
    screen.darken_rect(px + 4, py + 7, 8, 1, 55); // horizontal mullion
    screen.darken_rect(px + 7, py + 4, 1, 8, 55); // vertical mullion
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, _e: &Entity) -> bool {
    false
}

/// Windows connect into wall runs (and other windows) so a window mid-wall renders
/// as a continuous segment; walls reciprocate in `wall::connects_to`.
pub fn connects_to(_def: &TileDef, other: &TileDef, _is_side: bool) -> bool {
    matches!(other.kind, TileKind::Wall { .. } | TileKind::Window)
}

/// Glass shatters at a tap — any hit breaks the pane. The wooden frame's planks
/// remain (the window's own floor family), and the Glass drops half the time.
#[allow(clippy::too_many_arguments)]
pub fn hurt_by(
    g: &mut Game,
    _def: &TileDef,
    lvl: usize,
    x: i32,
    y: i32,
    _source: &mut Entity,
    _dmg: i32,
    _attack_dir: Direction,
) -> bool {
    if g.random.next_int_bound(2) == 0 {
        let glass = crate::item::registry::get(g, "glass");
        crate::level::drop_item(g, lvl, x * 16 + 8, y * 16 + 8, glass);
    }
    let planks = g.tiles.get("Wood Planks");
    g.set_tile_default(lvl, x, y, &planks);
    g.play_sound(Sound::MonsterHurt);
    let smash = crate::entity::particle::new_smash_particle(x * 16, y * 16);
    g.level_mut(lvl).add(smash, lvl);
    true
}
