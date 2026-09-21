//! Performance mode while a game runs: Windows power mode at "best performance", the game's
//! processes above normal priority, and the PC kept awake (controller play sends no input Windows
//! notices). Undone when the last game closes; a power mode left changed by a crash or a forced
//! exit is put back at the next start-up.

use rusqlite::Connection;
use std::collections::HashSet;

use crate::db;

/// Settings, each on unless set to "false".
pub const POWER: &str = "perf_power";
pub const PRIORITY: &str = "perf_priority";
pub const AWAKE: &str = "perf_awake";
/// Power mode to put back, stored while Hangar has changed it.
const SAVED_POWER_MODE: &str = "perf_saved_power_mode";

#[derive(Clone, Copy)]
pub struct Options {
    pub power: bool,
    pub priority: bool,
    pub awake: bool,
}

pub fn options(conn: &Connection) -> rusqlite::Result<Options> {
    let on = |key| db::get_setting(conn, key).map(|v| v.as_deref() != Some("false"));
    Ok(Options { power: on(POWER)?, priority: on(PRIORITY)?, awake: on(AWAKE)? })
}

#[derive(Default)]
pub struct PerfMode {
    boosted: HashSet<u32>,
    awake: bool,
}

impl PerfMode {
    /// Called every tracker tick with the processes of the games running right now.
    pub fn update(&mut self, conn: &Connection, game_pids: &[u32]) -> rusqlite::Result<()> {
        let playing = !game_pids.is_empty();
        let opts = options(conn)?;

        if playing && opts.power {
            if db::get_setting(conn, SAVED_POWER_MODE)?.is_none() {
                if let Some(current) = os::power_mode().filter(|m| *m != os::BEST_PERFORMANCE) {
                    if os::set_power_mode(os::BEST_PERFORMANCE) {
                        db::set_setting(conn, SAVED_POWER_MODE, &format!("{current:032x}"))?;
                    }
                }
            }
        } else {
            restore_power_mode(conn)?;
        }

        self.boosted.retain(|pid| game_pids.contains(pid));
        if opts.priority {
            for &pid in game_pids {
                // Tried once per process: an elevated game refuses, and that's fine.
                if self.boosted.insert(pid) {
                    os::raise_priority(pid);
                }
            }
        }

        let awake = playing && opts.awake;
        if awake != self.awake {
            os::keep_awake(awake);
            self.awake = awake;
        }
        Ok(())
    }
}

/// Puts back the power mode Hangar changed, if any: end of play, option turned off, start-up, exit.
pub fn restore_power_mode(conn: &Connection) -> rusqlite::Result<()> {
    if let Some(saved) = db::get_setting(conn, SAVED_POWER_MODE)? {
        if let Ok(mode) = u128::from_str_radix(&saved, 16) {
            os::set_power_mode(mode);
        }
        conn.execute("DELETE FROM settings WHERE key = ?1", [SAVED_POWER_MODE])?;
    }
    Ok(())
}

#[cfg(windows)]
mod os {
    use windows_sys::core::GUID;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Power::{
        SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, SetPriorityClass, ABOVE_NORMAL_PRIORITY_CLASS, PROCESS_SET_INFORMATION,
    };

    /// Windows 11 power mode "Meilleures performances".
    pub const BEST_PERFORMANCE: u128 = 0xded574b5_45a0_4f42_8737_46345c09c238;

    // Power mode (the "overlay" over the power plan): exported by powrprof.dll, used by the
    // Settings app, but absent from the SDK import libraries.
    #[link(name = "powrprof", kind = "raw-dylib")]
    unsafe extern "system" {
        fn PowerGetEffectiveOverlayScheme(mode: *mut GUID) -> u32;
        fn PowerSetActiveOverlayScheme(mode: *const GUID) -> u32;
    }

    fn to_u128(g: &GUID) -> u128 {
        (u128::from(g.data1) << 96)
            | (u128::from(g.data2) << 80)
            | (u128::from(g.data3) << 64)
            | u128::from(u64::from_be_bytes(g.data4))
    }

    pub fn power_mode() -> Option<u128> {
        let mut mode = GUID::from_u128(0);
        (unsafe { PowerGetEffectiveOverlayScheme(&mut mode) } == 0).then(|| to_u128(&mode))
    }

    pub fn set_power_mode(mode: u128) -> bool {
        unsafe { PowerSetActiveOverlayScheme(&GUID::from_u128(mode)) == 0 }
    }

    pub fn raise_priority(pid: u32) {
        unsafe {
            let process = OpenProcess(PROCESS_SET_INFORMATION, 0, pid);
            if !process.is_null() {
                SetPriorityClass(process, ABOVE_NORMAL_PRIORITY_CLASS);
                CloseHandle(process);
            }
        }
    }

    /// Per thread: called from the tracker thread, which lives as long as Hangar.
    pub fn keep_awake(on: bool) {
        let flags = if on { ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED } else { ES_CONTINUOUS };
        unsafe { SetThreadExecutionState(flags) };
    }
}

#[cfg(not(windows))]
mod os {
    pub const BEST_PERFORMANCE: u128 = 0;
    pub fn power_mode() -> Option<u128> {
        None
    }
    pub fn set_power_mode(_mode: u128) -> bool {
        false
    }
    pub fn raise_priority(_pid: u32) {}
    pub fn keep_awake(_on: bool) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_default_to_on() {
        let conn = db::open_in_memory().unwrap();
        db::set_setting(&conn, PRIORITY, "false").unwrap();
        let opts = options(&conn).unwrap();
        assert!(opts.power && !opts.priority && opts.awake);
    }

    #[cfg(windows)]
    #[test]
    fn guid_round_trip() {
        let g = windows_sys::core::GUID::from_u128(os::BEST_PERFORMANCE);
        assert_eq!(format!("{:08x}", g.data1), "ded574b5");
        assert_eq!(os::power_mode().is_some(), true);
    }
}
