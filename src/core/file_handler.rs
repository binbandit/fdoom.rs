//! Port of `fdoom.core.FileHandler`.

use std::path::{Path, PathBuf};

pub const REPLACE_EXISTING: i32 = 0;
pub const RENAME_COPY: i32 = 1;
pub const SKIP: i32 = 2;

/// Java `Save.extension` (referenced here for the rename-copy suffix).
pub const SAVE_EXTENSION: &str = ".fdoom";

/// Java `FileHandler.systemGameDir` — %APPDATA% on Windows, the home directory elsewhere.
pub fn system_game_dir() -> String {
    if cfg!(windows) {
        std::env::var("APPDATA").unwrap_or_default()
    } else {
        std::env::var("HOME").unwrap_or_default()
    }
}

/// Java `FileHandler.localGameDir` — "/fdoom" on mac/windows, "/.fdoom" on linux.
pub fn local_game_dir() -> &'static str {
    if cfg!(target_os = "linux") {
        "/.fdoom"
    } else {
        "/fdoom"
    }
}

/// Java `FileHandler.determineGameDir(saveDir)`. The `_debug` parameter is kept for
/// call-site compatibility; verbosity now follows the global log threshold.
pub fn determine_game_dir(save_dir: &str, _debug: bool) -> PathBuf {
    let game_dir = PathBuf::from(format!("{save_dir}{}", local_game_dir()));
    crate::log_debug!("determined game dir: {}", game_dir.display());

    let _ = std::fs::create_dir_all(&game_dir);

    // migrate saves from the legacy "/.fdoom" folder if one is present
    let old_folder = PathBuf::from(format!("{save_dir}/.fdoom"));
    if old_folder.exists() && old_folder != game_dir {
        if let Err(e) = copy_folder_contents(&old_folder, &game_dir, RENAME_COPY, true, _debug) {
            crate::log_error!(
                "migrating legacy saves from {} to {} failed: {e}; \
                 unmigrated saves remain in the legacy folder",
                old_folder.display(),
                game_dir.display()
            );
        }
    }

    game_dir
}

/// Java `FileHandler.copyFolderContents(origFolder, newFolder, ifExisting, deleteOriginal)`.
/// The `_debug` parameter is kept for call-site compatibility; verbosity now follows
/// the global log threshold.
pub fn copy_folder_contents(
    orig_folder: &Path,
    new_folder: &Path,
    if_existing: i32,
    delete_original: bool,
    _debug: bool,
) -> std::io::Result<()> {
    crate::log_debug!(
        "copying folder contents {} -> {} (mode {if_existing}, delete_original: {delete_original})",
        orig_folder.display(),
        new_folder.display()
    );

    copy_dir_recursive(orig_folder, orig_folder, new_folder, if_existing)?;

    if delete_original {
        std::fs::remove_dir_all(orig_folder)?;
    }
    Ok(())
}

fn copy_dir_recursive(
    root: &Path,
    dir: &Path,
    new_root: &Path,
    if_existing: i32,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            copy_dir_recursive(root, &path, new_root, if_existing)?;
        } else {
            let relative = path
                .strip_prefix(root)
                .expect("read_dir paths descend from root by construction");
            let mut new_filename = new_root.join(relative);
            if new_filename.exists() {
                if if_existing == SKIP {
                    continue;
                } else if if_existing == RENAME_COPY {
                    // keep the existing file: rename the incoming copy by appending
                    // "(Old)" to its stem until the name is unique
                    let stem = new_filename.with_extension("");
                    let mut candidate = stem.as_os_str().to_string_lossy().to_string();
                    loop {
                        candidate.push_str("(Old)");
                        if !Path::new(&candidate).exists() {
                            break;
                        }
                    }
                    candidate.push_str(SAVE_EXTENSION);
                    new_filename = PathBuf::from(candidate);
                }
            }
            if let Some(parent) = new_filename.parent() {
                std::fs::create_dir_all(parent)?;
            }
            if let Err(ex) = std::fs::copy(&path, &new_filename) {
                crate::log_error!(
                    "copying {} -> {} failed: {ex}; the file is skipped \
                     (and is lost if the source folder is deleted after the copy)",
                    path.display(),
                    new_filename.display()
                );
            }
        }
    }
    Ok(())
}
