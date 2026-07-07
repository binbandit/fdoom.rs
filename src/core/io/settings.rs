//! Game settings — a plain typed key/value store with a declared schema.
//!
//! (Until v0.1.x this stored shared `Rc<RefCell<ArrayEntry>>` menu widgets, mirroring
//! Java's `Settings`; the option screens now own their widgets and sync values through
//! this store instead.)

use std::collections::HashMap;

use crate::screen::entry::array_entry::Value;

/// Every setting: (key, display label). Options and defaults live in `options_of` /
/// `default_of` below — one place to touch when adding a setting.
pub const KEYS: [(&str, &str); 13] = [
    ("fps", "Max FPS"),
    ("diff", "Difficulty"),
    ("mode", "Game Mode"),
    ("daycycle", "Day Cycle"),
    ("sound", "Sound"),
    ("autosave", "Autosave"),
    ("size", "World Size"),
    ("theme", "World Theme"),
    ("type", "Terrain Type"),
    ("worldtype", "World"),
    ("unlockedskin", "Wear Suit"),
    ("skinon", "Wear Suit"),
    ("language", "Language"),
];

fn sv(s: &str) -> Value {
    Value::Str(s.to_string())
}

pub struct Settings {
    values: HashMap<String, Value>,
    /// Languages present in the assets (fixed at startup).
    languages: Vec<String>,
}

impl Settings {
    pub fn new(languages: Vec<String>) -> Settings {
        let mut s = Settings {
            values: HashMap::new(),
            languages,
        };
        for (key, _) in KEYS {
            let default = s.default_of(key);
            s.values.insert(key.to_string(), default);
        }
        s
    }

    /// The selectable options for a setting (single source of truth for the UI).
    pub fn options_of(&self, option: &str) -> Vec<Value> {
        match option.to_lowercase().as_str() {
            "fps" => (10..=300).map(Value::Int).collect(),
            "diff" => vec![sv("Easy"), sv("Normal"), sv("Hard")],
            // Survival is the only real mode; Creative remains for the --debug cheat toggle
            "mode" => vec![sv("Survival"), sv("Creative")],
            // in-game day pacing: Classic ~18min, Long ~72min, Realtime = 24 real hours
            "daycycle" => vec![sv("Classic"), sv("Long"), sv("Realtime")],
            "sound" | "autosave" | "unlockedskin" | "skinon" => {
                vec![Value::Bool(true), Value::Bool(false)]
            }
            "size" => [128, 256, 512].map(Value::Int).to_vec(),
            "theme" => vec![
                sv("Normal"),
                sv("Forest"),
                sv("Desert"),
                sv("Plain"),
                sv("Hell"),
            ],
            "type" => vec![sv("Island"), sv("Box"), sv("Mountain"), sv("Irregular")],
            "worldtype" => vec![sv("Infinite"), sv("Classic")],
            "language" => self.languages.iter().map(|l| sv(l)).collect(),
            _ => Vec::new(),
        }
    }

    fn default_of(&self, option: &str) -> Value {
        match option.to_lowercase().as_str() {
            "fps" => Value::Int(60),
            "diff" => sv("Normal"),
            "mode" => sv("Survival"),
            "daycycle" => sv("Classic"),
            "sound" | "autosave" | "unlockedskin" => Value::Bool(true),
            "skinon" => Value::Bool(false),
            "size" => Value::Int(128),
            "theme" => sv("Normal"),
            "type" => sv("Island"),
            "worldtype" => sv("Infinite"),
            "language" => sv("english"),
            _ => Value::Int(0),
        }
    }

    /// The display label for a setting.
    pub fn label_of(option: &str) -> &'static str {
        let option = option.to_lowercase();
        KEYS.iter()
            .find(|(k, _)| *k == option)
            .map(|(_, l)| *l)
            .unwrap_or("")
    }

    /// Current value.
    pub fn get(&self, option: &str) -> Value {
        self.values[&option.to_lowercase()].clone()
    }

    /// Index of the current value within `options_of` (difficulty scaling etc.).
    pub fn get_idx(&self, option: &str) -> i32 {
        let current = self.get(option);
        self.options_of(option)
            .iter()
            .position(|v| v.matches(&current))
            .map(|i| i as i32)
            .unwrap_or(0)
    }

    /// Set a value; ignored unless it is one of the setting's options (same contract as
    /// the old entry-based setter).
    pub fn set(&mut self, option: &str, value: impl Into<Value>) {
        let value = value.into();
        let key = option.to_lowercase();
        if let Some(v) = self
            .options_of(&key)
            .into_iter()
            .find(|v| v.matches(&value))
        {
            self.values.insert(key, v);
        }
    }

    /// Set by option index.
    pub fn set_idx(&mut self, option: &str, idx: i32) {
        let options = self.options_of(option);
        if idx >= 0 && (idx as usize) < options.len() {
            self.values
                .insert(option.to_lowercase(), options[idx as usize].clone());
        }
    }
}
