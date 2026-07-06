//! Port of `fdoom.screen.BookData` — the text of the in-game books.
//!
//! Java loaded the two long books from `/resources/*.txt` at class-init; here they're
//! embedded via `crate::assets` and processed on demand.

/// Java `BookData.about`.
pub const ABOUT: &str = "Modded by David.b and +Dillyg10+ until 1.8, then taken over by Chris J. Our goal is to expand Minicraft to be more fun and continuous.\nMinicraft was originally made by Markus Perrson for ludum dare 22 competition.";

/// Java `BookData.instructions`.
pub const INSTRUCTIONS: &str = "With the default controls...\n\nMove your character with arrow keys or WSAD. Press C to attack and X to open the inventory, and to use items. Pickup furniture and torches with V. Select an item in the inventory to equip it.\n\nThe Goal: Defeat the air wizard!";

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
