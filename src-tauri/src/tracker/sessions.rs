//! Session bookkeeping, independent of how processes are discovered.

use std::collections::{HashMap, HashSet};

/// Sessions shorter than this are dropped (menus opened by mistake, crashes at launch).
pub const MIN_SESSION_SECS: i64 = 60;

#[derive(Debug, PartialEq)]
pub enum Event {
    Started { game_id: i64 },
    /// The game is still running: refresh the heartbeat.
    Seen { game_id: i64 },
    /// `main_exe`: the executable that ran the longest, a candidate to learn.
    Stopped { game_id: i64, main_exe: Option<String>, main_exe_secs: i64 },
}

#[derive(Default)]
pub struct Sessions {
    /// Running game → seconds seen per executable during this session.
    active: HashMap<i64, HashMap<String, i64>>,
}

impl Sessions {
    pub fn running(&self) -> HashSet<i64> {
        self.active.keys().copied().collect()
    }

    /// Marks a game as already running (a session resumed after Hangar restarted).
    pub fn resume(&mut self, game_id: i64) {
        self.active.entry(game_id).or_default();
    }

    /// One tracker tick: `seen` maps each running game to the executables found for it.
    pub fn tick(&mut self, seen: &HashMap<i64, Vec<String>>, elapsed_secs: i64) -> Vec<Event> {
        let mut events = Vec::new();
        for (&game_id, exes) in seen {
            let entry = self.active.entry(game_id);
            let started = matches!(entry, std::collections::hash_map::Entry::Vacant(_));
            let per_exe = entry.or_default();
            for exe in exes {
                *per_exe.entry(exe.clone()).or_default() += elapsed_secs;
            }
            events.push(if started { Event::Started { game_id } } else { Event::Seen { game_id } });
        }
        let stopped: Vec<i64> = self.active.keys().filter(|id| !seen.contains_key(id)).copied().collect();
        for game_id in stopped {
            let per_exe = self.active.remove(&game_id).unwrap_or_default();
            let main = per_exe.into_iter().max_by_key(|(_, secs)| *secs);
            events.push(Event::Stopped {
                game_id,
                main_exe_secs: main.as_ref().map_or(0, |(_, s)| *s),
                main_exe: main.map(|(exe, _)| exe),
            });
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(entries: &[(i64, &[&str])]) -> HashMap<i64, Vec<String>> {
        entries.iter().map(|(id, exes)| (*id, exes.iter().map(|e| e.to_string()).collect())).collect()
    }

    #[test]
    fn start_continue_stop_learns_longest_exe() {
        let mut s = Sessions::default();
        assert_eq!(s.tick(&seen(&[(7, &["launcher.exe"])]), 5), vec![Event::Started { game_id: 7 }]);
        s.tick(&seen(&[(7, &["launcher.exe", "game.exe"])]), 5);
        for _ in 0..20 {
            assert_eq!(s.tick(&seen(&[(7, &["game.exe"])]), 5), vec![Event::Seen { game_id: 7 }]);
        }
        assert_eq!(
            s.tick(&seen(&[]), 5),
            vec![Event::Stopped { game_id: 7, main_exe: Some("game.exe".into()), main_exe_secs: 105 }]
        );
        assert!(s.running().is_empty());
    }

    #[test]
    fn resumed_session_is_not_restarted() {
        let mut s = Sessions::default();
        s.resume(3);
        assert_eq!(s.tick(&seen(&[(3, &["x.exe"])]), 5), vec![Event::Seen { game_id: 3 }]);
    }
}
