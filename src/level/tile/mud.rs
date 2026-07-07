//! Mud (sandbox era, no Java counterpart): wet ground that rings inland ponds and forms
//! pits in marshes. Walkable but boggy — entities wade at reduced speed, like shallow
//! quicksand without the sinking. Shovels dig it like dirt (yields dirt).

use super::{TileDef, TileKind, tool_use};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::{Direction, Entity};
use crate::gfx::{Screen, Sprite, color};
use crate::item::{Item, ToolType};
use crate::level::drop_item;

pub fn make(name: &str) -> TileDef {
    let mut def = TileDef::new(name, TileKind::Mud);
    def.connects_to_water = true;
    def
}

pub fn render(_g: &mut Game, screen: &mut Screen, _lvl: usize, x: i32, y: i32) {
    // dedicated wet-mud block (artgen `mud_cells`, cells 24..25,1..2):
    // 0 = puddle hollows, 1 = mud base, 2 = drier clod ridges, 3 = sheen glints
    Sprite::new(24, 1, 2, 2, color::get4(100, 210, 321, 433), 0).render(screen, x * 16, y * 16);
}

pub fn interact(
    g: &mut Game,
    lvl: usize,
    xt: i32,
    yt: i32,
    player: &mut Entity,
    item: &mut Item,
    _attack_dir: Direction,
) -> bool {
    // fossicking: mud is always pannable - creek beds are where the colors settle
    if super::fossick::try_pan(g, lvl, xt, yt, player, item) {
        return true;
    }
    if tool_use(g, player, item, ToolType::Shovel, 4).is_some() {
        let pit = g.tiles.get("Dug Pit");
        g.set_tile_default(lvl, xt, yt, &pit);
        let dirt = crate::item::registry::get(g, "dirt");
        drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, dirt);
        g.play_sound(Sound::MonsterHurt);
        return true;
    }
    false
}
