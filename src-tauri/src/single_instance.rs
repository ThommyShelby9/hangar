//! One Hangar per data folder: a second launch (shortcut, start-up entry…) asks the running one to
//! show its window and exits, so two play-time trackers never write to the same database.
//! Copies with their own data folder (another portable install) are independent.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;

#[cfg(windows)]
use windows_sys::Win32::Foundation::{GetLastError, HANDLE, ERROR_ALREADY_EXISTS};
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{CreateEventW, CreateMutexW, SetEvent, WaitForSingleObject, INFINITE};

pub struct Instance {
    #[cfg(windows)]
    show_event: HANDLE,
    #[cfg(windows)]
    _mutex: HANDLE,
}

// Kernel handles, used from the waiting thread only.
unsafe impl Send for Instance {}

fn names(data_dir: &Path) -> (Vec<u16>, Vec<u16>) {
    let mut hasher = DefaultHasher::new();
    data_dir.to_string_lossy().to_lowercase().hash(&mut hasher);
    let key = format!("{:016x}", hasher.finish());
    let wide = |s: String| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    (wide(format!("Local\\Hangar-{key}")), wide(format!("Local\\Hangar-show-{key}")))
}

/// `Some` for the first Hangar on this data folder; `None` after waking up the one already running.
#[cfg(windows)]
pub fn acquire(data_dir: &Path) -> Option<Instance> {
    let (mutex_name, event_name) = names(data_dir);
    unsafe {
        let show_event = CreateEventW(std::ptr::null(), 0, 0, event_name.as_ptr());
        let mutex = CreateMutexW(std::ptr::null(), 1, mutex_name.as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            SetEvent(show_event);
            return None;
        }
        Some(Instance { show_event, _mutex: mutex })
    }
}

#[cfg(not(windows))]
pub fn acquire(_data_dir: &Path) -> Option<Instance> {
    Some(Instance {})
}

impl Instance {
    /// Calls `on_show` each time another launch asks this Hangar to come forward.
    pub fn on_second_launch(self, on_show: impl Fn() + Send + 'static) {
        std::thread::Builder::new()
            .name("hangar-single-instance".into())
            .spawn(move || {
                // Moves the whole Instance (Send) rather than just its raw handle field.
                let instance = self;
                #[cfg(windows)]
                loop {
                    if unsafe { WaitForSingleObject(instance.show_event, INFINITE) } != 0 {
                        break;
                    }
                    on_show();
                }
                #[cfg(not(windows))]
                let _ = (&instance, &on_show);
            })
            .expect("impossible de démarrer l'écoute des lancements");
    }
}
