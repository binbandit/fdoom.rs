//! The sprite tree on disk: the browser's recursive walk and the atlas manifest.
//!
//! `assets/sprites/**` is the art source of truth, so the walk defines what the
//! studio considers editable — `*.png` only, backups and dotfiles hidden — and its
//! order is the file list the artist arrows through. The manifest gives each pinned
//! file its declared `pal`/`rgb` mode, which drives the per-file wrong-mode warning.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub(crate) struct Entry {
    pub(crate) path: PathBuf,
    /// Display path relative to the root, e.g. `tiles/grass.png` or `tiles/`.
    pub(crate) rel: String,
    pub(crate) depth: i32,
    pub(crate) is_dir: bool,
}

/// Recursive, sorted walk: each directory contributes a header row, then its `*.png`
/// files, then its subdirectories. Backups (`*.bak.png`) and dotfiles are skipped.
pub(crate) fn walk(root: &Path) -> Vec<Entry> {
    fn rec(dir: &Path, root: &Path, depth: i32, out: &mut Vec<Entry>) {
        let rel = dir
            .strip_prefix(root)
            .ok()
            .filter(|r| !r.as_os_str().is_empty())
            .map(|r| format!("{}/", r.display()))
            .unwrap_or_else(|| {
                format!(
                    "{}/",
                    root.file_name()
                        .map(|n| n.to_string_lossy())
                        .unwrap_or_default()
                )
            });
        out.push(Entry {
            path: dir.to_path_buf(),
            rel,
            depth,
            is_dir: true,
        });
        let mut names: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|it| it.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        names.sort();
        for p in names.iter().filter(|p| p.is_file()) {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if !name.to_ascii_lowercase().ends_with(".png")
                || name.to_ascii_lowercase().ends_with(".bak.png")
                || name.starts_with('.')
            {
                continue;
            }
            out.push(Entry {
                path: p.clone(),
                rel: p
                    .strip_prefix(root)
                    .map(|r| r.display().to_string())
                    .unwrap_or(name),
                depth: depth + 1,
                is_dir: false,
            });
        }
        for p in names.iter().filter(|p| p.is_dir()) {
            let hidden = p
                .file_name()
                .map(|n| n.to_string_lossy().starts_with('.'))
                .unwrap_or(true);
            if !hidden {
                rec(p, root, depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    rec(root, root, 0, &mut out);
    out
}

/// Parse the atlas manifest's `<path> <cx> <cy> <w> <h> <pal|rgb>` lines into a
/// rel-path -> is_palette map (used for per-file wrong-mode warnings).
pub(crate) fn load_manifest_modes(root: &Path) -> HashMap<String, bool> {
    let mut modes = HashMap::new();
    let Ok(text) = std::fs::read_to_string(root.join("manifest.txt")) else {
        return modes;
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut it = line.split_whitespace();
        let (Some(path), Some(mode)) = (it.next(), it.nth(4)) else {
            continue;
        };
        modes.insert(path.to_string(), mode == "pal");
    }
    modes
}

/// Whether `name` is a legal sprite path-name (no extension): lowercase ASCII,
/// digits, `_`, `-` and `/`, with no empty path segments.
///
/// Code addresses sprites by this exact string (`sheet.cell("items/moonfruit")`) and
/// the manifest is case-sensitive, so an uppercase or spaced name would build a file
/// the game can never find. Both the `N` modal and headless `--new` check this.
pub(crate) fn is_legal_sprite_name(name: &str) -> bool {
    !name.contains("//")
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_-/".contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("pixel_studio_lib_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"").unwrap();
    }

    /// The walk lists a folder header then its files then its subfolders, sorted, and
    /// hides everything the artist must not edit (backups, dotfiles, non-PNGs).
    #[test]
    fn walk_orders_folders_and_hides_backups() {
        let root = temp_root("walk");
        touch(&root.join("b.png"));
        touch(&root.join("a.png"));
        touch(&root.join("a.bak.png"));
        touch(&root.join(".hidden.png"));
        touch(&root.join("notes.txt"));
        touch(&root.join("tiles/grass.png"));
        touch(&root.join(".git/config.png"));

        let entries = walk(&root);
        let rels: Vec<&str> = entries.iter().map(|e| e.rel.as_str()).collect();
        assert_eq!(
            rels[0],
            format!("{}/", root.file_name().unwrap().to_string_lossy())
        );
        assert_eq!(&rels[1..], &["a.png", "b.png", "tiles/", "tiles/grass.png"]);

        assert!(entries[0].is_dir && entries[0].depth == 0);
        assert!(!entries[1].is_dir && entries[1].depth == 1);
        let tiles = entries.iter().find(|e| e.rel == "tiles/grass.png").unwrap();
        assert_eq!(tiles.depth, 2, "nested files indent one further");
        assert_eq!(tiles.path, root.join("tiles/grass.png"));

        std::fs::remove_dir_all(&root).ok();
    }

    /// A missing manifest is not an error — unpinned trees simply have no declared
    /// modes, and every file auto-allocates.
    #[test]
    fn a_missing_manifest_is_empty_not_fatal() {
        let root = temp_root("nomanifest");
        assert!(load_manifest_modes(&root).is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    /// Manifest parsing reads the mode column, skipping comments and blank lines and
    /// tolerating short rows rather than dropping the whole file.
    #[test]
    fn manifest_modes_parse_the_sixth_column() {
        let root = temp_root("manifest");
        std::fs::write(
            root.join("manifest.txt"),
            "# a comment\n\n\
             tiles/grass.png 0 0 2 2 rgb\n\
             font/a.png 4 4 1 1 pal\n\
             truncated.png 1 2\n   \n\
             items/pan.png 9 9 1 1 rgb\n",
        )
        .unwrap();

        let modes = load_manifest_modes(&root);
        assert_eq!(modes.len(), 3, "the truncated row is skipped");
        assert_eq!(modes.get("font/a.png"), Some(&true));
        assert_eq!(modes.get("tiles/grass.png"), Some(&false));
        assert_eq!(modes.get("items/pan.png"), Some(&false));
        assert_eq!(modes.get("truncated.png"), None);

        std::fs::remove_dir_all(&root).ok();
    }

    /// Sprite names must stay addressable by the atlas: lowercase, no spaces, no
    /// empty path segments.
    #[test]
    fn sprite_names_must_be_atlas_addressable() {
        for ok in [
            "items/moonfruit",
            "tiles/bog_flower",
            "mobs/mirelurk/walk",
            "a-1",
            "x",
        ] {
            assert!(is_legal_sprite_name(ok), "{ok} should be legal");
        }
        for bad in [
            "Items/Bad",
            "items/moon fruit",
            "items//x",
            "items/moon.fruit",
            "café",
        ] {
            assert!(!is_legal_sprite_name(bad), "{bad} should be rejected");
        }
        assert!(
            is_legal_sprite_name(""),
            "emptiness is checked by the callers"
        );
    }
}
