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

/// Java `FileHandler.determineGameDir(saveDir)`.
pub fn determine_game_dir(save_dir: &str, debug: bool) -> PathBuf {
    let game_dir = PathBuf::from(format!("{save_dir}{}", local_game_dir()));
    if debug {
        println!("determined gameDir: {}", game_dir.display());
    }

    let _ = std::fs::create_dir_all(&game_dir);

    // migrate saves from the legacy "/.fdoom" folder if one is present
    let old_folder = PathBuf::from(format!("{save_dir}/.fdoom"));
    if old_folder.exists() && old_folder != game_dir {
        if let Err(e) = copy_folder_contents(&old_folder, &game_dir, RENAME_COPY, true, debug) {
            eprintln!("error migrating old game folder: {e}");
        }
    }

    game_dir
}

/// Java `FileHandler.copyFolderContents(origFolder, newFolder, ifExisting, deleteOriginal)`.
pub fn copy_folder_contents(
    orig_folder: &Path,
    new_folder: &Path,
    if_existing: i32,
    delete_original: bool,
    debug: bool,
) -> std::io::Result<()> {
    if debug {
        println!(
            "copying contents of folder {} to new folder {}",
            orig_folder.display(),
            new_folder.display()
        );
    }

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
            let relative = path.strip_prefix(root).unwrap();
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
                eprintln!("copy failed: {ex}");
            }
        }
    }
    Ok(())
}
