//! Everything that opens, saves or replaces the open document.
//!
//! The PNGs on disk are the art source of truth, so this module is where the studio
//! is allowed to touch them — and the rules it keeps are the ones that make editing
//! in place safe:
//!
//! - **Back up once per session.** The first save of a file copies it to
//!   `<name>.bak.png`; later saves leave that backup alone, so it is always the art
//!   as it was when the studio opened.
//! - **Only write what changed.** Canvas mode saves the dirty placements and nothing
//!   else, so untouched files stay byte-identical on disk.
//! - **Never lose unsaved work silently.** Switching file, switching view, or
//!   creating a sprite refuses while dirty (Shift+click is the explicit discard).
//!
//! Replacing the document always drops the undo history with it: the snapshots
//! describe pixels that no longer exist.

use std::path::Path;

use crate::canvas::{Stitched, build_canvas, canvas_extract};
use crate::image::{Image, bak_path, load_png, write_png};
use crate::library::{is_legal_sprite_name, load_manifest_modes, walk};

use super::{NewSprite, SIZE_PRESETS, Source, Studio};

impl Studio {
    /* ------------------------------------ saving ------------------------------------ */

    pub(crate) fn save(&mut self) {
        match self.source {
            Source::Canvas { .. } => self.save_canvas(),
            _ => self.save_file(),
        }
    }

    /// Copy `path` to its `.bak.png` the first time this session saves it.
    fn backup_once(&mut self, path: &Path) -> Result<(), String> {
        if self.backed_up.contains(path) {
            return Ok(());
        }
        std::fs::copy(path, bak_path(path)).map_err(|e| e.to_string())?;
        self.backed_up.insert(path.to_path_buf());
        Ok(())
    }

    /// Sheet and file modes: write the one open image back.
    fn save_file(&mut self) {
        let path = self.path.clone();
        if let Err(e) = self.backup_once(&path) {
            self.status = format!("BACKUP FAILED: {e}");
            return;
        }
        match write_png(&self.path, &self.img) {
            Ok(()) => {
                self.dirty = false;
                self.esc_armed = false;
                self.status = format!("SAVED {}", self.file_label());
            }
            Err(e) => self.status = format!("SAVE FAILED: {e}"),
        }
    }

    /// Canvas mode `S`: write back only the dirty files, each backed up once per
    /// session; untouched files are never rewritten (byte-identical on disk).
    fn save_canvas(&mut self) {
        let Source::Canvas { placements, .. } = &self.source else {
            return;
        };
        let dirty: Vec<usize> = placements
            .iter()
            .enumerate()
            .filter(|(_, p)| p.dirty)
            .map(|(i, _)| i)
            .collect();
        if dirty.is_empty() {
            self.status = "NOTHING TO SAVE (NO DIRTY FILES)".into();
            return;
        }
        let mut saved = Vec::new();
        for i in dirty {
            let (path, rel, out) = {
                let Source::Canvas { placements, .. } = &self.source else {
                    return;
                };
                let p = &placements[i];
                (p.path.clone(), p.rel.clone(), canvas_extract(&self.img, p))
            };
            if let Err(e) = self.backup_once(&path) {
                self.status = format!("BACKUP FAILED ({rel}): {e}");
                return;
            }
            if let Err(e) = write_png(&path, &out) {
                self.status = format!("SAVE FAILED ({rel}): {e}");
                return;
            }
            if let Source::Canvas { placements, .. } = &mut self.source {
                placements[i].dirty = false;
            }
            saved.push(rel);
        }
        self.dirty = false;
        self.esc_armed = false;
        self.status = format!("SAVED {} FILE(S): {}", saved.len(), saved.join(", "));
    }

    /* ----------------------------------- reverting ----------------------------------- */

    /// `X`: reload the open file from disk, dropping unsaved edits.
    pub(crate) fn revert(&mut self) {
        if let Source::Canvas { .. } = self.source {
            self.revert_canvas();
            return;
        }
        match load_png(&self.path) {
            Ok(img) => {
                self.img = img;
                self.reset_after_reload();
                self.status = "REVERTED FROM DISK".into();
            }
            Err(e) => self.status = format!("REVERT FAILED: {e}"),
        }
    }

    fn revert_canvas(&mut self) {
        let Some(root) = self.root.clone() else {
            return;
        };
        match build_canvas(&root) {
            Ok(st) => {
                self.adopt_canvas(st);
                self.reset_after_reload();
                self.status = "CANVAS REBUILT FROM DISK".into();
            }
            Err(e) => self.status = format!("REVERT FAILED: {e}"),
        }
    }

    /// Shared tail of a reload: the document on screen now matches disk, and the
    /// undo snapshots describe pixels that no longer exist.
    fn reset_after_reload(&mut self) {
        self.dirty = false;
        self.history.clear();
        self.esc_armed = false;
        self.set_origin(self.bx, self.by);
    }

    /// Install a freshly stitched canvas as the open document.
    fn adopt_canvas(&mut self, st: Stitched) {
        let Stitched {
            img,
            placements,
            owner,
        } = st;
        self.img = img;
        self.source = Source::Canvas { placements, owner };
    }

    /* ------------------------------- switching views ------------------------------- */

    /// `W`: toggle between the file browser and the stitched all-files canvas.
    pub(crate) fn toggle_canvas(&mut self) {
        if self.dirty {
            self.status = "UNSAVED EDITS: S SAVE OR X REVERT BEFORE SWITCHING VIEWS".into();
            return;
        }
        let Some(root) = self.root.clone() else {
            self.status = "CANVAS: DIRECTORY TARGETS ONLY".into();
            return;
        };
        match &self.source {
            Source::Tree { .. } => self.enter_canvas(&root),
            Source::Canvas { .. } => self.leave_canvas(&root),
            Source::Sheet => self.status = "CANVAS: DIRECTORY TARGETS ONLY".into(),
        }
    }

    /// File browser -> canvas: stitch the tree and land on the file that was open.
    fn enter_canvas(&mut self, root: &Path) {
        let Source::Tree { entries, sel, .. } = &self.source else {
            return;
        };
        let open_rel = entries[*sel].rel.clone();
        self.tree_rel = Some(open_rel);
        match build_canvas(root) {
            Ok(st) => {
                let land = self
                    .tree_rel
                    .as_ref()
                    .and_then(|r| st.placements.iter().find(|p| &p.rel == r))
                    .map(|p| (p.x, p.y, p.w, p.h));
                self.adopt_canvas(st);
                self.history.clear();
                self.zoom_ovr = None;
                self.pan = (0, 0);
                self.anim_on = false;
                self.anim_files.clear();
                let (x, y, w, h) = land.unwrap_or((0, 0, 16, 16));
                self.set_view(w, h);
                self.set_origin(x, y);
                self.status = "CANVAS: EVERY FILE, ONE SHEET — W BACK TO FILES".into();
            }
            Err(e) => self.status = format!("CANVAS FAILED: {e}"),
        }
    }

    /// Canvas -> file browser: reselect the file the window was sitting on.
    fn leave_canvas(&mut self, root: &Path) {
        let entries = walk(root);
        let sel = self
            .tree_rel
            .as_ref()
            .and_then(|r| entries.iter().position(|e| !e.is_dir && &e.rel == r))
            .or_else(|| entries.iter().position(|e| !e.is_dir));
        let Some(sel) = sel else {
            self.status = "NO FILES LEFT UNDER THE TREE".into();
            return;
        };
        let path = entries[sel].path.clone();
        match load_png(&path) {
            Ok(img) => {
                self.source = Source::Tree {
                    entries,
                    sel,
                    scroll: 0,
                };
                self.path = path;
                self.img = img;
                self.history.clear();
                self.zoom_ovr = None;
                self.pan = (0, 0);
                self.set_view(16, 16);
                self.set_origin(0, 0);
                self.status = "FILE VIEW".into();
            }
            Err(e) => self.status = format!("OPEN FAILED: {e}"),
        }
    }

    /* ------------------------------- the new-sprite flow ------------------------------- */

    /// `N` (dir/canvas modes): open the new-sprite modal.
    pub(crate) fn open_new_sprite(&mut self) {
        if self.root.is_none() {
            self.status = "NEW SPRITE: DIRECTORY TARGETS ONLY".into();
            return;
        }
        if self.dirty {
            self.status = "UNSAVED EDITS: SAVE (S) BEFORE CREATING A NEW SPRITE".into();
            return;
        }
        let (w, h, _) = SIZE_PRESETS[0];
        self.new_sprite = Some(NewSprite {
            name: String::new(),
            preset: 0,
            w,
            h,
            pal: false,
        });
    }

    /// Enter inside the modal: validate, write the blank PNG, open it for editing.
    pub(crate) fn create_new_sprite(&mut self) {
        let Some(ns) = &self.new_sprite else { return };
        let Some(root) = self.root.clone() else {
            return;
        };
        let name = ns.name.trim_matches('/').to_string();
        let (w, h, pal) = (ns.w, ns.h, ns.pal);
        if name.is_empty() {
            self.status = "NEW: TYPE A NAME (E.G. ITEMS/MOONFRUIT)".into();
            return;
        }
        if !is_legal_sprite_name(&name) {
            self.status = "NEW: LOWERCASE LETTERS, DIGITS, _ - AND / ONLY".into();
            return;
        }
        let rel = format!("{name}.png");
        let path = root.join(&rel);
        if path.exists() {
            self.status = format!("NEW: {rel} ALREADY EXISTS");
            return;
        }
        if let Some(parent) = path.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            self.status = format!("NEW: MKDIR FAILED: {e}");
            return;
        }
        if let Err(e) = write_png(&path, &Image::blank(w, h)) {
            self.status = format!("NEW: WRITE FAILED: {e}");
            return;
        }
        self.new_sprite = None;
        self.open_created(&root, &rel);
        self.manifest = load_manifest_modes(&root);
        self.status = format!(
            "CREATED {rel} ({}) — ADD IT TO {} IN tests/sprite_atlas.rs",
            if pal { "PAL" } else { "RGB" },
            if pal { "UNPINNED_PAL" } else { "UNPINNED_RGB" },
        );
    }

    /// Land the editor on a sprite that was just created: canvas mode restitches so
    /// the file gets its auto-allocated cells, file mode rewalks and opens it.
    fn open_created(&mut self, root: &Path, rel: &str) {
        match &self.source {
            Source::Canvas { .. } => {
                if let Ok(st) = build_canvas(root) {
                    let land = st
                        .placements
                        .iter()
                        .find(|p| p.rel == rel)
                        .map(|p| (p.x, p.y, p.w, p.h));
                    self.adopt_canvas(st);
                    self.history.clear();
                    if let Some((x, y, w, h)) = land {
                        self.set_view(w, h);
                        self.set_origin(x, y);
                    }
                }
            }
            _ => {
                let entries = walk(root);
                let idx = entries.iter().position(|e| !e.is_dir && e.rel == rel);
                self.source = Source::Tree {
                    entries,
                    sel: 0,
                    scroll: 0,
                };
                if let Some(idx) = idx {
                    // fresh Tree source: sel 0 is the root dir, so open_entry always moves
                    self.open_entry(idx, true);
                }
            }
        }
    }

    /* -------------------------------- browsing files -------------------------------- */

    /// Dir mode: open the file entry at `idx` (blocked while dirty unless `force`).
    pub(crate) fn open_entry(&mut self, idx: usize, force: bool) {
        let Source::Tree { entries, sel, .. } = &mut self.source else {
            return;
        };
        if idx >= entries.len() || entries[idx].is_dir || idx == *sel {
            return;
        }
        if self.dirty && !force {
            self.status = "UNSAVED EDITS: S SAVE, X REVERT, OR SHIFT+CLICK TO DISCARD".into();
            return;
        }
        let path = entries[idx].path.clone();
        match load_png(&path) {
            Ok(img) => {
                *sel = idx;
                self.path = path;
                self.img = img;
                self.bx = 0;
                self.by = 0;
                self.zoom_ovr = None;
                self.pan = (0, 0);
                self.history.clear();
                self.dirty = false;
                self.hover = None;
                self.drag_anchor = None;
                self.esc_armed = false;
                self.anim_on = false;
                self.anim_files.clear();
                self.status = String::new();
            }
            Err(e) => self.status = format!("OPEN FAILED: {e}"),
        }
    }

    /// Dir mode: move the file selection up/down, skipping folder headers.
    pub(crate) fn move_file_sel(&mut self, dir: i32) {
        let Source::Tree { entries, sel, .. } = &self.source else {
            return;
        };
        let mut i = *sel as i32;
        loop {
            i += dir;
            if i < 0 || i >= entries.len() as i32 {
                return;
            }
            if !entries[i as usize].is_dir {
                break;
            }
        }
        self.open_entry(i as usize, false);
    }

    /// `/` finder: jump the file selection to the next entry whose path contains the
    /// typed needle. `dir` walks forward/backward; `from_next` skips the current row.
    pub(crate) fn find_apply(&mut self, dir: i32, from_next: bool) {
        let Some(needle) = self.find.clone() else {
            return;
        };
        let needle = needle.to_ascii_lowercase();
        let Source::Tree { entries, sel, .. } = &self.source else {
            return;
        };
        if needle.is_empty() {
            self.status = "FIND: TYPE PART OF A FILE NAME (ESC CANCELS)".into();
            return;
        }
        let (n, start) = (entries.len() as i32, *sel as i32);
        let hit = (i32::from(from_next)..n).find_map(|step| {
            let i = (start + dir * step).rem_euclid(n) as usize;
            (!entries[i].is_dir && entries[i].rel.to_ascii_lowercase().contains(&needle))
                .then_some(i)
        });
        match hit {
            Some(i) => {
                self.open_entry(i, false);
                self.status = format!("FIND {needle}: UP/DOWN NEXT/PREV, ENTER DONE");
            }
            None => self.status = format!("FIND {needle}: NO MATCH"),
        }
    }
}
