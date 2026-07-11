//! Port of the `fdoom.gfx` package: the software renderer.
//!
//! Note: `ConnectorSprite`'s neighbor-aware render logic lives in `crate::level::tile`
//! (its only caller) because it needs tile-registry types; the sprite-building half
//! (`make_sprite`) is here in `sprite.rs`.

pub mod ambience;
pub mod biome_palette;
pub mod color;
pub mod dimension;
pub mod ellipsis;
pub mod font;
pub mod font_style;
pub mod insets;
pub mod lighting;
pub mod point;
pub mod rectangle;
pub mod screen;
pub mod sprite;
pub mod sprite_sheet;

pub use dimension::Dimension;
pub use font_style::FontStyle;
pub use insets::Insets;
pub use point::Point;
pub use rectangle::Rectangle;
pub use screen::Screen;
pub use sprite::{MobAnims, Sprite};
pub use sprite_sheet::SpriteSheet;
