//! Game settings — a plain typed key/value store with a declared schema.
//!
//! (Until v0.1.x this stored shared `Rc<RefCell<ArrayEntry>>` menu widgets, mirroring
//! Java's `Settings`; the option screens now own their widgets and sync values through
//! this store instead.)

use std::collections::HashMap;

use crate::screen::entry::array_entry::Value;

/// The score-mode time options hidden until unlocked in-game (persisted via Unlocks).
pub const LOCKED_SCORETIMES: [i32; 2] = [10, 120];

/// Every setting: (key, display label). Options and defaults live in `options_of` /
/// `default_of` below — one place to touch when adding a setting.
pub const KEYS: [(&str, &str); 13] = [
    ("fps", "Max FPS"),
    ("diff", "Difficulty"),
    ("mode", "Game Mode"),
    ("scoretime", "Time (Score Mode)"),
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
    /// Score-time options unlocked by finishing score mode (persisted in the Unlocks
    /// file; formerly hidden entry options in the Java design).
    pub unlocked_scoretimes: Vec<i32>,
    /// Languages present in the assets (fixed at startup).
    languages: Vec<String>,
}

impl Settings {
    pub fn new(languages: Vec<String>) -> Settings {
        let mut s = Settings {
            values: HashMap::new(),
            unlocked_scoretimes: Vec::new(),
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
            "mode" => vec![sv("Survival"), sv("Creative"), sv("Hardcore"), sv("Score")],
            "scoretime" => [10, 20, 40, 60, 120].map(Value::Int).to_vec(),
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
            "scoretime" => Value::Int(20),
            "sound" | "autosave" => Value::Bool(true),
            "unlockedskin" | "skinon" => Value::Bool(false),
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

    /// Whether a score-time option is selectable (locked ones need unlocking).
    pub fn scoretime_visible(&self, minutes: i32) -> bool {
        !LOCKED_SCORETIMES.contains(&minutes) || self.unlocked_scoretimes.contains(&minutes)
    }

    /// Unlock a score-time option (formerly `entry.setValueVisibility(v, true)`).
    pub fn unlock_scoretime(&mut self, minutes: i32) {
        if !self.unlocked_scoretimes.contains(&minutes) {
            self.unlocked_scoretimes.push(minutes);
        }
    }
}
