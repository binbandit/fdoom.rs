//! Port of `fdoom.core.io.InputHandler`.
//!
//! Key handling works exactly like Java: a `keymap` maps action names ("ATTACK") to
//! physical key expressions ("C|SPACE|ENTER", "SHIFT-Q"), and `keyboard` holds the actual
//! per-physical-key press state machines. The platform layer feeds `key_toggled` /
//! `key_typed` from window events (Java: `KeyListener`), using Java `KeyEvent.VK_*` names
//! ("A", "UP", "SHIFT", "EQUALS", ...).

use std::collections::HashMap;

/// Snapshot of a key's state as returned by `get_key` (Java returned the live `Key`
/// object, but callers only read these two flags).
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyState {
    pub down: bool,
    pub clicked: bool,
}

/// Java `InputHandler.Key` — the press-processing state machine.
#[derive(Debug, Default, Clone)]
struct Key {
    presses: i32,
    absorbs: i32,
    down: bool,
    clicked: bool,
    sticky: bool,
    /// Set for modifier keys (SHIFT/CTRL/ALT) but never read; kept so the intent
    /// ("this key should not auto-release") stays visible at the construction sites.
    #[allow(dead_code)]
    stay_down: bool,
}

impl Key {
    fn new(stay_down: bool) -> Key {
        Key {
            stay_down,
            ..Key::default()
        }
    }

    fn toggle(&mut self, pressed: bool) {
        self.down = pressed;
        if pressed && !self.sticky {
            self.presses += 1;
        }
    }

    fn tick(&mut self) {
        if self.absorbs < self.presses {
            self.absorbs += 1;
            if self.presses - self.absorbs > 3 {
                self.absorbs = self.presses - 3;
            }
            self.clicked = true;
        } else {
            if !self.sticky {
                self.sticky = self.presses > 3;
            } else {
                self.sticky = self.down;
            }
            self.clicked = self.sticky;
            self.presses = 0;
            self.absorbs = 0;
        }
    }

    fn release(&mut self) {
        self.down = false;
        self.clicked = false;
        self.presses = 0;
        self.absorbs = 0;
        self.sticky = false;
    }
}

/// One action binding: an action name mapped to a "KEY|OTHER-KEY" expression.
#[derive(Debug, Clone)]
struct Mapping {
    action: String,
    keys: String,
    /// Debug-build-only cheats (formerly the Java "=debug" name-suffix hack).
    debug_only: bool,
}

pub struct InputHandler {
    /// Action bindings, in display order for the key-binding screen.
    keymap: Vec<Mapping>,
    /// The actual map of physical key names to Key state (auto-generated on demand).
    keyboard: HashMap<String, Key>,
    last_key_typed: String,
    key_typed_buffer: String,

    /// Set when listening for a new key binding (Java `keyToChange`).
    pub key_to_change: Option<String>,
    key_changed: Option<String>,
    overwrite: bool,

    /// Mirror of `Game.debug` (Java read the static directly inside `getKey`).
    pub debug: bool,
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl InputHandler {
    pub fn new() -> InputHandler {
        let mut input = InputHandler {
            keymap: Vec::new(),
            keyboard: HashMap::new(),
            last_key_typed: String::new(),
            key_typed_buffer: String::new(),
            key_to_change: None,
            key_changed: None,
            overwrite: false,
            debug: true, // stays true until the CLI args are parsed at startup
        };
        input.init_key_map();
        input.keyboard.insert("SHIFT".into(), Key::new(true));
        input.keyboard.insert("CTRL".into(), Key::new(true));
        input.keyboard.insert("ALT".into(), Key::new(true));
        input
    }

    fn init_key_map(&mut self) {
        // modern defaults (v0.1.0 had Java's: ATTACK=C|SPACE|ENTER, MENU=X|E,
        // PICKUP=V|P clashing with POTIONEFFECTS=P, NIGHT always bound)
        let defaults: &[(&str, &str, bool)] = &[
            ("UP", "UP|W", false),
            ("DOWN", "DOWN|S", false),
            ("LEFT", "LEFT|A", false),
            ("RIGHT", "RIGHT|D", false),
            ("SELECT", "ENTER", false),
            ("EXIT", "ESCAPE", false),
            ("ATTACK", "SPACE|C", false),
            ("MENU", "X", false),
            ("INVENTORY", "E|I", false),
            ("CRAFT", "Z|SHIFT-E", false),
            ("PICKUP", "V", false),
            ("DROP-ONE", "Q", false),
            ("DROP-STACK", "SHIFT-Q", false),
            ("SAVE", "R", false),
            ("PAUSE", "ESCAPE", false),
            ("MAP", "M", false),
            ("NIGHT", "N", true),
            ("SURVIVAL", "SHIFT-S|SHIFT-1", true),
            ("CREATIVE", "SHIFT-C|SHIFT-2", true),
            ("POTIONEFFECTS", "P", false),
            ("INFO", "SHIFT-I", false),
        ];
        self.keymap = defaults
            .iter()
            .map(|(action, keys, debug_only)| Mapping {
                action: (*action).to_string(),
                keys: (*keys).to_string(),
                debug_only: *debug_only,
            })
            .collect();
    }

    pub fn reset_key_bindings(&mut self) {
        self.init_key_map();
    }

    fn keymap_get(&self, action: &str) -> Option<&Mapping> {
        self.keymap.iter().find(|m| m.action == action)
    }

    fn keymap_put(&mut self, action: &str, keys: String) {
        if let Some(m) = self.keymap.iter_mut().find(|m| m.action == action) {
            m.keys = keys;
        } else {
            self.keymap.push(Mapping {
                action: action.to_string(),
                keys,
                debug_only: false,
            });
        }
    }

    /// Java `getChangedKey()` — consumes and returns "ACTION;binding".
    pub fn get_changed_key(&mut self) -> String {
        let key = self.key_changed.take();
        match key {
            Some(k) => {
                let keys = self
                    .keymap_get(&k)
                    .map(|m| m.keys.clone())
                    .unwrap_or_default();
                format!("{k};{keys}")
            }
            None => "null;".to_string(),
        }
    }

    /// Java `tick()` — processes each key's press state. Called once per game tick.
    pub fn tick(&mut self) {
        self.last_key_typed = std::mem::take(&mut self.key_typed_buffer);
        for key in self.keyboard.values_mut() {
            key.tick();
        }
    }

    /// Java `releaseAll()` — used when the game window loses focus.
    pub fn release_all(&mut self) {
        for key in self.keyboard.values_mut() {
            key.release();
        }
    }

    /// Java `setKey(keymapKey, keyboardKey)` — for changing default bindings.
    pub fn set_key(&mut self, keymap_key: &str, keyboard_key: &str, debug: bool) {
        // tolerate the old on-disk "ACTION=debug" names from pre-refactor prefs files
        let action = keymap_key.split("=debug").next().unwrap_or(keymap_key);
        if let Some(m) = self.keymap_get(action) {
            if !m.debug_only || debug {
                self.keymap_put(action, keyboard_key.to_string());
            }
        }
    }

    /// Java `getMapping(actionKey)` — the mapped physical keys for display purposes.
    pub fn get_mapping(&self, action_key: &str) -> String {
        let action_key = action_key.to_uppercase();
        match self.keymap_get(&action_key) {
            Some(m) => m.keys.replace('|', "/"),
            None => "NO_KEY".to_string(),
        }
    }

    /// Java `getKey(keytext)` — THE way to query keys, by action or physical name.
    pub fn get_key(&mut self, keytext: &str) -> KeyState {
        let debug = self.debug;
        self.get_key_impl(keytext, true, debug)
    }

    fn get_key_impl(&mut self, keytext: &str, get_from_map: bool, debug: bool) -> KeyState {
        if keytext.is_empty() {
            return KeyState::default();
        }

        let mut keytext = keytext.to_uppercase();

        if get_from_map {
            if let Some(m) = self.keymap_get(&keytext) {
                if m.debug_only && !debug {
                    return KeyState::default(); // debug-only cheat outside --debug
                }
                keytext = m.keys.clone();
            }
        }

        let full_keytext = keytext.clone();

        if keytext.contains('|') {
            // multiple key possibilities exist for this action; combine each with "or"
            let mut key = KeyState::default();
            for keyposs in keytext.split('|') {
                let a_key = self.get_key_impl(keyposs, false, debug);
                key.down = key.down || a_key.down;
                key.clicked = key.clicked || a_key.clicked;
            }
            return key;
        }

        // truncate compound keys to only the base key, no modifiers
        let base_key = match keytext.rfind('-') {
            Some(idx) => keytext[idx + 1..].to_string(),
            None => keytext.clone(),
        };

        let key = self.keyboard.entry(base_key).or_default();
        let mut key = KeyState {
            down: key.down,
            clicked: key.clicked,
        };

        let keytext = full_keytext;

        if keytext == "SHIFT" || keytext == "CTRL" || keytext == "ALT" {
            return key; // nothing more must be done with modifier keys
        }

        let mut found_s = false;
        let mut found_c = false;
        let mut found_a = false;
        if keytext.contains('-') {
            for keyname in keytext.split('-') {
                match keyname {
                    "SHIFT" => found_s = true,
                    "CTRL" => found_c = true,
                    "ALT" => found_a = true,
                    _ => {}
                }
            }
        }
        let mod_match = self.get_key_impl("shift", true, debug).down == found_s
            && self.get_key_impl("ctrl", true, debug).down == found_c
            && self.get_key_impl("alt", true, debug).down == found_a;

        if keytext.contains('-') {
            // compound key: reflect the trigger key only when the modifiers match
            key = KeyState {
                down: mod_match && key.down,
                clicked: mod_match && key.clicked,
            };
        } else if !mod_match {
            key = KeyState::default();
        }

        key
    }

    /// Query a physical key directly, bypassing the action keymap (used by text-entry
    /// rows so typed letters never double as navigation).
    pub fn get_physical_key(&mut self, keytext: &str) -> KeyState {
        let debug = self.debug;
        self.get_key_impl(keytext, false, debug)
    }

    /// Java `pressKey(keyname, pressed)` — press physical keys programmatically.
    pub fn press_key(&mut self, keyname: &str, pressed: bool) {
        let keyname = keyname.to_uppercase();
        if let Some(key) = self.keyboard.get_mut(&keyname) {
            key.toggle(pressed);
        }
    }

    pub fn get_all_pressed_keys(&self) -> Vec<String> {
        self.keyboard
            .iter()
            .filter(|(_, k)| k.down)
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Platform event entry point (Java `keyPressed`/`keyReleased` via `toggle`).
    /// `keytext` must be a Java-style key name ("A", "UP", "SHIFT", "EQUALS", ...).
    pub fn key_toggled(&mut self, keytext: &str, pressed: bool) {
        let keytext = keytext.to_uppercase();

        if pressed && self.key_to_change.is_some() && !is_mod(&keytext) {
            let to_change = self.key_to_change.take().unwrap();
            let new_binding = format!(
                "{}{}{}",
                if self.overwrite {
                    String::new()
                } else {
                    format!(
                        "{}|",
                        self.keymap_get(&to_change)
                            .map(|m| m.keys.as_str())
                            .unwrap_or("")
                    )
                },
                self.get_cur_modifiers(),
                keytext
            );
            self.keymap_put(&to_change, new_binding);
            self.key_changed = Some(to_change);
            return;
        }
        if let Some(key) = self.keyboard.get_mut(&keytext) {
            key.toggle(pressed);
        } else {
            // Keys are normally created lazily by get_key queries, but events can arrive
            // for keys nothing has queried yet — register those on first press so the
            // press isn't lost.
            let mut key = Key::default();
            key.toggle(pressed);
            self.keyboard.insert(keytext, key);
        }
    }

    /// Platform event entry point (Java `keyTyped`).
    pub fn key_typed(&mut self, ch: char) {
        // append: several chars can arrive within one game tick (fast typing, scripted
        // demo runs) — overwriting dropped all but the last
        self.key_typed_buffer.push(ch);
    }

    fn get_cur_modifiers(&mut self) -> String {
        format!(
            "{}{}{}",
            if self.get_key("ctrl").down {
                "CTRL-"
            } else {
                ""
            },
            if self.get_key("alt").down { "ALT-" } else { "" },
            if self.get_key("shift").down {
                "SHIFT-"
            } else {
                ""
            }
        )
    }

    /// Java `getKeyPrefs()` — used by Save to store key preferences.
    pub fn get_key_prefs(&self, debug: bool) -> Vec<String> {
        self.keymap
            .iter()
            .filter(|m| !m.debug_only || debug)
            .map(|m| format!("{};{}", m.action, m.keys))
            .collect()
    }

    pub fn change_key_binding(&mut self, action_key: &str) {
        self.key_to_change = Some(action_key.to_uppercase());
        self.overwrite = true;
    }

    pub fn add_key_binding(&mut self, action_key: &str) {
        self.key_to_change = Some(action_key.to_uppercase());
        self.overwrite = false;
    }

    /// Java `addKeyTyped(typing, pattern)` — accumulate typed text (world names etc.).
    /// `pattern` restricts allowed characters (Java regex; here a char predicate).
    pub fn add_key_typed(&mut self, typing: &str, pattern: Option<fn(char) -> bool>) -> String {
        let mut typing = typing.to_string();
        if !self.last_key_typed.is_empty() {
            let letter = std::mem::take(&mut self.last_key_typed);
            for ch in letter.chars() {
                // Java: \p{Print} — printable characters only
                if !ch.is_control() && pattern.map(|p| p(ch)).unwrap_or(true) {
                    typing.push(ch);
                }
            }
        }
        if self.get_key("backspace").clicked && !typing.is_empty() {
            typing.pop();
        }
        typing
    }
}

fn is_mod(keyname: &str) -> bool {
    matches!(keyname, "SHIFT" | "CTRL" | "ALT")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_is_consumed_by_tick() {
        let mut input = InputHandler::new();
        input.key_toggled("C", true);
        input.tick();
        assert!(input.get_key("attack").clicked);
        assert!(input.get_key("attack").down);
        input.tick();
        assert!(
            !input.get_key("attack").clicked,
            "click should only last one tick"
        );
        assert!(input.get_key("attack").down, "key is still held");
        input.key_toggled("C", false);
        input.tick();
        assert!(!input.get_key("attack").down);
    }

    #[test]
    fn held_key_becomes_sticky() {
        let mut input = InputHandler::new();
        // press more than 3 times without ticking, then hold
        for _ in 0..5 {
            input.key_toggled("X", true);
        }
        for _ in 0..6 {
            input.tick();
        }
        // sticky: clicked stays true while held
        assert!(input.get_key("menu").clicked);
    }

    #[test]
    fn compound_keys_need_modifiers() {
        let mut input = InputHandler::new();
        input.key_toggled("Q", true);
        input.tick();
        assert!(input.get_key("drop-one").clicked);
        assert!(!input.get_key("drop-stack").clicked);

        let mut input = InputHandler::new();
        input.key_toggled("SHIFT", true);
        input.key_toggled("Q", true);
        input.tick();
        assert!(
            !input.get_key("drop-one").clicked,
            "shift-q must not trigger plain q action"
        );
        assert!(input.get_key("drop-stack").clicked);
    }

    #[test]
    fn multi_mapping_or_combines() {
        let mut input = InputHandler::new();
        input.key_toggled("SPACE", true);
        input.tick();
        assert!(input.get_key("attack").down);
    }

    #[test]
    fn typing_accumulates_and_backspaces() {
        let mut input = InputHandler::new();
        input.key_typed('a');
        input.tick();
        let typed = input.add_key_typed("nam", None);
        assert_eq!(typed, "nama");
        input.key_toggled("BACKSPACE", true);
        input.tick();
        let typed = input.add_key_typed(&typed, None);
        assert_eq!(typed, "nam");
    }
}
