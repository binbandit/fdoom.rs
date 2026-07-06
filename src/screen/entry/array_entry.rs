//! Port of `fdoom.screen.entry.ArrayEntry` (and its `BooleanEntry`/`RangeEntry`
//! subclasses, which become constructor flavors — only `BooleanEntry` changed behavior,
//! by rendering its value as On/Off).

use crate::core::game::Game;
use crate::core::io::localization::Localization;
use crate::core::io::sound::Sound;
use crate::gfx::font;

use super::{EntryFlags, ListEntry};

/// Java's generic `T` — settings values are strings, ints, or booleans.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Int(i32),
    Bool(bool),
}

impl Value {
    /// Java `Object.toString()`.
    pub fn to_display(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            Value::Int(i) => i.to_string(),
            Value::Bool(b) => b.to_string(),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Value::Str(s) => s,
            _ => panic!("setting value is not a string: {self:?}"),
        }
    }

    pub fn as_int(&self) -> i32 {
        match self {
            Value::Int(i) => *i,
            _ => panic!("setting value is not an int: {self:?}"),
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            _ => panic!("setting value is not a bool: {self:?}"),
        }
    }

    /// Java `ArrayEntry.getIndex` equality: strings compare case-insensitively.
    fn matches(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Str(a), Value::Str(b)) => a.eq_ignore_ascii_case(b),
            (a, b) => a == b,
        }
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::Str(s.to_string())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::Str(s)
    }
}
impl From<i32> for Value {
    fn from(i: i32) -> Value {
        Value::Int(i)
    }
}
impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Bool(b)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Flavor {
    Normal,
    /// Java `BooleanEntry` — displays On/Off.
    Boolean,
}

pub type ChangeListener = Box<dyn Fn(&Value)>;

pub struct ArrayEntry {
    label: String,
    options: Vec<Value>,
    option_vis: Vec<bool>,
    selection: i32,
    wrap: bool,
    localize: bool,
    max_width: i32,
    change_action: Option<ChangeListener>,
    flavor: Flavor,
    flags: EntryFlags,
}

impl ArrayEntry {
    /// Java `new ArrayEntry(label, options...)`.
    pub fn new(label: &str, options: Vec<Value>, loc: &Localization) -> ArrayEntry {
        Self::with_flags(label, true, true, options, loc)
    }

    /// Java `new ArrayEntry(label, wrap, localize, options...)`.
    pub fn with_flags(label: &str, wrap: bool, localize: bool, options: Vec<Value>, loc: &Localization) -> ArrayEntry {
        let mut max_width = 0;
        for option in &options {
            let text = if localize { loc.get_localized(&option.to_display()) } else { option.to_display() };
            max_width = max_width.max(font::text_width(&text));
        }
        let option_vis = vec![true; options.len()];
        ArrayEntry {
            label: label.to_string(),
            options,
            option_vis,
            selection: 0,
            wrap,
            localize,
            max_width,
            change_action: None,
            flavor: Flavor::Normal,
            flags: EntryFlags::default(),
        }
    }

    /// Java `new BooleanEntry(label, initial)`.
    pub fn boolean(label: &str, initial: bool, loc: &Localization) -> ArrayEntry {
        let mut e = Self::with_flags(label, true, true, vec![Value::Bool(true), Value::Bool(false)], loc);
        e.flavor = Flavor::Boolean;
        e.set_selection(if initial { 0 } else { 1 });
        e
    }

    /// Java `new RangeEntry(label, min, max, initial)`.
    pub fn range(label: &str, min: i32, max: i32, initial: i32, loc: &Localization) -> ArrayEntry {
        let options: Vec<Value> = (min..=max).map(Value::Int).collect();
        let mut e = Self::with_flags(label, false, true, options, loc);
        e.set_value(&Value::Int(initial));
        e
    }

    pub fn get_label(&self) -> &str {
        &self.label
    }

    pub fn set_selection(&mut self, idx: i32) {
        let diff = idx != self.selection;
        if idx >= 0 && (idx as usize) < self.options.len() {
            self.selection = idx;
            if diff {
                if let Some(action) = &self.change_action {
                    action(&self.options[self.selection as usize]);
                }
            }
        }
    }

    pub fn set_value(&mut self, value: &Value) {
        if let Some(idx) = self.get_index(value) {
            self.set_selection(idx);
        }
    }

    pub fn get_selection(&self) -> i32 {
        self.selection
    }

    pub fn get_value(&self) -> &Value {
        &self.options[self.selection as usize]
    }

    pub fn value_is(&self, value: &Value) -> bool {
        self.get_value().matches(value)
    }

    fn get_index(&self, value: &Value) -> Option<i32> {
        self.options.iter().position(|o| o.matches(value)).map(|i| i as i32)
    }

    pub fn set_value_visibility(&mut self, value: &Value, visible: bool) {
        if let Some(idx) = self.get_index(value) {
            self.option_vis[idx as usize] = visible;
            if idx == self.selection && !visible {
                self.move_selection(1);
            }
        }
    }

    pub fn get_value_visibility(&self, value: &Value) -> bool {
        match self.get_index(value) {
            Some(idx) => self.option_vis[idx as usize],
            None => false,
        }
    }

    fn move_selection(&mut self, dir: i32) {
        // stuff for changing the selection, including skipping locked entries
        let prev_sel = self.selection;
        let mut selection = self.selection;
        let len = self.options.len() as i32;
        loop {
            selection += dir;
            if self.wrap {
                selection %= len;
                if selection < 0 {
                    selection = len - 1;
                }
            } else {
                selection = selection.min(len - 1).max(0);
            }
            if self.option_vis[selection as usize] || selection == prev_sel {
                break;
            }
        }
        self.set_selection(selection);
    }

    /// Java `setChangeAction(listener)` — immediately fires with the current value.
    pub fn set_change_action(&mut self, l: ChangeListener) {
        l(self.get_value());
        self.change_action = Some(l);
    }
}

impl ListEntry for ArrayEntry {
    fn flags(&self) -> EntryFlags {
        self.flags
    }

    fn flags_mut(&mut self) -> &mut EntryFlags {
        &mut self.flags
    }

    fn tick(&mut self, g: &mut Game) {
        let prev_sel = self.selection;
        let mut selection = self.selection;

        if g.input.get_key("left").clicked {
            selection -= 1;
        }
        if g.input.get_key("right").clicked {
            selection += 1;
        }

        if prev_sel != selection {
            g.play_sound(Sound::Select);
            self.move_selection(selection - prev_sel);
        }
    }

    fn to_display_string(&self, g: &Game) -> String {
        match self.flavor {
            Flavor::Normal => {
                let mut str = format!("{}: ", g.localization.get_localized(&self.label));
                let option = self.options[self.selection as usize].to_display();
                str.push_str(&if self.localize { g.localization.get_localized(&option) } else { option });
                str
            }
            // JAVA: BooleanEntry does not localize its label
            Flavor::Boolean => format!(
                "{}: {}",
                self.label,
                g.localization.get_localized(if self.get_value().as_bool() { "On" } else { "Off" })
            ),
        }
    }

    fn get_width(&self, g: &Game) -> i32 {
        font::text_width(&format!("{}: ", g.localization.get_localized(&self.label))) + self.max_width
    }

    fn is_array_entry(&self) -> bool {
        true
    }
}
