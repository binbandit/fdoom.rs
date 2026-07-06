//! Port of `fdoom.screen.entry.InputEntry` — a text-input row (world names, seeds).
//!
//! Java constrained input with a regex; the Rust `InputHandler::add_key_typed` takes a
//! char predicate instead. Java's anonymous-subclass overrides of `isValid()` /
//! `getUserInput()` (see `WorldGenDisplay.makeWorldNameInput` and the seed entry) become
//! the `Validation` enum.

use crate::core::game::Game;
use crate::gfx::{Screen, color, font};

use super::{COL_UNSLCT, EntryFlags, ListEntry};

/// The char class of Java `WorldGenDisplay.worldNameRegex` = `[a-zA-Z0-9 ]+`.
pub fn world_name_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == ' '
}

/// The char class of the seed entry's `[0-9]+`.
pub fn digit_char(ch: char) -> bool {
    ch.is_ascii_digit()
}

/// Java `isValid()` variants (base class + the two anonymous-subclass overrides).
pub enum Validation {
    /// Java default: `userInput.matches(regex)` — every char in the class, one or more.
    Pattern,
    /// The `WorldGenDisplay.worldSeed` override: always valid.
    Always,
    /// The `WorldGenDisplay.makeWorldNameInput` override: pattern-valid and not equal
    /// (ignoring case) to any taken name. Also lowercases `get_user_input` (Java's
    /// `getUserInput()` override).
    UniqueName(Vec<String>),
}

pub type InputChangeListener = Box<dyn FnMut(&str)>;

pub struct InputEntry {
    prompt: String,
    pattern: Option<fn(char) -> bool>,
    max_length: i32,
    user_input: String,
    validation: Validation,
    listener: Option<InputChangeListener>,
    flags: EntryFlags,
}

impl InputEntry {
    /// Java `new InputEntry(prompt, regex, maxLen)`.
    pub fn new(prompt: &str, pattern: Option<fn(char) -> bool>, max_len: i32) -> InputEntry {
        Self::with_init(prompt, pattern, max_len, "")
    }

    /// Java `new InputEntry(prompt, regex, maxLen, initValue)`.
    pub fn with_init(
        prompt: &str,
        pattern: Option<fn(char) -> bool>,
        max_len: i32,
        init_value: &str,
    ) -> InputEntry {
        InputEntry {
            prompt: prompt.to_string(),
            pattern,
            max_length: max_len,
            user_input: init_value.to_string(),
            validation: Validation::Pattern,
            listener: None,
            flags: EntryFlags::default(),
        }
    }

    pub fn set_validation(&mut self, validation: Validation) {
        self.validation = validation;
    }

    /// Java `getUserInput()` (lowercased for the world-name flavor, per its override).
    pub fn get_user_input(&self) -> String {
        match self.validation {
            Validation::UniqueName(_) => self.user_input.to_lowercase(),
            _ => self.user_input.clone(),
        }
    }

    /// Java `isValid()`.
    pub fn is_valid(&self) -> bool {
        match &self.validation {
            Validation::Always => true,
            Validation::Pattern => self.matches_pattern(),
            Validation::UniqueName(taken) => {
                if !self.matches_pattern() {
                    return false;
                }
                let name = self.get_user_input();
                !taken.iter().any(|other| other.eq_ignore_ascii_case(&name))
            }
        }
    }

    /// Java `userInput.matches(regex)` — the regexes used are all `[class]+` shaped.
    fn matches_pattern(&self) -> bool {
        !self.user_input.is_empty()
            && self
                .user_input
                .chars()
                .all(|ch| self.pattern.map(|p| p(ch)).unwrap_or(true))
    }

    /// Java `setChangeListener(l)`.
    pub fn set_change_listener(&mut self, l: InputChangeListener) {
        self.listener = Some(l);
    }
}

impl ListEntry for InputEntry {
    fn captures_typing(&self) -> bool {
        true
    }

    fn flags(&self) -> EntryFlags {
        self.flags
    }

    fn flags_mut(&mut self) -> &mut EntryFlags {
        &mut self.flags
    }

    fn tick(&mut self, g: &mut Game) {
        let prev = self.user_input.clone();
        self.user_input = g.input.add_key_typed(&self.user_input, self.pattern);
        if prev != self.user_input {
            if let Some(listener) = &mut self.listener {
                listener(&self.user_input);
            }
        }

        if self.max_length > 0 && self.user_input.chars().count() > self.max_length as usize {
            // truncates extra
            self.user_input = self
                .user_input
                .chars()
                .take(self.max_length as usize)
                .collect();
        }
    }

    fn to_display_string(&self, g: &Game) -> String {
        format!(
            "{}{}{}",
            g.localization.get_localized(&self.prompt),
            if self.prompt.is_empty() { "" } else { ": " },
            self.user_input
        )
    }

    fn render(&mut self, screen: &mut Screen, g: &mut Game, x: i32, y: i32, is_selected: bool) {
        let text = self.to_display_string(g);
        let col = if self.is_valid() {
            if is_selected {
                color::GREEN
            } else {
                COL_UNSLCT
            }
        } else {
            color::RED
        };
        font::draw(&text, screen, x, y, col);
    }
}
