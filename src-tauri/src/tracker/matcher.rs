//! Which running process belongs to which game.
//!
//! A process belongs to a game when its executable lives in the game's folder. Elevated processes
//! (a game started as administrator) hide their path from a non-elevated Hangar, so the file name
//! is compared too. Once Hangar has learned a game's real executable, only that one counts —
//! launchers and helpers are then ignored.

use std::collections::HashSet;

/// What the tracker knows about one game.
#[derive(Debug, Clone)]
pub struct WatchedGame {
    pub id: i64,
    /// Lowercase, with a trailing backslash.
    pub install_dir: Option<String>,
    /// Lowercase file names learned or chosen for this game.
    pub tracked: HashSet<String>,
    /// Lowercase file name of the default launch action, for elevated processes.
    pub default_exe: Option<String>,
}

/// A running process: lowercase file name, and lowercase full path when readable.
#[derive(Debug, Clone)]
pub struct RunningProcess {
    pub name: String,
    pub path: Option<String>,
}

/// Helpers that sit in game folders but never mean the game is being played.
fn is_helper(name: &str) -> bool {
    const HELPERS: &[&str] = &["unins", "crash", "setup", "redist", "dxsetup", "uploader", "handler"];
    HELPERS.iter().any(|h| name.contains(h))
}

pub fn install_dir_key(dir: &str) -> String {
    let mut key = dir.replace('/', "\\").to_lowercase();
    if !key.ends_with('\\') {
        key.push('\\');
    }
    key
}

pub fn game_for(process: &RunningProcess, games: &[WatchedGame]) -> Option<i64> {
    games.iter().find(|g| matches(process, g)).map(|g| g.id)
}

fn matches(p: &RunningProcess, g: &WatchedGame) -> bool {
    let in_folder = match (&p.path, &g.install_dir) {
        (Some(path), Some(dir)) => Some(path.starts_with(dir.as_str())),
        _ => None, // path hidden (elevated process) or game without folder
    };
    if !g.tracked.is_empty() {
        return g.tracked.contains(&p.name) && in_folder != Some(false);
    }
    match in_folder {
        Some(inside) => inside && !is_helper(&p.name),
        None => g.default_exe.as_deref() == Some(p.name.as_str()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(tracked: &[&str]) -> WatchedGame {
        WatchedGame {
            id: 1,
            install_dir: Some(install_dir_key(r"N:\Red Dead Redemption 2")),
            tracked: tracked.iter().map(|s| s.to_string()).collect(),
            default_exe: Some("rdr2.exe".into()),
        }
    }

    fn proc(name: &str, path: Option<&str>) -> RunningProcess {
        RunningProcess { name: name.into(), path: path.map(|p| p.to_lowercase()) }
    }

    #[test]
    fn any_exe_in_the_folder_counts_until_learned() {
        let games = [game(&[])];
        assert_eq!(game_for(&proc("launcher.exe", Some(r"N:\Red Dead Redemption 2\Launcher.exe")), &games), Some(1));
        assert_eq!(game_for(&proc("unins000.exe", Some(r"N:\Red Dead Redemption 2\unins000.exe")), &games), None);
        assert_eq!(game_for(&proc("rdr2.exe", Some(r"N:\Red Dead Redemption 2 Mods\RDR2.exe")), &games), None);
    }

    #[test]
    fn learned_exe_ignores_launchers() {
        let games = [game(&["rdr2.exe"])];
        assert_eq!(game_for(&proc("launcher.exe", Some(r"N:\Red Dead Redemption 2\Launcher.exe")), &games), None);
        assert_eq!(game_for(&proc("rdr2.exe", Some(r"N:\Red Dead Redemption 2\RDR2.exe")), &games), Some(1));
    }

    #[test]
    fn elevated_process_matches_by_name() {
        assert_eq!(game_for(&proc("rdr2.exe", None), &[game(&[])]), Some(1));
        assert_eq!(game_for(&proc("rdr2.exe", None), &[game(&["rdr2.exe"])]), Some(1));
        assert_eq!(game_for(&proc("other.exe", None), &[game(&[])]), None);
    }
}
