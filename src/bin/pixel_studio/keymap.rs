//! Every keyboard binding, in one place — the tool's manual is
//! docs/DEV_GUIDE.md#every-key and this module is what it documents.
//!
//! Two modes capture text before anything else gets a look in: the `N` new-sprite
//! modal and the `/` finder. Otherwise a key is offered to each group in turn and the
//! first one that claims it wins, so a key belongs to exactly one group and the
//! guarded variants of a key (`Ctrl+C` before `C`, `Shift+R` before `R`) sit next to
//! each other where the precedence is visible.
//!
//! Only a claimed key triggers a redraw; unbound keys fall through silently.

use winit::keyboard::KeyCode;

use crate::app::App;
use crate::atlas::CELL;
use crate::color::PREVIEW_PALS;
use crate::studio::{Paint, SIZE_PRESETS, Source, Tool};

/// Text entry for the finder and the new-sprite modal (file-name characters only).
pub(crate) fn key_char(code: KeyCode, shift: bool) -> Option<char> {
    use KeyCode::*;
    let c = match code {
        KeyA => 'a',
        KeyB => 'b',
        KeyC => 'c',
        KeyD => 'd',
        KeyE => 'e',
        KeyF => 'f',
        KeyG => 'g',
        KeyH => 'h',
        KeyI => 'i',
        KeyJ => 'j',
        KeyK => 'k',
        KeyL => 'l',
        KeyM => 'm',
        KeyN => 'n',
        KeyO => 'o',
        KeyP => 'p',
        KeyQ => 'q',
        KeyR => 'r',
        KeyS => 's',
        KeyT => 't',
        KeyU => 'u',
        KeyV => 'v',
        KeyW => 'w',
        KeyX => 'x',
        KeyY => 'y',
        KeyZ => 'z',
        Digit0 => '0',
        Digit1 => '1',
        Digit2 => '2',
        Digit3 => '3',
        Digit4 => '4',
        Digit5 => '5',
        Digit6 => '6',
        Digit7 => '7',
        Digit8 => '8',
        Digit9 => '9',
        Minus => {
            if shift {
                '_'
            } else {
                '-'
            }
        }
        Slash => '/',
        _ => return None,
    };
    Some(c)
}

impl App {
    pub(crate) fn on_key(&mut self, code: KeyCode) {
        if self.st.new_sprite.is_some() {
            self.on_key_new_sprite(code);
            self.refresh();
            return;
        }
        if self.st.find.is_some() {
            self.on_key_find(code);
            self.refresh();
            return;
        }
        let shift = self.mods.shift_key();
        let ctrl = self.mods.control_key() || self.mods.super_key();
        let handled = self.key_arrows(code, shift)
            || self.key_window(code)
            || self.key_paint(code, shift, ctrl)
            || self.key_history(code, shift, ctrl)
            || self.key_document(code)
            || self.key_view(code, shift);
        if handled {
            self.refresh();
        }
    }

    /// Arrows and `I`/`K`. The arrows are overloaded three ways, most specific first:
    /// the RGB stepper while the custom swatch is active, Shift to wrap-nudge the
    /// image, and plain arrows to navigate.
    fn key_arrows(&mut self, code: KeyCode, shift: bool) -> bool {
        let st = &mut self.st;
        let custom = st.cur == Paint::Custom;
        match code {
            KeyCode::ArrowLeft if custom => st.chan = (st.chan + 2) % 3,
            KeyCode::ArrowRight if custom => st.chan = (st.chan + 1) % 3,
            KeyCode::ArrowUp if custom => {
                let step = if shift { 1 } else { 8 };
                st.custom[st.chan] = st.custom[st.chan].saturating_add(step);
            }
            KeyCode::ArrowDown if custom => {
                let step = if shift { 1 } else { 8 };
                st.custom[st.chan] = st.custom[st.chan].saturating_sub(step);
            }
            KeyCode::ArrowLeft if shift => st.nudge(-1, 0),
            KeyCode::ArrowRight if shift => st.nudge(1, 0),
            KeyCode::ArrowUp if shift => st.nudge(0, -1),
            KeyCode::ArrowDown if shift => st.nudge(0, 1),
            // plain arrows: files in dir mode, window cells in the sheet views
            KeyCode::ArrowUp => match st.source {
                Source::Tree { .. } => st.move_file_sel(-1),
                _ => st.move_block(0, -1),
            },
            KeyCode::ArrowDown => match st.source {
                Source::Tree { .. } => st.move_file_sel(1),
                _ => st.move_block(0, 1),
            },
            KeyCode::ArrowLeft => st.move_block(-1, 0),
            KeyCode::ArrowRight => st.move_block(1, 0),
            // vertical window stepping inside tall images (dir-mode strips)
            KeyCode::KeyI => st.move_block(0, -1),
            KeyCode::KeyK => st.move_block(0, 1),
            _ => return false,
        }
        true
    }

    /// Sizing and snapping the edit window.
    fn key_window(&mut self, code: KeyCode) -> bool {
        let st = &mut self.st;
        match code {
            KeyCode::Tab => {
                let s = if st.view_w == 16 && st.view_h == 16 {
                    8
                } else {
                    16
                };
                st.set_view(s, s);
            }
            KeyCode::KeyG => st.snap_to_sprite(),
            _ => return false,
        }
        true
    }

    /// Brushes, tools and the clipboard.
    fn key_paint(&mut self, code: KeyCode, shift: bool, ctrl: bool) -> bool {
        let st = &mut self.st;
        let custom = st.cur == Paint::Custom;
        match code {
            KeyCode::KeyE => st.cur = Paint::Erase,
            KeyCode::KeyC if ctrl => st.copy_block(),
            KeyCode::KeyV if ctrl => {
                if st.clipboard.is_some() {
                    st.paste_armed = !st.paste_armed;
                    st.status = if st.paste_armed {
                        "PASTE: CLICK THE CANVAS TO PLACE".into()
                    } else {
                        String::new()
                    };
                } else {
                    st.status = "PASTE: NOTHING COPIED YET (CTRL+C)".into();
                }
            }
            KeyCode::KeyC => {
                if custom {
                    st.cur = st.prev_paint;
                } else {
                    st.prev_paint = st.cur;
                    st.cur = Paint::Custom;
                }
            }
            KeyCode::KeyF => {
                if let Some((px, py)) = st.hover {
                    st.flood_fill(px, py);
                } else {
                    st.status = "FILL: HOVER A CANVAS PIXEL FIRST".into();
                }
            }
            KeyCode::KeyL => {
                st.tool = if st.tool == Tool::Line {
                    Tool::Pencil
                } else {
                    Tool::Line
                };
            }
            KeyCode::KeyR if shift => {
                st.tool = if st.tool == Tool::RectFill {
                    Tool::Pencil
                } else {
                    Tool::RectFill
                };
            }
            KeyCode::KeyR => {
                st.tool = if st.tool == Tool::Rect {
                    Tool::Pencil
                } else {
                    Tool::Rect
                };
            }
            KeyCode::KeyM => st.mirror = !st.mirror,
            KeyCode::BracketLeft => st.shade_shift(false),
            KeyCode::BracketRight => st.shade_shift(true),
            KeyCode::KeyH => st.flip(true),
            KeyCode::KeyV => st.flip(false),
            _ => return false,
        }
        true
    }

    /// Undo and redo. `Y` alone redoes as well as `Ctrl+Y`, so a hand on the left of
    /// the keyboard never needs a modifier.
    fn key_history(&mut self, code: KeyCode, shift: bool, ctrl: bool) -> bool {
        let st = &mut self.st;
        match code {
            KeyCode::KeyZ if ctrl && shift => st.redo_pop(),
            KeyCode::KeyU => st.undo_pop(),
            KeyCode::KeyZ if ctrl => st.undo_pop(),
            KeyCode::KeyY => st.redo_pop(),
            _ => return false,
        }
        true
    }

    /// Saving, reverting, switching view, and creating a sprite.
    fn key_document(&mut self, code: KeyCode) -> bool {
        let st = &mut self.st;
        match code {
            KeyCode::KeyS => st.save(), // plain S and Ctrl+S both save
            KeyCode::KeyX => st.revert(),
            KeyCode::KeyW => st.toggle_canvas(),
            KeyCode::KeyN => st.open_new_sprite(),
            _ => return false,
        }
        true
    }

    /// Preview controls and the finder: nothing here changes a pixel.
    fn key_view(&mut self, code: KeyCode, shift: bool) -> bool {
        let st = &mut self.st;
        match code {
            KeyCode::KeyD if shift => {
                st.backdrop_idx = (st.backdrop_idx + st.backdrops.len() - 1) % st.backdrops.len();
            }
            KeyCode::KeyD => st.backdrop_idx = (st.backdrop_idx + 1) % st.backdrops.len(),
            KeyCode::Slash if !shift => {
                if let Source::Tree { .. } = st.source {
                    st.find = Some(String::new());
                    st.status = "FIND: TYPE PART OF A FILE NAME (ESC CANCELS)".into();
                } else {
                    st.status = "FIND: FILE LIST ONLY (PRESS W FOR FILES)".into();
                }
            }
            KeyCode::KeyP if shift => {
                st.pal_idx = (st.pal_idx + PREVIEW_PALS.len() - 1) % PREVIEW_PALS.len();
            }
            KeyCode::KeyP => st.pal_idx = (st.pal_idx + 1) % PREVIEW_PALS.len(),
            KeyCode::KeyA => st.toggle_anim(),
            KeyCode::KeyB => st.capture_onion(),
            KeyCode::KeyO => st.toggle_onion(),
            KeyCode::Slash if shift => st.help_on = !st.help_on,
            _ => return false,
        }
        true
    }

    /// Key handling while the new-sprite modal is open (captures all text keys).
    fn on_key_new_sprite(&mut self, code: KeyCode) {
        let shift = self.mods.shift_key();
        if code == KeyCode::Enter {
            self.st.create_new_sprite();
            return;
        }
        let Some(ns) = &mut self.st.new_sprite else {
            return;
        };
        match code {
            KeyCode::Backspace => {
                ns.name.pop();
            }
            KeyCode::Tab => ns.pal = !ns.pal,
            KeyCode::ArrowRight if shift => {
                ns.w = (ns.w + CELL).min(256);
                ns.preset = usize::MAX;
            }
            KeyCode::ArrowLeft if shift => {
                ns.w = (ns.w - CELL).max(CELL);
                ns.preset = usize::MAX;
            }
            KeyCode::ArrowDown if shift => {
                ns.h = (ns.h + CELL).min(256);
                ns.preset = usize::MAX;
            }
            KeyCode::ArrowUp if shift => {
                ns.h = (ns.h - CELL).max(CELL);
                ns.preset = usize::MAX;
            }
            KeyCode::ArrowUp | KeyCode::ArrowDown => {
                let n = SIZE_PRESETS.len();
                let cur = if ns.preset >= n { 0 } else { ns.preset };
                ns.preset = if code == KeyCode::ArrowDown {
                    (cur + 1) % n
                } else if ns.preset >= n {
                    0
                } else {
                    (cur + n - 1) % n
                };
                (ns.w, ns.h) = (SIZE_PRESETS[ns.preset].0, SIZE_PRESETS[ns.preset].1);
            }
            _ => {
                if let Some(c) = key_char(code, shift) {
                    ns.name.push(c);
                }
            }
        }
    }

    /// Key handling while the `/` finder is active (captures all text keys).
    fn on_key_find(&mut self, code: KeyCode) {
        let shift = self.mods.shift_key();
        match code {
            KeyCode::Enter => {
                self.st.find = None;
                self.st.status.clear();
            }
            KeyCode::Backspace => {
                if let Some(f) = &mut self.st.find {
                    f.pop();
                }
                self.st.find_apply(1, false);
            }
            KeyCode::ArrowDown => self.st.find_apply(1, true),
            KeyCode::ArrowUp => self.st.find_apply(-1, true),
            _ => {
                if let Some(c) = key_char(code, shift) {
                    if let Some(f) = &mut self.st.find {
                        f.push(c);
                    }
                    self.st.find_apply(1, false);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Text entry accepts exactly the characters a sprite path may contain, so the
    /// finder and the new-sprite modal can never type an illegal name.
    #[test]
    fn text_entry_covers_legal_name_characters() {
        assert_eq!(key_char(KeyCode::KeyA, false), Some('a'));
        assert_eq!(key_char(KeyCode::KeyA, true), Some('a'), "always lowercase");
        assert_eq!(key_char(KeyCode::Digit7, false), Some('7'));
        assert_eq!(key_char(KeyCode::Slash, false), Some('/'));
        assert_eq!(key_char(KeyCode::Minus, false), Some('-'));
        assert_eq!(key_char(KeyCode::Minus, true), Some('_'), "shift+- is _");

        // every letter and digit maps to itself
        let letters = [KeyCode::KeyB, KeyCode::KeyM, KeyCode::KeyQ, KeyCode::KeyZ];
        for (code, want) in letters.iter().zip("bmqz".chars()) {
            assert_eq!(key_char(*code, false), Some(want));
        }
    }

    /// Keys that are not name characters are refused rather than typed, so Escape,
    /// Enter and the arrows keep working inside a text field.
    #[test]
    fn text_entry_refuses_control_keys() {
        for code in [
            KeyCode::Escape,
            KeyCode::Enter,
            KeyCode::Tab,
            KeyCode::Backspace,
            KeyCode::ArrowUp,
            KeyCode::Space,
            KeyCode::Period,
        ] {
            assert_eq!(key_char(code, false), None, "{code:?} should not type");
        }
    }

    /// Every character the keyboard can produce is legal in a sprite name — the
    /// modal cannot build a name that `--new` would then reject.
    #[test]
    fn everything_typable_is_a_legal_name_character() {
        use crate::library::is_legal_sprite_name;
        let all = [
            KeyCode::KeyA,
            KeyCode::KeyZ,
            KeyCode::Digit0,
            KeyCode::Digit9,
            KeyCode::Minus,
            KeyCode::Slash,
        ];
        for code in all {
            for shift in [false, true] {
                let c = key_char(code, shift).unwrap();
                assert!(
                    is_legal_sprite_name(&c.to_string()),
                    "{c:?} types but is not a legal name character"
                );
            }
        }
    }
}
