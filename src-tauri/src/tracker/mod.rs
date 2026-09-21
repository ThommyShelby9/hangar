//! Play-time tracking: a background thread polls running processes every few seconds and records
//! sessions for every game in the library, whether it was started from Hangar or not.

mod matcher;
mod sessions;
pub mod stats;

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use rusqlite::{params, Connection};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
use tauri::{AppHandle, Emitter, Manager};

use crate::{commands, db, hardware, pads, perf, saves, AppState};
use matcher::{game_for, install_dir_key, RunningProcess, WatchedGame};
use sessions::{Event, Sessions, MIN_SESSION_SECS};

/// Emitted when a game starts or stops; the UI refreshes "running" badges and stats.
pub const SESSIONS_CHANGED: &str = "sessions-changed";
/// Emitted with a `stats::SessionEnd` when a game closes after a session worth keeping.
pub const SESSION_ENDED: &str = "session-ended";
const TICK_SECS: i64 = 5;
/// An open session whose heartbeat is older than this was interrupted (crash, shutdown).
const RESUME_WINDOW_SECS: i64 = 120;

pub fn spawn(app: AppHandle) {
    std::thread::Builder::new()
        .name("hangar-tracker".into())
        .spawn(move || run(app))
        .expect("impossible de démarrer le suivi du temps de jeu");
}

fn with_db<T>(app: &AppHandle, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Option<T> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().ok()?;
    f(&conn).map_err(|e| eprintln!("Hangar : suivi du temps de jeu : {e}")).ok()
}

fn run(app: AppHandle) {
    let own_pid = sysinfo::get_current_pid().ok();
    let mut system = System::new();
    let mut sessions = Sessions::default();
    let mut resumable = with_db(&app, |c| recover(c, db::now())).unwrap_or_default();
    let mut perf_mode = perf::PerfMode::default();
    // Left over by a crash or a forced exit; re-applied below if a game still runs.
    with_db(&app, perf::restore_power_mode);

    loop {
        let games = with_db(&app, watched_games).unwrap_or_default();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
        );

        let mut seen: HashMap<i64, Vec<String>> = HashMap::new();
        let mut game_pids = Vec::new();
        for (pid, process) in system.processes() {
            if Some(*pid) == own_pid {
                continue;
            }
            let running = RunningProcess {
                name: process.name().to_string_lossy().to_lowercase(),
                path: process.exe().map(|p| p.display().to_string().to_lowercase()),
            };
            if let Some(game_id) = game_for(&running, &games) {
                game_pids.push(pid.as_u32());
                let exes = seen.entry(game_id).or_default();
                if !exes.contains(&running.name) {
                    exes.push(running.name);
                }
            }
        }

        // First tick after start-up: continue sessions of games still running, close the others.
        for game_id in resumable.drain() {
            if seen.contains_key(&game_id) {
                sessions.resume(game_id);
            } else {
                with_db(&app, |c| close_session(c, game_id));
            }
        }

        let events = sessions.tick(&seen, TICK_SECS);
        let now = db::now();
        let mut changed = false;
        let mut ended = Vec::new();
        with_db(&app, |c| {
            for event in &events {
                match event {
                    Event::Started { game_id } => {
                        open_session(c, *game_id, now)?;
                        changed = true;
                    }
                    Event::Seen { game_id } => heartbeat(c, *game_id, now)?,
                    Event::Stopped { game_id, main_exe, main_exe_secs } => {
                        close_session(c, *game_id)?;
                        if let Some(exe) = main_exe.as_deref().filter(|_| *main_exe_secs >= MIN_SESSION_SECS) {
                            learn(c, *game_id, exe)?;
                        }
                        // Too-short sessions were dropped by close_session: nothing to sum up.
                        ended.extend(stats::last_session_summary(c, *game_id, now - TICK_SECS * 3)?);
                        changed = true;
                    }
                }
            }
            Ok(())
        });

        with_db(&app, |c| perf_mode.update(c, &game_pids));

        // Hardware readings while something runs: kept with the session for its summary.
        if !sessions.running().is_empty() {
            let state = app.state::<AppState>();
            let sample = state.monitor.lock().ok().map(|mut monitor| {
                monitor.get_or_insert_with(|| hardware::Monitor::new(&state.paths.data)).session_sample()
            });
            if let Some(sample) = sample {
                with_db(&app, |c| record_sample(c, &sample));
            }
        }
        for summary in ended {
            backup_after_session(&app, summary.game_id, summary.title.clone());
            let _ = app.emit(SESSION_ENDED, summary);
        }

        // DS4Windows follows the games: the profile of the one that starts, the usual ones when all close.
        for event in &events {
            if let Event::Started { game_id } = event {
                let (app, game_id) = (app.clone(), *game_id);
                std::thread::spawn(move || pads::game_started(&app.state::<AppState>().db, game_id));
            }
        }
        if changed && sessions.running().is_empty() {
            std::thread::spawn(pads::games_stopped);
        }

        if changed {
            if let Ok(mut running) = app.state::<AppState>().running.lock() {
                *running = sessions.running();
            }
            let _ = app.emit(SESSIONS_CHANGED, ());
            let _ = app.emit(commands::LIBRARY_CHANGED, ());
        }
        std::thread::sleep(Duration::from_secs(TICK_SECS as u64));
    }
}

/// The game just closed: its saves are fresh, back them up (in the background, Ludusavi takes a moment).
fn backup_after_session(app: &AppHandle, game_id: i64, title: String) {
    let state = app.state::<AppState>();
    let enabled = with_db(app, |c| saves::settings(c, &state.paths)).is_some_and(|s| s.auto);
    if !enabled || !saves::Ludusavi::new(&state.paths).installed() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let error = saves::backup(&state.paths, &state.db, game_id).err();
        let _ = app.emit(commands::SAVES_BACKED_UP, commands::SavesBackedUp { game_id, title, error });
    });
}

fn watched_games(conn: &Connection) -> rusqlite::Result<Vec<WatchedGame>> {
    let mut tracked: HashMap<i64, HashSet<String>> = HashMap::new();
    let mut stmt = conn.prepare("SELECT game_id, exe_name FROM tracked_processes")?;
    for row in stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))? {
        let (id, name) = row?;
        tracked.entry(id).or_default().insert(name);
    }

    let mut stmt = conn.prepare(
        "SELECT g.id, g.install_dir, a.target FROM games g
         LEFT JOIN launch_actions a ON a.game_id = g.id AND a.is_default = 1
         WHERE g.status = 'ok'",
    )?;
    let games = stmt
        .query_map([], |r| {
            let id: i64 = r.get(0)?;
            let install_dir: Option<String> = r.get(1)?;
            let target: Option<String> = r.get(2)?;
            let default_exe = target
                .filter(|t| !t.contains("://"))
                .and_then(|t| t.rsplit(['\\', '/']).next().map(str::to_lowercase));
            Ok(WatchedGame {
                id,
                install_dir: install_dir.as_deref().map(install_dir_key),
                tracked: tracked.remove(&id).unwrap_or_default(),
                default_exe,
            })
        })?
        .collect();
    games
}

fn open_session(conn: &Connection, game_id: i64, now: i64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO sessions (game_id, started_at, last_seen_at) VALUES (?1, ?2, ?2)",
        params![game_id, now],
    )?;
    Ok(())
}

/// Folds one hardware reading into every open session: running maxima and running average.
fn record_sample(conn: &Connection, sample: &hardware::SessionSample) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sessions SET
            max_cpu_temp = CASE WHEN ?1 IS NULL THEN max_cpu_temp ELSE MAX(COALESCE(max_cpu_temp, ?1), ?1) END,
            max_gpu_temp = CASE WHEN ?2 IS NULL THEN max_gpu_temp ELSE MAX(COALESCE(max_gpu_temp, ?2), ?2) END,
            avg_cpu_load = (COALESCE(avg_cpu_load, 0) * load_samples + ?3) / (load_samples + 1),
            load_samples = load_samples + 1
         WHERE ended_at IS NULL",
        params![sample.cpu_temp, sample.gpu_temp, sample.cpu_load],
    )?;
    Ok(())
}

fn heartbeat(conn: &Connection, game_id: i64, now: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sessions SET last_seen_at = ?2 WHERE game_id = ?1 AND ended_at IS NULL",
        params![game_id, now],
    )?;
    Ok(())
}

/// Ends the open session at its last heartbeat; too-short sessions are dropped.
fn close_session(conn: &Connection, game_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sessions SET ended_at = last_seen_at WHERE game_id = ?1 AND ended_at IS NULL",
        [game_id],
    )?;
    conn.execute(
        "DELETE FROM sessions WHERE game_id = ?1 AND ended_at - started_at < ?2",
        params![game_id, MIN_SESSION_SECS],
    )?;
    // "Last played" only moves when a session was kept: a launch that failed or lasted seconds doesn't count.
    conn.execute(
        "UPDATE games SET last_played_at = (SELECT MAX(ended_at) FROM sessions WHERE game_id = ?1) WHERE id = ?1",
        [game_id],
    )?;
    Ok(())
}

/// Remembers the game's real executable, unless one is already known.
fn learn(conn: &Connection, game_id: i64, exe: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO tracked_processes (game_id, exe_name)
         SELECT ?1, ?2 WHERE NOT EXISTS (SELECT 1 FROM tracked_processes WHERE game_id = ?1)",
        params![game_id, exe],
    )?;
    Ok(())
}

/// Start-up: sessions left open by a crash or a shutdown. Recent ones may be resumed if the game is
/// still running; stale ones are closed at their last heartbeat.
fn recover(conn: &Connection, now: i64) -> rusqlite::Result<HashSet<i64>> {
    let mut stmt = conn.prepare("SELECT game_id, last_seen_at FROM sessions WHERE ended_at IS NULL")?;
    let open: Vec<(i64, i64)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_, _>>()?;
    let mut resumable = HashSet::new();
    for (game_id, last_seen) in open {
        if now - last_seen <= RESUME_WINDOW_SECS {
            resumable.insert(game_id);
        } else {
            close_session(conn, game_id)?;
        }
    }
    Ok(resumable)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn_with_game() -> Connection {
        let conn = db::open_in_memory().unwrap();
        conn.execute(
            "INSERT INTO games (id, title, sort_title, source, source_id, added_at) VALUES (1, 'G', 'g', 'manual', 'g', 0)",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn crashed_session_is_closed_at_last_heartbeat() {
        let conn = conn_with_game();
        open_session(&conn, 1, 1_000).unwrap();
        heartbeat(&conn, 1, 1_600).unwrap();
        assert!(recover(&conn, 10_000).unwrap().is_empty());
        let (ended, dur): (i64, i64) = conn
            .query_row("SELECT ended_at, ended_at - started_at FROM sessions", [], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!((ended, dur), (1_600, 600));
    }

    #[test]
    fn recent_session_can_be_resumed_and_short_ones_are_dropped() {
        let conn = conn_with_game();
        open_session(&conn, 1, 1_000).unwrap();
        heartbeat(&conn, 1, 1_030).unwrap();
        assert_eq!(recover(&conn, 1_060).unwrap(), HashSet::from([1]));
        close_session(&conn, 1).unwrap();
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 0, "30 s de jeu ne font pas une session");
    }

    /// End-to-end check on a throw-away portable copy of Hangar:
    /// `HANGAR_TEST_DB=…\data\hangar.db cargo test seed_test_instance -- --ignored`.
    #[test]
    #[ignore]
    fn seed_test_instance() {
        let path = std::env::var("HANGAR_TEST_DB").expect("HANGAR_TEST_DB");
        let mut conn = db::open(std::path::Path::new(&path)).unwrap();
        crate::library::add_manual_game(
            &mut conn,
            &crate::library::NewManualGame {
                dir: r"O:\tmp\hangar\FakeGame".into(),
                title: "Fake Game".into(),
                exe: r"O:\tmp\hangar\FakeGame\FakeGame.exe".into(),
                args: String::new(),
                extras: vec![],
            },
        )
        .unwrap();
    }

    #[test]
    #[ignore]
    fn print_test_instance() {
        let path = std::env::var("HANGAR_TEST_DB").expect("HANGAR_TEST_DB");
        let conn = Connection::open(path).unwrap();
        let mut stmt = conn
            .prepare("SELECT game_id, started_at, last_seen_at, ended_at FROM sessions")
            .unwrap();
        for row in stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, Option<i64>>(3)?)))
            .unwrap()
        {
            let (game, start, seen, end) = row.unwrap();
            println!("session jeu {game} : {} s, terminée : {}", seen - start, end.is_some());
        }
        let learned: Vec<String> = conn
            .prepare("SELECT exe_name FROM tracked_processes")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        println!("exe appris : {learned:?}");
    }

    #[test]
    fn last_played_follows_kept_sessions_only() {
        let conn = conn_with_game();
        let last_played = |c: &Connection| -> Option<i64> {
            c.query_row("SELECT last_played_at FROM games WHERE id = 1", [], |r| r.get(0)).unwrap()
        };
        open_session(&conn, 1, 1_000).unwrap();
        heartbeat(&conn, 1, 1_020).unwrap();
        close_session(&conn, 1).unwrap();
        assert_eq!(last_played(&conn), None, "20 s ne comptent pas comme une partie");

        open_session(&conn, 1, 2_000).unwrap();
        heartbeat(&conn, 1, 2_900).unwrap();
        close_session(&conn, 1).unwrap();
        assert_eq!(last_played(&conn), Some(2_900));
    }

    #[test]
    fn learns_only_the_first_exe() {
        let conn = conn_with_game();
        learn(&conn, 1, "game.exe").unwrap();
        learn(&conn, 1, "other.exe").unwrap();
        let names: Vec<String> = conn
            .prepare("SELECT exe_name FROM tracked_processes")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(names, ["game.exe"]);
    }
}
