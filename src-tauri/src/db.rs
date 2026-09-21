//! SQLite connection and versioned migrations (tracked with `PRAGMA user_version`).

use rusqlite::{Connection, OptionalExtension};
use std::path::Path;

/// Migrations run in order; index + 1 is the schema version. Never edit a released entry — append.
const MIGRATIONS: &[&str] = &[
    // 1 — settings
    "CREATE TABLE settings (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );",
    // 2 — library
    "CREATE TABLE games (
        id             INTEGER PRIMARY KEY,
        title          TEXT    NOT NULL,
        sort_title     TEXT    NOT NULL,
        source         TEXT    NOT NULL,              -- 'manual' | 'steam' | 'epic'
        source_id      TEXT    NOT NULL,              -- appid, Epic app name, or install folder for manual games
        install_dir    TEXT,
        status         TEXT    NOT NULL DEFAULT 'ok', -- 'ok' | 'missing'
        favorite       INTEGER NOT NULL DEFAULT 0,
        hidden         INTEGER NOT NULL DEFAULT 0,
        sgdb_id        INTEGER,
        added_at       INTEGER NOT NULL,
        last_played_at INTEGER,
        UNIQUE (source, source_id)
    );
    CREATE TABLE game_images (
        game_id INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
        kind    TEXT    NOT NULL,                     -- 'grid' | 'hero' | 'logo' | 'icon'
        file    TEXT    NOT NULL,                     -- relative to the cache folder
        origin  TEXT    NOT NULL,                     -- 'steamgriddb' | 'custom'
        PRIMARY KEY (game_id, kind)
    );
    CREATE TABLE launch_actions (
        id           INTEGER PRIMARY KEY,
        game_id      INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
        name         TEXT    NOT NULL,
        target       TEXT    NOT NULL,                -- exe path or URI (steam://…)
        args         TEXT    NOT NULL DEFAULT '',
        working_dir  TEXT,
        run_as_admin INTEGER NOT NULL DEFAULT 0,
        is_default   INTEGER NOT NULL DEFAULT 0,
        position     INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE watched_folders (
        path     TEXT PRIMARY KEY,
        added_at INTEGER NOT NULL
    );
    CREATE TABLE ignored_paths (
        path TEXT PRIMARY KEY                         -- lowercase folder or exe the user declined
    );",
];

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let mut conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&mut conn)?;
    Ok(conn)
}

#[cfg(test)]
pub fn open_in_memory() -> rusqlite::Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&mut conn)?;
    Ok(conn)
}

fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    let current: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i as u32 + 1)?;
        tx.commit()?;
    }
    Ok(())
}

pub fn schema_version(conn: &Connection) -> rusqlite::Result<u32> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
}

pub fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| row.get(0))
        .optional()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

/// Seconds since the Unix epoch.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_apply_and_are_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        migrate(&mut conn).unwrap();
        assert_eq!(schema_version(&conn).unwrap() as usize, MIGRATIONS.len());
        conn.execute("INSERT INTO settings (key, value) VALUES ('k', '\"v\"')", [])
            .unwrap();
    }
}
