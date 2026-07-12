//! Behavior of `fdoom.entity.furniture.Crafter` — and THE BENCH's module logic.

use crate::core::game::Game;
use crate::entity::furniture::crafter::{CrafterType, Module};
use crate::entity::{Entity, EntityKind};
use crate::item::Recipe;

/// Using a station opens the survival screen on CRAFT with the station's recipe
/// set and its name as a sub-header (UI_REDESIGN §3.5) — the other tabs stay
/// live, so the pack is reachable without stepping away from the bench.
///
/// THE BENCH additionally fits a held module on use (consume it, bolt it on) and
/// assembles its recipe list from the saw (built-in workbench list), the module
/// recipes themselves (the visible grind path — never loot-locked), and every
/// fitted module's absorbed family.
pub fn use_furniture(g: &mut Game, e: &mut Entity, player: &mut Entity) -> bool {
    let EntityKind::Crafter(c) = &e.kind else {
        return false;
    };
    let crafter_type = c.crafter_type;

    if crafter_type == CrafterType::Bench {
        // hold-to-fit: using the bench with a module in hand bolts it on
        let held_module = player
            .player()
            .active_item
            .as_ref()
            .and_then(|i| Module::from_item_name(i.get_name()));
        if let Some(m) = held_module {
            let EntityKind::Crafter(c) = &mut e.kind else {
                return false;
            };
            if c.modules.contains(&m) {
                g.push_ambient(&format!("The bench already has a {}.", m.item_name()));
            } else {
                c.modules.push(m);
                let pd = player.player_mut();
                // consume one from the held stack
                match pd.active_item.as_mut().and_then(|i| i.count_mut()) {
                    Some(count) if *count > 1 => *count -= 1,
                    _ => pd.active_item = None,
                }
                g.push_ambient(&format!("The {} bolts onto the bench.", m.item_name()));
            }
        }
        let EntityKind::Crafter(c) = &e.kind else {
            return false;
        };
        let fitted = fitted_mask(&c.modules);
        g.set_menu(crate::screen::survival_display::SurvivalDisplay::at_bench(
            g,
            player,
            bench_recipes(g, &c.modules),
            fitted,
        ));
        return true;
    }

    let recipes = match crafter_type {
        CrafterType::Workbench => g.recipes.workbench.clone(),
        CrafterType::Oven => g.recipes.oven.clone(),
        CrafterType::Furnace => g.recipes.furnace.clone(),
        CrafterType::Anvil => g.recipes.anvil.clone(),
        CrafterType::Enchanter => g.recipes.enchant.clone(),
        CrafterType::Loom => g.recipes.loom.clone(),
        CrafterType::Bench => unreachable!("handled above"),
    };
    g.set_menu(
        crate::screen::survival_display::SurvivalDisplay::at_station(
            g,
            player,
            crafter_type.name(),
            recipes,
        ),
    );
    true
}

/// The bench's live recipe list: saw (workbench) + the module recipes + fitted
/// families, in that order — the module recipes sit right under the basics so the
/// grind path is always on screen (affordability dimming grays them until viable).
pub fn bench_recipes(g: &Game, modules: &[Module]) -> Vec<Recipe> {
    let mut out = g.recipes.workbench.clone();
    out.extend(g.recipes.bench_modules.clone());
    for m in modules {
        let family = match m {
            Module::Vice => &g.recipes.anvil,
            Module::Spindle => &g.recipes.loom,
            Module::AssayKit => &g.recipes.enchant,
        };
        out.extend(family.clone());
    }
    out
}

/// The rack state in `Module::VALUES` order (vice, spindle, assay kit).
pub fn fitted_mask(modules: &[Module]) -> [bool; 3] {
    let mut mask = [false; 3];
    for (i, m) in Module::VALUES.iter().enumerate() {
        mask[i] = modules.contains(m);
    }
    mask
}
