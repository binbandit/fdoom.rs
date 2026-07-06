//! Bridges the plain `Settings` store to menu widgets: option screens build their own
//! `ArrayEntry` rows from the settings schema and sync edited values back every tick.

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::game::Game;
use crate::core::io::settings::Settings;
use crate::screen::entry::array_entry::{ArrayEntry, Value};

pub type SettingEntry = (String, Rc<RefCell<ArrayEntry>>);

/// Build a menu row for a setting, pre-selected to its current value.
pub fn make_entry(g: &Game, key: &str) -> SettingEntry {
    let options = g.settings.options_of(key);
    let label = Settings::label_of(key);
    let entry = if matches!(options.first(), Some(Value::Bool(_))) {
        ArrayEntry::boolean(label, g.settings.get(key).as_bool(), &g.localization)
    } else {
        // fps doesn't wrap (a 10..300 range); language options aren't localized
        let wrap = key != "fps";
        let localize = key != "language";
        let mut e = ArrayEntry::with_flags(label, wrap, localize, options, &g.localization);
        e.set_value(&g.settings.get(key));
        e
    };
    (key.to_string(), Rc::new(RefCell::new(entry)))
}

/// Write the widget's current value back into the settings store.
pub fn sync(g: &mut Game, entries: &[SettingEntry]) {
    for (key, entry) in entries {
        let value = entry.borrow().get_value().clone();
        g.settings.set(key, value);
    }
}
