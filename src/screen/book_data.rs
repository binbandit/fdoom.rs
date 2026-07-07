//! Port of `fdoom.screen.BookData` — the text of the in-game books.
//!
//! Java loaded the two long books from `/resources/*.txt` at class-init; here they're
//! embedded via `crate::assets` and processed on demand.

/// Java `BookData.about`.
pub const ABOUT: &str = "Modded by David.b and +Dillyg10+ until 1.8, then taken over by Chris J. Our goal is to expand Minicraft to be more fun and continuous.\nMinicraft was originally made by Markus Perrson for ludum dare 22 competition.";

/// Java `BookData.instructions`.
pub const INSTRUCTIONS: &str = "Move with WASD or the arrow keys. SPACE attacks and uses your held item; E opens your inventory; Z crafts by hand.\n\nGather fibers from tall grass, sticks from trees, and stones — twist cord, knap a sharp stone, and lash together your first crude tools.\n\nShovel down through the dirt until you hit rock, then break through with a pickaxe to reach the mines below. Leave a ladder trail home.\n\nThe world is endless and it is yours: explore its biomes, loot its ruins, brave its nights, and build whatever you like.";

/// Java `BookData.antVenomBook`.
pub fn ant_venom_book() -> String {
    load_book(crate::assets::ANTIDOUS_TXT)
}

/// Java `BookData.storylineGuide`.
pub fn storyline_guide() -> String {
    load_book(crate::assets::STORY_GUIDE_TXT)
}

/// Java `BookData.loadBook(title)` — joins the file's lines with "\n" and turns the
/// literal `\0` escapes into real NUL page breaks.
fn load_book(text: &str) -> String {
    let book = text.lines().collect::<Vec<&str>>().join("\n");
    book.replace("\\0", "\0")
}
