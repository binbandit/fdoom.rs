//! Port of `fdoom.core.io.Settings`.
//!
//! The options are menu-entry objects shared between this registry and the options/world-gen
//! screens, exactly as in Java — hence `Rc<RefCell<ArrayEntry>>` handles.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::screen::entry::array_entry::{ArrayEntry, Value};
use crate::screen::entry::ListEntry;

use super::localization::Localization;

pub struct Settings {
    options: HashMap<String, Rc<RefCell<ArrayEntry>>>,
}

impl Settings {
    pub fn new(loc: &Localization) -> Settings {
        let mut options: HashMap<String, Rc<RefCell<ArrayEntry>>> = HashMap::new();
        let mut put = |name: &str, entry: ArrayEntry| {
            options.insert(name.to_string(), Rc::new(RefCell::new(entry)));
        };

        put("fps", ArrayEntry::range("Max FPS", 10, 300, 60, loc));

        let diff = ArrayEntry::new("Difficulty", vec!["Easy".into(), "Normal".into(), "Hard".into()], loc);
        put("diff", diff);

        put(
            "mode",
            ArrayEntry::new(
                "Game Mode",
                vec!["Survival".into(), "Creative".into(), "Hardcore".into(), "Score".into()],
                loc,
            ),
        );

        put(
            "scoretime",
            ArrayEntry::new(
                "Time (Score Mode)",
                vec![10.into(), 20.into(), 40.into(), 60.into(), 120.into()],
                loc,
            ),
        );

        put("sound", ArrayEntry::boolean("Sound", true, loc));
        put("autosave", ArrayEntry::boolean("Autosave", true, loc));

        put("size", ArrayEntry::new("World Size", vec![128.into(), 256.into(), 512.into()], loc));
        put(
            "theme",
            ArrayEntry::new(
                "World Theme",
                vec!["Normal".into(), "Forest".into(), "Desert".into(), "Plain".into(), "Hell".into()],
                loc,
            ),
        );
        put(
            "type",
            ArrayEntry::new(
                "Terrain Type",
                vec!["Island".into(), "Box".into(), "Mountain".into(), "Irregular".into()],
                loc,
            ),
        );

        put("unlockedskin", ArrayEntry::boolean("Wear Suit", false, loc));
        put("skinon", ArrayEntry::boolean("Wear Suit", false, loc));

        let language = ArrayEntry::with_flags(
            "Language",
            true,
            false,
            loc.get_languages().into_iter().map(Value::Str).collect(),
            loc,
        );
        put("language", language);

        let settings = Settings { options };

        settings.get_entry("diff").borrow_mut().set_selection(1);
        settings.get_entry("scoretime").borrow_mut().set_value_visibility(&Value::Int(10), false);
        settings.get_entry("scoretime").borrow_mut().set_value_visibility(&Value::Int(120), false);
        settings
            .get_entry("language")
            .borrow_mut()
            .set_value(&Value::Str(loc.get_selected_language().to_string()));

        // Java change actions (closures over the shared entries):
        {
            let scoretime = settings.get_entry("scoretime");
            settings.get_entry("mode").borrow_mut().set_change_action(Box::new(move |value| {
                scoretime.borrow_mut().set_visible(matches!(value, Value::Str(s) if s == "Score"));
            }));
        }
        {
            let skinon = settings.get_entry("skinon");
            settings.get_entry("unlockedskin").borrow_mut().set_change_action(Box::new(move |value| {
                skinon.borrow_mut().set_visible(value.as_bool());
            }));
        }

        settings
    }

    /// Java `Settings.get(option)` — the current value.
    pub fn get(&self, option: &str) -> Value {
        self.options[&option.to_lowercase()].borrow().get_value().clone()
    }

    /// Java `Settings.getIdx(option)` — index of the current value.
    pub fn get_idx(&self, option: &str) -> i32 {
        self.options[&option.to_lowercase()].borrow().get_selection()
    }

    /// Java `Settings.getEntry(option)`.
    pub fn get_entry(&self, option: &str) -> Rc<RefCell<ArrayEntry>> {
        self.options[&option.to_lowercase()].clone()
    }

    /// Java `Settings.set(option, value)`.
    pub fn set(&self, option: &str, value: impl Into<Value>) {
        self.options[&option.to_lowercase()].borrow_mut().set_value(&value.into());
    }

    /// Java `Settings.setIdx(option, idx)`.
    pub fn set_idx(&self, option: &str, idx: i32) {
        self.options[&option.to_lowercase()].borrow_mut().set_selection(idx);
    }
}
