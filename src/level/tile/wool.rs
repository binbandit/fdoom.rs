//! Port of `fdoom.level.tile.WoolTile` (including the `WoolColor` enum).

use super::{TileDef, TileKind};
use crate::core::game::Game;
use crate::core::io::sound::Sound;
use crate::entity::Direction;
use crate::entity::Entity;
use crate::gfx::{Screen, Sprite, color};
use crate::item::{Item, ItemKind, ToolType};

/// Java `WoolTile.WoolColor` — (name, col) in ordinal order.
const WOOL_COLORS: [(&str, i32); 6] = [
    ("NONE", color::get4(444, 333, 444, 555)),
    ("RED", color::get4(400, 500, 400, 500)),
    ("YELLOW", color::get4(550, 661, 440, 550)),
    ("GREEN", color::get4(30, 40, 40, 50)),
    ("BLUE", color::get4(15, 115, 15, 115)),
    ("BLACK", color::get4(111, 111, 0, 111)),
];

/// Java `WoolTile` constructor.
pub fn make() -> TileDef {
    let mut def = TileDef::new("Wool", TileKind::Wool);
    def.sprite = Some(Sprite::repeat(17, 0, 2, 2, 0));
    def
}

#[allow(clippy::too_many_arguments)]
pub fn render(g: &mut Game, screen: &mut Screen, def: &TileDef, lvl: usize, x: i32, y: i32) {
    let data = g.level(lvl).get_data(x, y);
    // FIX: out-of-range data (bad save data) falls back to uncolored wool instead of
    // panicking (Java threw ArrayIndexOutOfBoundsException).
    let color = WOOL_COLORS.get(data as usize).unwrap_or(&WOOL_COLORS[0]).1;
    if let Some(sprite) = &def.sprite {
        sprite.render_color(screen, x * 16, y * 16, color);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn interact(
    g: &mut Game,
    _def: &TileDef,
    lvl: usize,
    xt: i32,
    yt: i32,
    player: &mut Entity,
    item: &mut Item,
    _attack_dir: Direction,
) -> bool {
    if let ItemKind::Tool { ttype, level, .. } = item.kind {
        if ttype == ToolType::Shovel
            && crate::entity::mob::player_behavior::pay_stamina(player, 3 - level)
            && item.pay_durability(g.is_mode("creative"))
        {
            let hole = g.tiles.get("hole");
            g.set_tile_default(lvl, xt, yt, &hole);
            let wool = crate::item::registry::get(g, "Wool");
            crate::level::drop_item(g, lvl, xt * 16 + 8, yt * 16 + 8, wool);
            g.play_sound(Sound::MonsterHurt);
            return true;
        }
    }
    false
}

#[allow(clippy::too_many_arguments)]
pub fn may_pass(_g: &Game, _def: &TileDef, _lvl: usize, _x: i32, _y: i32, e: &Entity) -> bool {
    crate::entity::behavior::can_wool(e)
}

/// Java `WoolTile.getData(String)` — `Enum.valueOf(WoolColor.class, data.toUpperCase()).ordinal()`.
pub fn get_data_str(_def: &TileDef, data: &str) -> i32 {
    let data = data.to_uppercase();
    for (i, (name, _)) in WOOL_COLORS.iter().enumerate() {
        if *name == data {
            return i as i32;
        }
    }
    // JAVA: Enum.valueOf threw IllegalArgumentException on an unknown constant, crashing
    // on a corrupted save. FIX: log and fall back to uncolored wool (ordinal 0).
    println!("WoolTile.getData: unknown wool color {data:?}, defaulting to NONE");
    0
}

/// Java `WoolTile.getName(String)` — the wool color is treated as a data value; the Rust
/// hook receives it already parsed as the ordinal.
pub fn get_name(def: &TileDef, data: i32) -> String {
    // FIX: guard the index like `render` — bad data no longer panics.
    let color_name = WOOL_COLORS.get(data as usize).unwrap_or(&WOOL_COLORS[0]).0;
    format!("{} {}", color_name, def.name)
}

/// Java `WoolTile.matches(int thisData, String tileInfo)`.
pub fn matches(def: &TileDef, this_data: i32, tile_info: &str) -> bool {
    if !tile_info.contains('_') {
        def.name == tile_info
    } else {
        let parts: Vec<&str> = tile_info.split('_').collect();
        let tname = parts[0];
        let tdata: i32 = parts[1].parse().unwrap_or(0);
        def.name == tname && this_data == tdata
    }
}
