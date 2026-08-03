//! The two full-screen overlays: the `?` key list and the `N` new-sprite modal.
//!
//! Both are drawn last so they sit over everything, and both are pure chrome — the
//! new-sprite modal displays the pending [`NewSprite`](super::NewSprite) that the
//! keymap edits and `files::create_new_sprite` acts on.

use crate::layout::{GRID_MAJOR, PANEL, TXT, TXT_DIM, TXT_WARN, VIEW_H, VIEW_W};

use super::{SIZE_PRESETS, Studio};

/// Left column of the key list: everything that changes pixels.
const HELP_EDIT: [&str; 15] = [
    "L-CLICK/DRAG   PAINT",
    "R-CLICK        EYEDROP",
    "F              FLOOD FILL",
    "L              LINE TOOL",
    "R / SHIFT+R    RECT / FILLED",
    "M              MIRROR-DRAW",
    "BRACKET KEYS   SHADE SHIFT",
    "H / V          FLIP WINDOW",
    "CTRL+C         COPY WINDOW",
    "CTRL+V         PASTE (CLICK)",
    "SHIFT+ARROWS   NUDGE (WRAPS)",
    "U / CTRL+Z     UNDO",
    "Y / CTRL+Y     REDO",
    "E              ERASER",
    "C              CUSTOM COLOR",
];

/// Right column: navigating, previewing and saving.
const HELP_NAV: [&str; 16] = [
    "ARROWS         MOVE CELL/FILE",
    "I / K          STEP VERTICALLY",
    "TAB            8/16 WINDOW",
    "G              SNAP TO SPRITE",
    "W              FILES <> WHOLE SHEET",
    "N              NEW SPRITE FILE",
    "SLASH          FIND FILE BY NAME",
    "WHEEL          ZOOM AT CURSOR",
    "MIDDLE-DRAG    PAN",
    "P / SHIFT+P    PREVIEW PALETTE",
    "D / SHIFT+D    PREVIEW BACKDROP",
    "A              ANIMATE FRAMES",
    "B / O          ONION SET / TOGGLE",
    "S / CTRL+S     SAVE (+.BAK)",
    "X              REVERT FROM DISK",
    "ESC            CLOSE / QUIT",
];

impl Studio {
    pub(crate) fn draw_help(&mut self) {
        let (x, y, w, h) = (120, 80, VIEW_W - 240, VIEW_H - 160);
        self.fill_rect(x - 2, y - 2, w + 4, h + 4, GRID_MAJOR);
        self.fill_rect(x, y, w, h, PANEL);
        self.draw_text(x + 16, y + 10, "PIXEL STUDIO KEYS", TXT);
        for (i, l) in HELP_EDIT.iter().enumerate() {
            self.draw_text(x + 16, y + 30 + i as i32 * 12, l, TXT_DIM);
        }
        for (i, l) in HELP_NAV.iter().enumerate() {
            self.draw_text(x + w / 2 + 8, y + 30 + i as i32 * 12, l, TXT_DIM);
        }
        self.draw_text(
            x + 16,
            y + h - 20,
            "PAL GRAYS 0/85/170/255 ONLY - NEVER MIX PAL + RGB IN A FILE",
            TXT_WARN,
        );
    }

    /// The `N` new-sprite modal.
    pub(crate) fn draw_new_sprite(&mut self) {
        let Some(ns) = &self.new_sprite else { return };
        let preset_label = SIZE_PRESETS
            .get(ns.preset)
            .map(|&(.., l)| l)
            .unwrap_or("CUSTOM");
        let name_line = format!("NAME: {}_", ns.name);
        let size_line = format!("SIZE: {}X{} PX  ({preset_label})", ns.w, ns.h);
        let mode_line = format!(
            "MODE: {}",
            if ns.pal {
                "PAL - GRAY LADDER, RECOLORED IN-GAME (TAB SWITCHES)"
            } else {
                "RGB - TRUE COLOR, THE DEFAULT FOR NEW ART (TAB SWITCHES)"
            }
        );
        let (x, y, w, h) = (140, 220, VIEW_W - 280, 200);
        self.fill_rect(x - 2, y - 2, w + 4, h + 4, GRID_MAJOR);
        self.fill_rect(x, y, w, h, PANEL);
        self.draw_text(x + 16, y + 12, "NEW SPRITE", TXT);
        self.draw_text(x + 16, y + 36, &name_line, TXT);
        self.draw_text(x + 16, y + 52, &size_line, TXT);
        self.draw_text(x + 16, y + 68, &mode_line, TXT_DIM);
        let help = [
            "TYPE THE PATH NAME - FOLDERS WITH /  (E.G. ITEMS/MOONFRUIT)",
            "UP/DOWN SIZE PRESETS - SHIFT+ARROWS CUSTOM SIZE (8PX STEPS)",
            "ENTER CREATE + OPEN - ESC CANCEL",
            "",
            "NO MANIFEST LINE NEEDED: NEW FILES AUTO-ALLOCATE ON THE",
            "ATLAS AND ARE ADDRESSED BY NAME (ART_GUIDE.MD).",
        ];
        for (i, l) in help.iter().enumerate() {
            self.draw_text(x + 16, y + 96 + i as i32 * 14, l, TXT_DIM);
        }
    }
}
