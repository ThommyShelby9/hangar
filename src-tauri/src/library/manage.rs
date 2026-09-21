//! Changing games after import: new folder, uninstall, own images, launch actions, collections.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use super::{manual_key, names};

// ── Relocate ────────────────────────────────────────────────────────────────

/// Points a manual game at its new folder (drive letter changed, folder moved). Launch actions under
/// the old folder follow; play time and images are kept.
pub fn relocate(conn: &mut Connection, id: i64, new_dir: &str) -> Result<(), String> {
    let (source, old_dir): (String, Option<String>) = conn
        .query_row("SELECT source, install_dir FROM games WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| e.to_string())?;
    if source != "manual" {
        return Err("Seuls les jeux ajoutés à la main peuvent être déplacés : Steam et Epic se mettent à jour seuls.".into());
    }
    let old_dir = old_dir.ok_or("Ce jeu n'a pas de dossier.")?;
    let new_dir = new_dir.trim_end_matches(['\\', '/']).to_string();

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let actions: Vec<(i64, String, Option<String>)> = tx
        .prepare("SELECT id, target, working_dir FROM launch_actions WHERE game_id = ?1")
        .and_then(|mut s| s.query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect())
        .map_err(|e| e.to_string())?;
    for (action_id, target, working_dir) in actions {
        let target = rebase(&target, &old_dir, &new_dir);
        if !target.contains("://") && !Path::new(&target).exists() {
            return Err(format!("{target} n'existe pas : ce n'est pas le bon dossier."));
        }
        let working_dir = working_dir.map(|w| rebase(&w, &old_dir, &new_dir));
        tx.execute(
            "UPDATE launch_actions SET target = ?2, working_dir = ?3 WHERE id = ?1",
            params![action_id, target, working_dir],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.execute(
        "UPDATE games SET install_dir = ?2, source_id = ?3, status = 'ok' WHERE id = ?1",
        params![id, new_dir, manual_key(&new_dir)],
    )
    .map_err(|e| format!("Un autre jeu utilise déjà ce dossier ({e})."))?;
    tx.commit().map_err(|e| e.to_string())
}

/// Replaces the `old` folder prefix of `path` with `new`, ignoring case (Windows paths).
fn rebase(path: &str, old: &str, new: &str) -> String {
    let old = old.trim_end_matches(['\\', '/']);
    if path.len() >= old.len() && path[..old.len()].eq_ignore_ascii_case(old) {
        format!("{new}{}", &path[old.len()..])
    } else {
        path.to_string()
    }
}

// ── Uninstall ───────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UninstallInfo {
    pub install_dir: Option<String>,
    /// The game's own uninstaller (unins000.exe…), when there is one.
    pub uninstaller: Option<String>,
}

pub fn uninstall_info(conn: &Connection, id: i64) -> rusqlite::Result<UninstallInfo> {
    let install_dir: Option<String> =
        conn.query_row("SELECT install_dir FROM games WHERE id = ?1", [id], |r| r.get(0))?;
    let uninstaller = install_dir.as_deref().and_then(|d| find_uninstaller(Path::new(d)));
    Ok(UninstallInfo { install_dir, uninstaller: uninstaller.map(|p| p.display().to_string()) })
}

fn find_uninstaller(dir: &Path) -> Option<PathBuf> {
    let candidates = [dir.to_path_buf(), dir.join("Uninstall"), dir.join("uninstall")];
    candidates.iter().filter_map(|d| std::fs::read_dir(d).ok()).flatten().flatten().map(|e| e.path()).find(|p| {
        let name = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        name.ends_with(".exe") && (name.starts_with("unins") || name.starts_with("uninstall") || name.starts_with("desinstal"))
    })
}

/// Sends a folder to the Recycle Bin (recoverable), with Windows' own progress window.
#[cfg(windows)]
pub fn recycle(dir: &Path) -> Result<(), String> {
    use windows_sys::Win32::UI::Shell::{
        SHFileOperationW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FO_DELETE, SHFILEOPSTRUCTW,
    };
    if !dir.is_dir() {
        return Err(format!("{} n'existe plus.", dir.display()));
    }
    // Double NUL-terminated list of paths.
    let from: Vec<u16> = dir.as_os_str().to_string_lossy().encode_utf16().chain([0, 0]).collect();
    let mut op: SHFILEOPSTRUCTW = unsafe { std::mem::zeroed() };
    op.wFunc = FO_DELETE;
    op.pFrom = from.as_ptr();
    op.fFlags = (FOF_ALLOWUNDO | FOF_NOCONFIRMATION) as u16;
    let result = unsafe { SHFileOperationW(&mut op) };
    if result != 0 || op.fAnyOperationsAborted != 0 {
        return Err(format!("La suppression a échoué ou a été annulée (code {result})."));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn recycle(_dir: &Path) -> Result<(), String> {
    Err("Disponible uniquement sous Windows".into())
}

// ── Own images ──────────────────────────────────────────────────────────────

/// Copies a picture chosen by the user into the cache and returns its path relative to the cache.
pub fn copy_custom_image(cache: &Path, game_id: i64, kind: &str, source: &Path) -> Result<String, String> {
    let ext = source
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .filter(|e| matches!(e.as_str(), "png" | "jpg" | "jpeg" | "webp" | "ico"))
        .ok_or("Choisis une image PNG, JPG, WebP ou ICO.")?;
    let dir = cache.join("games").join(game_id.to_string());
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with(&format!("{kind}-")) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    let name = format!("{kind}-{}.{ext}", crate::db::now());
    std::fs::copy(source, dir.join(&name)).map_err(|e| e.to_string())?;
    Ok(format!("games/{game_id}/{name}"))
}

// ── Launch actions ──────────────────────────────────────────────────────────

pub fn add_action(conn: &Connection, game_id: i64, name: &str, target: &str, args: &str) -> rusqlite::Result<i64> {
    let working_dir = (!target.contains("://"))
        .then(|| Path::new(target).parent().map(|p| p.display().to_string()))
        .flatten();
    let position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), 0) + 1 FROM launch_actions WHERE game_id = ?1",
        [game_id],
        |r| r.get(0),
    )?;
    conn.execute(
        "INSERT INTO launch_actions (game_id, name, target, args, working_dir, position) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![game_id, name, target, args, working_dir, position],
    )?;
    Ok(conn.last_insert_rowid())
}

/// The default action (the Play button) can't be removed, only replaced by another one.
pub fn remove_action(conn: &Connection, action_id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM launch_actions WHERE id = ?1 AND is_default = 0", [action_id])?;
    Ok(())
}

pub fn set_default_action(conn: &mut Connection, action_id: i64) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    let game_id: Option<i64> =
        tx.query_row("SELECT game_id FROM launch_actions WHERE id = ?1", [action_id], |r| r.get(0)).optional()?;
    if let Some(game_id) = game_id {
        tx.execute("UPDATE launch_actions SET is_default = (id = ?2) WHERE game_id = ?1", params![game_id, action_id])?;
    }
    tx.commit()
}

// ── Collections ─────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub game_count: i64,
}

pub fn list_collections(conn: &Connection) -> rusqlite::Result<Vec<Collection>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, COUNT(cg.game_id) FROM collections c
         LEFT JOIN collection_games cg ON cg.collection_id = c.id
         GROUP BY c.id ORDER BY c.name COLLATE NOCASE",
    )?;
    let list = stmt.query_map([], |r| Ok(Collection { id: r.get(0)?, name: r.get(1)?, game_count: r.get(2)? }))?.collect();
    list
}

pub fn create_collection(conn: &Connection, name: &str) -> Result<i64, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Donne un nom à la collection.".into());
    }
    conn.execute("INSERT INTO collections (name) VALUES (?1)", [name])
        .map_err(|_| format!("La collection « {name} » existe déjà."))?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_collection(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM collections WHERE id = ?1", [id])?;
    Ok(())
}

pub fn set_game_collections(conn: &mut Connection, game_id: i64, ids: &[i64]) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM collection_games WHERE game_id = ?1", [game_id])?;
    for id in ids {
        tx.execute("INSERT OR IGNORE INTO collection_games (collection_id, game_id) VALUES (?1, ?2)", params![id, game_id])?;
    }
    tx.commit()
}

/// A manual game from a shortcut pointing at a URI (e.g. a Steam `.url`): no folder.
pub fn title_from_shortcut(path: &Path) -> String {
    names::clean_title(&path.file_stem().unwrap_or_default().to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::library::{add_manual_game, get_game, NewManualGame};

    #[test]
    fn rebase_is_case_insensitive() {
        assert_eq!(rebase(r"n:\games\x\X.exe", r"N:\Games\X", r"E:\Games\X"), r"E:\Games\X\X.exe");
        assert_eq!(rebase(r"steam://run/1", r"N:\Games\X", r"E:\X"), "steam://run/1");
    }

    #[test]
    fn relocate_refuses_a_folder_without_the_game() {
        let mut conn = db::open_in_memory().unwrap();
        let id = add_manual_game(
            &mut conn,
            &NewManualGame {
                dir: r"Z:\Nowhere\Game".into(),
                title: "Game".into(),
                exe: r"Z:\Nowhere\Game\game.exe".into(),
                args: String::new(),
                extras: vec![],
            },
        )
        .unwrap();
        assert!(relocate(&mut conn, id, r"Y:\Still\Nowhere").is_err());
        assert_eq!(get_game(&conn, id).unwrap().unwrap().game.install_dir.as_deref(), Some(r"Z:\Nowhere\Game"));
    }

    #[test]
    fn collections_and_actions() {
        let mut conn = db::open_in_memory().unwrap();
        let game = add_manual_game(
            &mut conn,
            &NewManualGame {
                dir: r"N:\Limbo".into(),
                title: "Limbo".into(),
                exe: r"N:\Limbo\limbo.exe".into(),
                args: String::new(),
                extras: vec![],
            },
        )
        .unwrap();
        let solo = create_collection(&conn, "Solo").unwrap();
        assert!(create_collection(&conn, "Solo").is_err());
        set_game_collections(&mut conn, game, &[solo]).unwrap();
        assert_eq!(list_collections(&conn).unwrap()[0].game_count, 1);

        let mods = add_action(&conn, game, "Mods", r"N:\Limbo\mods.exe", "").unwrap();
        set_default_action(&mut conn, mods).unwrap();
        let actions = crate::library::list_actions(&conn, game).unwrap();
        assert_eq!(actions[0].name, "Mods");
        remove_action(&conn, mods).unwrap();
        assert_eq!(crate::library::list_actions(&conn, game).unwrap().len(), 2, "l'action par défaut reste");
    }
}
