//! Screenshot gallery: captures from the Xbox Game Bar, Windows (Impr. écran) and Steam, each filed
//! under the game it shows. The capture time is compared with play sessions first; Steam folders
//! carry their app id, and Game Bar file names ("Red Dead Redemption 2 10_06_2025 21_39_38.png")
//! are the fallback. Windows screenshots are only kept when taken during a session: that folder
//! also holds everything else the user ever captured.

use std::collections::HashSet;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rusqlite::Connection;
use serde::Serialize;

use crate::library::names;

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp"];
/// A capture a few seconds after the heartbeat that closed a session still belongs to it.
const SESSION_MARGIN_SECS: i64 = 30;
const THUMB_WIDTH: u32 = 480;
const THUMB_HEIGHT: u32 = 300;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Source {
    GameBar,
    Windows,
    Steam(u32),
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Shot {
    pub path: String,
    /// `None`: a Game Bar capture of something that isn't in the library.
    pub game_id: Option<i64>,
    pub taken_at: i64,
    pub source: &'static str,
}

struct Game {
    id: i64,
    compact_title: String,
    steam_app: Option<u32>,
}

struct Session {
    game_id: i64,
    start: i64,
    end: i64,
}

/// What captures are matched against, read from the database before the folders are scanned.
pub struct Known {
    games: Vec<Game>,
    sessions: Vec<Session>,
}

impl Known {
    pub fn load(conn: &Connection) -> rusqlite::Result<Known> {
        Ok(Known { games: games(conn)?, sessions: sessions(conn)? })
    }
}

/// Every capture found, newest first.
pub fn list(known: &Known) -> Vec<Shot> {
    let Known { games, sessions } = known;
    let mut shots: Vec<Shot> = folders()
        .into_iter()
        .flat_map(|(dir, source)| images_in(&dir).into_iter().map(move |(path, taken_at)| (path, taken_at, source)))
        .filter_map(|(path, taken_at, source)| {
            let game_id = assign(&path, taken_at, source, games, sessions);
            // A Windows screenshot outside any session is a desktop capture, not a game's.
            if game_id.is_none() && source != Source::GameBar {
                return None;
            }
            Some(Shot {
                path: path.display().to_string(),
                game_id,
                taken_at,
                source: match source {
                    Source::GameBar => "gamebar",
                    Source::Windows => "windows",
                    Source::Steam(_) => "steam",
                },
            })
        })
        .collect();
    shots.sort_by(|a, b| b.taken_at.cmp(&a.taken_at));
    shots
}

fn assign(path: &Path, taken_at: i64, source: Source, games: &[Game], sessions: &[Session]) -> Option<i64> {
    if let Source::Steam(app) = source {
        if let Some(game) = games.iter().find(|g| g.steam_app == Some(app)) {
            return Some(game.id);
        }
    }
    let during = sessions
        .iter()
        .filter(|s| s.start <= taken_at && taken_at <= s.end + SESSION_MARGIN_SECS)
        .max_by_key(|s| s.start);
    if let Some(session) = during {
        return Some(session.game_id);
    }
    if source == Source::GameBar {
        let stem = path.file_stem()?.to_string_lossy();
        return game_named(&game_bar_title(&stem), games);
    }
    None
}

/// "Red Dead Redemption 2 10_06_2025 21_39_38" → "Red Dead Redemption 2".
fn game_bar_title(stem: &str) -> String {
    let mut words: Vec<&str> = stem.split(' ').collect();
    // "10_06_2025", "21_39_38": digits joined by underscores, unlike a sequel number.
    let is_stamp = |w: &str| w.contains('_') && w.chars().all(|c| c.is_ascii_digit() || c == '_');
    while words.last().is_some_and(|w| is_stamp(w)) {
        words.pop();
    }
    words.join(" ")
}

/// Window titles often add an edition or a version after the game's name: the longest library
/// title the window title starts with wins.
fn game_named(window_title: &str, games: &[Game]) -> Option<i64> {
    let window = names::compact(window_title);
    games
        .iter()
        .filter(|g| g.compact_title.len() >= 3 && window.starts_with(&g.compact_title))
        .max_by_key(|g| g.compact_title.len())
        .map(|g| g.id)
}

fn games(conn: &Connection) -> rusqlite::Result<Vec<Game>> {
    let mut stmt = conn.prepare("SELECT id, title, source, source_id FROM games")?;
    let rows = stmt.query_map([], |r| {
        let source: String = r.get(2)?;
        let source_id: Option<String> = r.get(3)?;
        Ok(Game {
            id: r.get(0)?,
            compact_title: names::compact(&r.get::<_, String>(1)?),
            steam_app: (source == "steam").then(|| source_id.and_then(|s| s.parse().ok())).flatten(),
        })
    })?;
    rows.collect()
}

fn sessions(conn: &Connection) -> rusqlite::Result<Vec<Session>> {
    let mut stmt = conn.prepare("SELECT game_id, started_at, COALESCE(ended_at, last_seen_at) FROM sessions")?;
    let rows = stmt.query_map([], |r| Ok(Session { game_id: r.get(0)?, start: r.get(1)?, end: r.get(2)? }))?;
    rows.collect()
}

fn folders() -> Vec<(PathBuf, Source)> {
    let mut folders = Vec::new();
    if let Some(videos) = os::known_folder(os::Folder::Videos) {
        folders.push((videos.join("Captures"), Source::GameBar));
    }
    if let Some(screenshots) = os::known_folder(os::Folder::Screenshots) {
        folders.push((screenshots, Source::Windows));
    }
    if let Some(steam) = crate::scanners::steam::steam_path() {
        for user in read_dirs(&steam.join("userdata")) {
            for app in read_dirs(&user.join("760").join("remote")) {
                if let Some(id) = app.file_name().and_then(|n| n.to_str()).and_then(|n| n.parse().ok()) {
                    folders.push((app.join("screenshots"), Source::Steam(id)));
                }
            }
        }
    }
    folders
}

fn read_dirs(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default()
}

fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

fn images_in(dir: &Path) -> Vec<(PathBuf, i64)> {
    let Ok(entries) = fs::read_dir(dir) else { return vec![] };
    entries
        .flatten()
        .filter(|e| is_image(&e.path()))
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?;
            Some((e.path(), modified.duration_since(UNIX_EPOCH).ok()?.as_secs() as i64))
        })
        .collect()
}

/// Files the UI may load through the `hangar` protocol: only those a listing returned.
#[derive(Default)]
pub struct Allowed(std::sync::Mutex<HashSet<PathBuf>>);

impl Allowed {
    pub fn replace(&self, shots: &[Shot]) {
        if let Ok(mut set) = self.0.lock() {
            *set = shots.iter().map(|s| PathBuf::from(&s.path)).collect();
        }
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.0.lock().is_ok_and(|set| set.contains(path))
    }

    pub fn remove(&self, path: &Path) {
        if let Ok(mut set) = self.0.lock() {
            set.remove(path);
        }
    }
}

/// Small JPEG copy for the grid, cached in `thumbs` and rebuilt when the capture changes.
pub fn thumbnail(thumbs: &Path, path: &Path) -> Result<Vec<u8>, String> {
    let modified = fs::metadata(path).and_then(|m| m.modified()).map_err(|e| e.to_string())?;
    let mut hasher = DefaultHasher::new();
    (path, modified).hash(&mut hasher);
    let cached = thumbs.join(format!("{:016x}.jpg", hasher.finish()));
    if let Ok(bytes) = fs::read(&cached) {
        return Ok(bytes);
    }

    let image = image::open(path).map_err(|e| e.to_string())?;
    let small = image.thumbnail(THUMB_WIDTH, THUMB_HEIGHT).to_rgb8();
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 82)
        .encode_image(&small)
        .map_err(|e| e.to_string())?;
    let _ = fs::create_dir_all(thumbs).and_then(|_| fs::write(&cached, &bytes));
    Ok(bytes)
}

#[cfg(windows)]
mod os {
    use std::path::PathBuf;
    use windows_sys::Win32::System::Com::CoTaskMemFree;
    use windows_sys::Win32::UI::Shell::{FOLDERID_Screenshots, FOLDERID_Videos, SHGetKnownFolderPath};

    pub enum Folder {
        Videos,
        Screenshots,
    }

    /// Follows the user's own location for these folders (moved to another drive, OneDrive…).
    pub fn known_folder(folder: Folder) -> Option<PathBuf> {
        let id = match folder {
            Folder::Videos => &FOLDERID_Videos,
            Folder::Screenshots => &FOLDERID_Screenshots,
        };
        unsafe {
            let mut raw = std::ptr::null_mut();
            let path = (SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut raw) == 0).then(|| {
                let len = (0..).take_while(|&i| *raw.add(i) != 0).count();
                PathBuf::from(String::from_utf16_lossy(std::slice::from_raw_parts(raw, len)))
            });
            CoTaskMemFree(raw as _);
            path
        }
    }
}

#[cfg(not(windows))]
mod os {
    pub enum Folder {
        Videos,
        Screenshots,
    }
    pub fn known_folder(_folder: Folder) -> Option<std::path::PathBuf> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(id: i64, title: &str) -> Game {
        Game { id, compact_title: names::compact(title), steam_app: None }
    }

    #[test]
    fn game_bar_names() {
        assert_eq!(game_bar_title("Red Dead Redemption 2 10_06_2025 21_39_38"), "Red Dead Redemption 2");
        let games = [game(1, "Horizon Forbidden West"), game(2, "Call of Duty"), game(3, "Horizon Zero Dawn")];
        let title = game_bar_title("Horizon Forbidden West™ Complete Edition v1.0.38.0 01_02_2025 10_00_00");
        assert_eq!(game_named(&title, &games), Some(1));
        // Zero-width spaces and ® from the Game Bar are ignored.
        assert_eq!(game_named("C\u{200b}a\u{200b}l\u{200b}l of Duty® HQ", &games), Some(2));
        assert_eq!(game_named("Pour Toi — Mozilla Firefox", &games), None);
    }

    #[test]
    fn session_time_wins_over_the_name() {
        let games = [game(1, "Limbo"), game(2, "Steam")];
        let sessions = [Session { game_id: 1, start: 1_000, end: 2_000 }];
        let shot = Path::new("Steam 01_01_2025 10_00_00.png");
        assert_eq!(assign(shot, 1_500, Source::GameBar, &games, &sessions), Some(1));
        assert_eq!(assign(shot, 5_000, Source::GameBar, &games, &sessions), Some(2));
        assert_eq!(assign(Path::new("Capture.png"), 5_000, Source::Windows, &games, &sessions), None);
    }
}
