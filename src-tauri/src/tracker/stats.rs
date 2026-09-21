//! Play-time statistics computed from `sessions`.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

/// Length of a session, open ones counted up to their last heartbeat.
const DURATION: &str = "(COALESCE(ended_at, last_seen_at) - started_at)";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStats {
    pub total_secs: i64,
    pub session_count: i64,
    pub last_session: Option<SessionSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub started_at: i64,
    pub duration_secs: i64,
}

pub fn game_stats(conn: &Connection, game_id: i64) -> rusqlite::Result<GameStats> {
    let (total_secs, session_count) = conn.query_row(
        &format!("SELECT COALESCE(SUM({DURATION}), 0), COUNT(*) FROM sessions WHERE game_id = ?1"),
        [game_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let last_session = conn
        .query_row(
            &format!(
                "SELECT started_at, {DURATION} FROM sessions WHERE game_id = ?1 ORDER BY started_at DESC LIMIT 1"
            ),
            [game_id],
            |r| Ok(SessionSummary { started_at: r.get(0)?, duration_secs: r.get(1)? }),
        )
        .optional()?;
    Ok(GameStats { total_secs, session_count, last_session })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayTotal {
    /// Local date, "YYYY-MM-DD".
    pub day: String,
    pub secs: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameTotal {
    pub game_id: i64,
    pub title: String,
    pub secs: i64,
    pub sessions: i64,
    pub grid: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentSession {
    pub id: i64,
    pub game_id: i64,
    pub title: String,
    pub started_at: i64,
    pub duration_secs: i64,
    pub running: bool,
    pub max_cpu_temp: Option<f64>,
    pub max_gpu_temp: Option<f64>,
}

/// Sent when a game closes: what the session looked like.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEnd {
    pub game_id: i64,
    pub title: String,
    pub duration_secs: i64,
    pub max_cpu_temp: Option<f64>,
    pub max_gpu_temp: Option<f64>,
    pub avg_cpu_load: Option<f64>,
}

/// The game's session that ended after `since`, if it was kept (long enough).
pub fn last_session_summary(conn: &Connection, game_id: i64, since: i64) -> rusqlite::Result<Option<SessionEnd>> {
    conn.query_row(
        "SELECT g.id, g.title, s.ended_at - s.started_at, s.max_cpu_temp, s.max_gpu_temp, s.avg_cpu_load
         FROM sessions s JOIN games g ON g.id = s.game_id
         WHERE s.game_id = ?1 AND s.ended_at >= ?2
         ORDER BY s.ended_at DESC LIMIT 1",
        params![game_id, since],
        |r| {
            Ok(SessionEnd {
                game_id: r.get(0)?,
                title: r.get(1)?,
                duration_secs: r.get(2)?,
                max_cpu_temp: r.get(3)?,
                max_gpu_temp: r.get(4)?,
                avg_cpu_load: r.get(5)?,
            })
        },
    )
    .optional()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub total_secs: i64,
    pub session_count: i64,
    pub games_played: i64,
    pub daily: Vec<DayTotal>,
    pub top_games: Vec<GameTotal>,
    pub recent: Vec<RecentSession>,
}

/// Totals for sessions started since `since` (Unix seconds; 0 for all time).
pub fn overview(conn: &Connection, since: i64) -> rusqlite::Result<Overview> {
    let (total_secs, session_count, games_played) = conn.query_row(
        &format!(
            "SELECT COALESCE(SUM({DURATION}), 0), COUNT(*), COUNT(DISTINCT game_id)
             FROM sessions WHERE started_at >= ?1"
        ),
        [since],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;

    let daily = conn
        .prepare(&format!(
            "SELECT date(started_at, 'unixepoch', 'localtime') AS day, SUM({DURATION})
             FROM sessions WHERE started_at >= ?1 GROUP BY day ORDER BY day"
        ))?
        .query_map([since], |r| Ok(DayTotal { day: r.get(0)?, secs: r.get(1)? }))?
        .collect::<Result<_, _>>()?;

    let top_games = conn
        .prepare(&format!(
            "SELECT g.id, g.title, SUM({DURATION}) AS total, COUNT(*),
                    (SELECT file FROM game_images i WHERE i.game_id = g.id AND i.kind = 'grid')
             FROM sessions s JOIN games g ON g.id = s.game_id
             WHERE s.started_at >= ?1
             GROUP BY g.id ORDER BY total DESC LIMIT 8"
        ))?
        .query_map([since], |r| {
            Ok(GameTotal {
                game_id: r.get(0)?,
                title: r.get(1)?,
                secs: r.get(2)?,
                sessions: r.get(3)?,
                grid: r.get(4)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    let recent = conn
        .prepare(&format!(
            "SELECT s.id, g.id, g.title, s.started_at, {DURATION}, s.ended_at IS NULL, s.max_cpu_temp, s.max_gpu_temp
             FROM sessions s JOIN games g ON g.id = s.game_id
             WHERE s.started_at >= ?1
             ORDER BY s.started_at DESC LIMIT 20"
        ))?
        .query_map(params![since], |r| {
            Ok(RecentSession {
                id: r.get(0)?,
                game_id: r.get(1)?,
                title: r.get(2)?,
                started_at: r.get(3)?,
                duration_secs: r.get(4)?,
                running: r.get(5)?,
                max_cpu_temp: r.get(6)?,
                max_gpu_temp: r.get(7)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    Ok(Overview { total_secs, session_count, games_played, daily, top_games, recent })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    #[test]
    fn summary_of_a_finished_session() {
        let conn = db::open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO games (id, title, sort_title, source, source_id, added_at) VALUES (1, 'A', 'a', 'manual', 'a', 0);
             INSERT INTO sessions (game_id, started_at, last_seen_at, ended_at, max_cpu_temp, avg_cpu_load, load_samples)
                VALUES (1, 1000, 4600, 4600, 88.0, 42.5, 720);",
        )
        .unwrap();
        let end = last_session_summary(&conn, 1, 4500).unwrap().unwrap();
        assert_eq!((end.duration_secs, end.max_cpu_temp, end.max_gpu_temp), (3600, Some(88.0), None));
        assert!(last_session_summary(&conn, 1, 5000).unwrap().is_none(), "trop ancienne");
    }

    #[test]
    fn totals_per_game_and_period() {
        let conn = db::open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO games (id, title, sort_title, source, source_id, added_at) VALUES
                (1, 'A', 'a', 'manual', 'a', 0), (2, 'B', 'b', 'manual', 'b', 0);
             INSERT INTO sessions (game_id, started_at, last_seen_at, ended_at) VALUES
                (1, 1000, 4600, 4600), (1, 90000, 91800, 91800), (2, 95000, 95600, 95600);",
        )
        .unwrap();

        let a = game_stats(&conn, 1).unwrap();
        assert_eq!((a.total_secs, a.session_count), (3600 + 1800, 2));
        assert_eq!(a.last_session.unwrap().started_at, 90000);

        let all = overview(&conn, 0).unwrap();
        assert_eq!((all.total_secs, all.session_count, all.games_played), (6000, 3, 2));
        assert_eq!(all.top_games[0].title, "A");

        let recent = overview(&conn, 80000).unwrap();
        assert_eq!(recent.total_secs, 2400);
        assert_eq!(recent.recent[0].title, "B");
    }
}
