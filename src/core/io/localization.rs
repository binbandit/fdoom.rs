//! Port of `fdoom.core.io.Localization`. Language files (.mcpl) are embedded.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};

use crate::assets;

pub struct Localization {
    /// Interior mutability so `get_localized` can be called with `&self` from render code
    /// (Java's method was static; the set only backs a debug log).
    known_unlocalized_strings: RefCell<HashSet<String>>,
    localization: HashMap<String, String>,
    selected_language: String,
    loaded_languages: Vec<String>,
    /// Mirror of `Game.debug` (Java read the static directly).
    pub debug: Cell<bool>,
}

impl Default for Localization {
    fn default() -> Self {
        Self::new()
    }
}

impl Localization {
    pub fn new() -> Localization {
        let mut loc = Localization {
            known_unlocalized_strings: RefCell::new(HashSet::new()),
            localization: HashMap::new(),
            selected_language: "english".to_string(),
            loaded_languages: assets::LOCALIZATIONS.iter().map(|(name, _)| name.to_string()).collect(),
            debug: Cell::new(true),
        };
        loc.load_selected_language_file();
        loc
    }

    /// Java `getLocalized(string)`.
    pub fn get_localized(&self, string: &str) -> String {
        if string.chars().all(|c| c == ' ') {
            return string.to_string(); // blank, or just whitespace
        }
        if string.parse::<f64>().is_ok() {
            return string.to_string(); // this is a number; don't try to localize it
        }

        let local_string = self.localization.get(string);

        if self.debug.get() && local_string.is_none() {
            let mut known = self.known_unlocalized_strings.borrow_mut();
            if !known.contains(string) {
                println!("The string \"{string}\" is not localized, returning itself instead.");
                known.insert(string.to_string());
            }
        }

        local_string.cloned().unwrap_or_else(|| string.to_string())
    }

    pub fn get_selected_language(&self) -> &str {
        &self.selected_language
    }

    pub fn change_language(&mut self, new_language: &str) {
        self.selected_language = new_language.to_string();
        self.load_selected_language_file();
    }

    fn load_selected_language_file(&mut self) {
        let file_text = assets::LOCALIZATIONS
            .iter()
            .find(|(name, _)| *name == self.selected_language)
            .map(|(_, text)| *text)
            .unwrap_or("");

        // JAVA: entries accumulate across language switches (the map is never cleared).
        let mut current_key = String::new();
        for line in file_text.lines() {
            // # at the start of a line means the line is a comment
            if line.starts_with('#') {
                continue;
            }
            if line.chars().all(|c| c == ' ') {
                continue;
            }
            if current_key.is_empty() {
                current_key = line.to_string();
            } else {
                self.localization.insert(std::mem::take(&mut current_key), line.to_string());
            }
        }
    }

    pub fn get_languages(&self) -> Vec<String> {
        self.loaded_languages.clone()
    }
}
