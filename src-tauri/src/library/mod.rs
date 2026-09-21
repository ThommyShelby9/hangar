//! The game library: one record per game, whatever its source.
//! Every feature hangs off a `games.id`, never off the platform.

pub mod exe_info;
pub mod folders;
pub mod manage;
pub mod names;
pub mod shortcut;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::db;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub id: i64,
    pub title: String,
    pub sort_title: String,
    pub source: String,
    pub install_dir: Option<String>,
    pub status: String,
    pub favorite: bool,
    pub hidden: bool,
    pub sgdb_id: Option<i64>,
    pub added_at: i64,
    pub last_played_at: Option<i64>,
    /// Total recorded play time.
    pub playtime_secs: i64,
    /// Ids of the collections this game belongs to.
    pub collections: Vec<i64>,
    /// kind ('grid', 'hero', 'logo', 'icon') → file relative to the cache folder.
    pub images: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchAction {
    pub id: i64,
    pub name: String,
    pub target: String,
    pub args: String,
    pub working_dir: Option<String>,
    pub run_as_admin: bool,
    pub is_default: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameDetail {
    #[serde(flatten)]
    pub game: Game,
    pub actions: Vec<LaunchAction>,
}

/// A game reported by a platform scanner (Steam, Epic…).
pub struct PlatformGame {
    pub source: &'static str,
    pub source_id: String,
    pub title: String,
    pub install_dir: Option<String>,
    pub launch_uri: String,
}

/// A game confirmed by the user from a folder analysis or a dropped exe.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewManualGame {
    pub dir: String,
    pub title: String,
    /// Executable, or a URI (`steam://…`) for games added from a `.url` shortcut.
    pub exe: String,
    #[serde(default)]
    pub args: String,
    #[serde(default)]
    pub extras: Vec<folders::ExtraAction>,
}

const GAME_COLUMNS: &str = "id, title, sort_title, source, install_dir, status, favorite, hidden, \
    sgdb_id, added_at, last_played_at, \
    (SELECT COALESCE(SUM(COALESCE(s.ended_at, s.last_seen_at) - s.started_at), 0) \
     FROM sessions s WHERE s.game_id = games.id)";

fn game_from_row(row: &rusqlite::Row) -> rusqlite::Result<Game> {
    Ok(Game {
        id: row.get(0)?,
        title: row.get(1)?,
        sort_title: row.get(2)?,
        source: row.get(3)?,
        install_dir: row.get(4)?,
        status: row.get(5)?,
        favorite: row.get(6)?,
        hidden: row.get(7)?,
        sgdb_id: row.get(8)?,
        added_at: row.get(9)?,
        last_played_at: row.get(10)?,
        playtime_secs: row.get(11)?,
        collections: Vec::new(),
        images: HashMap::new(),
    })
}

pub fn list_games(conn: &Connection) -> rusqlite::Result<Vec<Game>> {
    let mut stmt = conn.prepare(&format!("SELECT {GAME_COLUMNS} FROM games ORDER BY sort_title"))?;
    let mut games: Vec<Game> = stmt.query_map([], game_from_row)?.collect::<Result<_, _>>()?;

    let mut images: HashMap<i64, HashMap<String, String>> = HashMap::new();
    let mut stmt = conn.prepare("SELECT game_id, kind, file FROM game_images")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get(1)?, r.get(2)?)))?;
    for row in rows {
        let (id, kind, file) = row?;
        images.entry(id).or_default().insert(kind, file);
    }
    let mut collections: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut stmt = conn.prepare("SELECT game_id, collection_id FROM collection_games")?;
    for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get(1)?)))? {
        let (game, collection) = row?;
        collections.entry(game).or_default().push(collection);
    }
    for game in &mut games {
        game.images = images.remove(&game.id).unwrap_or_default();
        game.collections = collections.remove(&game.id).unwrap_or_default();
    }
    Ok(games)
}

pub fn get_game(conn: &Connection, id: i64) -> rusqlite::Result<Option<GameDetail>> {
    let Some(mut game) = conn
        .query_row(&format!("SELECT {GAME_COLUMNS} FROM games WHERE id = ?1"), [id], game_from_row)
        .optional()?
    else {
        return Ok(None);
    };
    let mut stmt = conn.prepare("SELECT kind, file FROM game_images WHERE game_id = ?1")?;
    game.images = stmt
        .query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    game.collections = conn
        .prepare("SELECT collection_id FROM collection_games WHERE game_id = ?1")?
        .query_map([id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(Some(GameDetail { game, actions: list_actions(conn, id)? }))
}

pub fn list_actions(conn: &Connection, game_id: i64) -> rusqlite::Result<Vec<LaunchAction>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, target, args, working_dir, run_as_admin, is_default
         FROM launch_actions WHERE game_id = ?1 ORDER BY is_default DESC, position, id",
    )?;
    let actions = stmt
        .query_map([game_id], |r| {
            Ok(LaunchAction {
                id: r.get(0)?,
                name: r.get(1)?,
                target: r.get(2)?,
                args: r.get(3)?,
                working_dir: r.get(4)?,
                run_as_admin: r.get(5)?,
                is_default: r.get(6)?,
            })
        })?
        .collect();
    actions
}

fn insert_action(
    conn: &Connection,
    game_id: i64,
    name: &str,
    target: &str,
    working_dir: Option<&str>,
    is_default: bool,
    position: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO launch_actions (game_id, name, target, working_dir, is_default, position)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![game_id, name, target, working_dir, is_default, position],
    )?;
    Ok(())
}

/// Manual games are keyed by their folder, lowercased.
pub fn manual_key(dir: &str) -> String {
    dir.trim_end_matches(['\\', '/']).to_lowercase()
}

pub fn add_manual_game(conn: &mut Connection, game: &NewManualGame) -> rusqlite::Result<i64> {
    // A `.url` shortcut (steam://…) has no folder: the link itself identifies the game.
    let is_uri = game.exe.contains("://");
    let (key, install_dir) = if is_uri { (game.exe.to_lowercase(), None) } else { (manual_key(&game.dir), Some(&game.dir)) };
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO games (title, sort_title, source, source_id, install_dir, added_at)
         VALUES (?1, ?2, 'manual', ?3, ?4, ?5)",
        params![game.title, names::sort_title(&game.title), key, install_dir, db::now()],
    )?;
    let id = tx.last_insert_rowid();
    let exe_dir = (!is_uri).then(|| Path::new(&game.exe).parent().map(|p| p.display().to_string())).flatten();
    insert_action(&tx, id, "Jouer", &game.exe, exe_dir.as_deref(), true, 0)?;
    if !game.args.is_empty() {
        tx.execute("UPDATE launch_actions SET args = ?2 WHERE game_id = ?1", params![id, game.args])?;
    }
    for (i, extra) in game.extras.iter().enumerate() {
        let dir = Path::new(&extra.path).parent().map(|p| p.display().to_string());
        insert_action(&tx, id, &extra.name, &extra.path, dir.as_deref(), false, i as i64 + 1)?;
    }
    tx.commit()?;
    Ok(id)
}

/// Inserts or refreshes platform games; games of `source` not in `found` are marked missing.
/// Returns the ids of newly inserted games.
pub fn sync_platform_games(
    conn: &mut Connection,
    source: &str,
    found: &[PlatformGame],
) -> rusqlite::Result<Vec<i64>> {
    let tx = conn.transaction()?;
    let mut inserted = Vec::new();
    tx.execute("UPDATE games SET status = 'missing' WHERE source = ?1", [source])?;
    for g in found {
        let existing: Option<i64> = tx
            .query_row(
                "SELECT id FROM games WHERE source = ?1 AND source_id = ?2",
                [g.source, &g.source_id],
                |r| r.get(0),
            )
            .optional()?;
        match existing {
            Some(id) => {
                tx.execute(
                    "UPDATE games SET status = 'ok', install_dir = ?2 WHERE id = ?1",
                    params![id, g.install_dir],
                )?;
            }
            None => {
                tx.execute(
                    "INSERT INTO games (title, sort_title, source, source_id, install_dir, added_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![g.title, names::sort_title(&g.title), g.source, g.source_id, g.install_dir, db::now()],
                )?;
                let id = tx.last_insert_rowid();
                insert_action(&tx, id, "Jouer", &g.launch_uri, None, true, 0)?;
                inserted.push(id);
            }
        }
    }
    tx.commit()?;
    Ok(inserted)
}

/// Manual games whose default executable disappeared (unplugged drive, deleted folder) become 'missing'.
pub fn refresh_manual_status(conn: &Connection) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(
        "SELECT g.id, a.target FROM games g
         JOIN launch_actions a ON a.game_id = g.id AND a.is_default = 1
         WHERE g.source = 'manual'",
    )?;
    let rows: Vec<(i64, String)> =
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
    for (id, target) in rows {
        let status = if Path::new(&target).exists() { "ok" } else { "missing" };
        conn.execute("UPDATE games SET status = ?2 WHERE id = ?1", params![id, status])?;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GamePatch {
    pub title: Option<String>,
    pub favorite: Option<bool>,
    pub hidden: Option<bool>,
}

pub fn update_game(conn: &Connection, id: i64, patch: &GamePatch) -> rusqlite::Result<()> {
    if let Some(title) = &patch.title {
        conn.execute(
            "UPDATE games SET title = ?2, sort_title = ?3 WHERE id = ?1",
            params![id, title, names::sort_title(title)],
        )?;
    }
    if let Some(favorite) = patch.favorite {
        conn.execute("UPDATE games SET favorite = ?2 WHERE id = ?1", params![id, favorite])?;
    }
    if let Some(hidden) = patch.hidden {
        conn.execute("UPDATE games SET hidden = ?2 WHERE id = ?1", params![id, hidden])?;
    }
    Ok(())
}

pub fn update_action(conn: &Connection, action: &LaunchAction) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE launch_actions SET name = ?2, target = ?3, args = ?4, working_dir = ?5, run_as_admin = ?6
         WHERE id = ?1",
        params![action.id, action.name, action.target, action.args, action.working_dir, action.run_as_admin],
    )?;
    Ok(())
}

/// Removes a game from the library. Manual games are remembered as ignored so rescans don't offer them again.
pub fn remove_game(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    let row: Option<(String, String)> = conn
        .query_row("SELECT source, source_id FROM games WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    if let Some((source, key)) = row {
        if source == "manual" {
            ignore_path(conn, &key)?;
        }
    }
    conn.execute("DELETE FROM games WHERE id = ?1", [id])?;
    Ok(())
}

pub fn set_sgdb_id(conn: &Connection, id: i64, sgdb_id: i64) -> rusqlite::Result<()> {
    conn.execute("UPDATE games SET sgdb_id = ?2 WHERE id = ?1", params![id, sgdb_id])?;
    Ok(())
}

pub fn set_image(conn: &Connection, game_id: i64, kind: &str, file: &str, origin: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO game_images (game_id, kind, file, origin) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (game_id, kind) DO UPDATE SET file = excluded.file, origin = excluded.origin",
        params![game_id, kind, file, origin],
    )?;
    Ok(())
}

/// Games that have no cover yet.
pub fn games_without_artwork(conn: &Connection) -> rusqlite::Result<Vec<i64>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM games WHERE id NOT IN (SELECT game_id FROM game_images WHERE kind = 'grid')",
    )?;
    let ids = stmt.query_map([], |r| r.get(0))?.collect();
    ids
}

// ── Watched folders & ignored paths ─────────────────────────────────────────

/// Folders offered on first launch.
const DEFAULT_WATCHED_FOLDERS: &[&str] = &[r"H:\Games", r"N:\"];

pub fn seed_watched_folders(conn: &Connection) -> rusqlite::Result<()> {
    if db::get_setting(conn, "watched_folders_seeded")?.is_some() {
        return Ok(());
    }
    for path in DEFAULT_WATCHED_FOLDERS.iter().filter(|p| Path::new(p).is_dir()) {
        add_watched_folder(conn, path)?;
    }
    db::set_setting(conn, "watched_folders_seeded", "true")
}

pub fn watched_folders(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT path FROM watched_folders ORDER BY path")?;
    let paths = stmt.query_map([], |r| r.get(0))?.collect();
    paths
}

pub fn add_watched_folder(conn: &Connection, path: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO watched_folders (path, added_at) VALUES (?1, ?2)",
        params![path, db::now()],
    )?;
    Ok(())
}

pub fn remove_watched_folder(conn: &Connection, path: &str) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM watched_folders WHERE path = ?1", [path])?;
    Ok(())
}

pub fn ignore_path(conn: &Connection, path: &str) -> rusqlite::Result<()> {
    conn.execute("INSERT OR IGNORE INTO ignored_paths (path) VALUES (?1)", [manual_key(path)])?;
    Ok(())
}

/// Folder candidates from every watched folder, minus games already in the library and ignored folders.
pub fn scan_watched_folders(conn: &Connection) -> rusqlite::Result<Vec<folders::FolderCandidate>> {
    let known: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT source_id FROM games WHERE source = 'manual' UNION SELECT path FROM ignored_paths",
        )?;
        let known = stmt.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
        known
    };
    let roots = watched_folders(conn)?;
    Ok(roots
        .iter()
        .flat_map(|root| folders::scan_root(Path::new(root)))
        .filter(|c| !known.contains(&manual_key(&c.dir)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        db::open_in_memory().unwrap()
    }

    #[test]
    fn manual_game_round_trip() {
        let mut conn = conn();
        let id = add_manual_game(
            &mut conn,
            &NewManualGame {
                dir: r"N:\Limbo".into(),
                title: "Limbo".into(),
                exe: r"N:\Limbo\limbo.exe".into(),
                args: String::new(),
                extras: vec![folders::ExtraAction { name: "Configuration".into(), path: r"N:\Limbo\cfg.exe".into() }],
            },
        )
        .unwrap();
        let detail = get_game(&conn, id).unwrap().unwrap();
        assert_eq!(detail.game.source, "manual");
        assert_eq!(detail.actions.len(), 2);
        assert!(detail.actions[0].is_default);
        assert_eq!(detail.actions[0].working_dir.as_deref(), Some(r"N:\Limbo"));

        remove_game(&conn, id).unwrap();
        assert!(get_game(&conn, id).unwrap().is_none());
        let ignored: String = conn.query_row("SELECT path FROM ignored_paths", [], |r| r.get(0)).unwrap();
        assert_eq!(ignored, r"n:\limbo");
    }

    #[test]
    fn platform_sync_marks_missing_games() {
        let mut conn = conn();
        let game = |id: &str| PlatformGame {
            source: "steam",
            source_id: id.into(),
            title: format!("Game {id}"),
            install_dir: None,
            launch_uri: format!("steam://rungameid/{id}"),
        };
        assert_eq!(sync_platform_games(&mut conn, "steam", &[game("1"), game("2")]).unwrap().len(), 2);
        assert!(sync_platform_games(&mut conn, "steam", &[game("1")]).unwrap().is_empty());
        let statuses: Vec<String> = conn
            .prepare("SELECT status FROM games ORDER BY source_id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(statuses, ["ok", "missing"]);
    }
}
