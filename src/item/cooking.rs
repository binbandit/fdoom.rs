//! The cooking design table: which foods are raw flesh (queasy risk), what a
//! campfire roasts them into, and which composed dishes count as a hot meal.
//!
//! Kept as name-keyed tables (matching the registry's name-keyed prototypes) so the
//! whole farming/cooking food design reads in one place. Heal-value tiers, for
//! reference (values live on the registry `food()` one-liners):
//! foraged/raw = 1..2, cooked single = 3..4, composed hot dish = 7..8 + the
//! [`is_hearty`] warm-meal bonus.

/// What a lit campfire (or the oven/furnace recipes) turns this raw food into.
/// The campfire's field-cooking path and the station recipe lists both follow this
/// one table — if it maps here, it cooks.
pub fn cooked_result(raw_name: &str) -> Option<&'static str> {
    let table: [(&str, &str); 11] = [
        ("Raw Pork", "Cooked Pork"),
        ("Raw Beef", "Steak"),
        ("Venison", "Cooked Venison"),
        ("Raw Fish", "Cooked Fish"),
        ("Big Fish", "Cooked Big Fish"),
        ("Cave Eel", "Cooked Cave Eel"),
        ("Mushroom", "Cooked Mushroom"),
        ("Potato", "Baked Potato"),
        ("Corn", "Roast Corn"),
        ("Pumpkin", "Roast Pumpkin"),
        ("Mushroom Skewer", "Roasted Skewer"),
    ];
    table
        .iter()
        .find(|(raw, _)| raw.eq_ignore_ascii_case(raw_name))
        .map(|(_, cooked)| *cooked)
}

/// Raw flesh (and raw potato — solanine is real) carries a chance of a brief
/// Queasy spell when eaten. Cooking always clears the risk.
pub fn queasy_risk(name: &str) -> bool {
    [
        "Raw Pork", "Raw Beef", "Venison", "Raw Fish", "Big Fish", "Cave Eel", "Potato",
    ]
    .iter()
    .any(|n| n.eq_ignore_ascii_case(name))
}

/// Composed hot dishes: eating one refills stamina and grants a short Regen —
/// the payoff for cooking with a pot instead of poking meat at a fire.
pub fn is_hearty(name: &str) -> bool {
    ["Hearty Stew", "Fish Chowder"]
        .iter()
        .any(|n| n.eq_ignore_ascii_case(name))
}
