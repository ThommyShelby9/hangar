//! Disk space each game takes, measured in the background: walking a 100 GB folder takes a while,
//! so sizes are kept in the database and measured again once a week (updates change them).

use std::fs;
use std::path::Path;

use rusqlite::{params, Connection};

use crate::db;

const MAX_AGE_SECS: i64 = 7 * 86_400;

/// Games with a folder whose size is unknown or old.
pub fn stale(conn: &Connection) -> rusqlite::Result<Vec<(i64, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, install_dir FROM games
         WHERE install_dir IS NOT NULL AND status = 'ok' AND (size_checked_at IS NULL OR size_checked_at < ?1)",
    )?;
    let rows = stmt.query_map([db::now() - MAX_AGE_SECS], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

pub fn store(conn: &Connection, game_id: i64, bytes: u64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE games SET size_bytes = ?2, size_checked_at = ?3 WHERE id = ?1",
        params![game_id, bytes as i64, db::now()],
    )?;
    Ok(())
}

/// Total size of the files below `dir`. Links and junctions aren't followed: a junction to another
/// drive would count someone else's files.
pub fn folder_size(dir: &Path) -> u64 {
    let mut total = 0;
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let Ok(meta) = entry.path().symlink_metadata() else { continue };
            if meta.file_type().is_symlink() || is_reparse_point(&meta) {
                continue;
            }
            if meta.is_dir() {
                pending.push(entry.path());
            } else {
                total += meta.len();
            }
        }
    }
    total
}

#[cfg(windows)]
fn is_reparse_point(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_meta: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_nested_files() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("hangar-size-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("a").join("b")).unwrap();
        fs::write(dir.join("x.bin"), [0u8; 100]).unwrap();
        fs::write(dir.join("a").join("b").join("y.bin"), [0u8; 23]).unwrap();
        assert_eq!(folder_size(&dir), 123);
        fs::remove_dir_all(&dir).unwrap();
    }
}
