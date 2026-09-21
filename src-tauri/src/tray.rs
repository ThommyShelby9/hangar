//! Hangar in the notification area: it keeps counting play time while its window is hidden, and
//! can start with Windows (hidden).

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, WindowEvent};

use crate::{db, webview_memory, AppState};

/// Launch argument used by the Windows start-up entry: start hidden in the notification area.
pub const MINIMIZED_ARG: &str = "--minimized";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Hangar";
/// Setting: the window's ✕ hides Hangar instead of quitting (default: on).
pub const HIDE_ON_CLOSE: &str = "hide_on_close";

pub fn install(app: &AppHandle, window: &WebviewWindow) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Ouvrir Hangar", true, None::<&str>)?;
    let tv = MenuItem::with_id(app, "tv", "Mode TV", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quitter Hangar", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &tv, &PredefinedMenuItem::separator(app)?, &quit])?;

    let mut tray = TrayIconBuilder::with_id("hangar")
        .tooltip("Hangar")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show(app),
            "tv" => {
                show(app);
                let _ = app.emit("open-tv", ());
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;

    // ✕ hides the window (play time keeps being counted) unless the user turned that off.
    let handle = app.clone();
    let target = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            if hide_on_close(&handle) {
                api.prevent_close();
                hide(&target);
            }
        }
    });
    Ok(())
}

pub fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        webview_memory::set_low_memory(&window, false);
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn hide(window: &WebviewWindow) {
    let _ = window.hide();
    webview_memory::set_low_memory(window, true);
}

fn hide_on_close(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let Ok(conn) = state.db.lock() else { return true };
    db::get_setting(&conn, HIDE_ON_CLOSE).ok().flatten().is_none_or(|v| v != "false")
}

/// Whether Windows starts Hangar at log-on (a value under HKCU\…\Run, nothing written to disk).
#[cfg(windows)]
pub fn autostart_enabled() -> bool {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(RUN_KEY)
        .and_then(|key| key.get_value::<String, _>(RUN_VALUE))
        .is_ok()
}

#[cfg(windows)]
pub fn set_autostart(enabled: bool) -> Result<(), String> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN_KEY).map_err(|e| e.to_string())?;
    if enabled {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        key.set_value(RUN_VALUE, &format!("\"{}\" {MINIMIZED_ARG}", exe.display())).map_err(|e| e.to_string())
    } else {
        match key.delete_value(RUN_VALUE) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}

#[cfg(not(windows))]
pub fn autostart_enabled() -> bool {
    false
}

#[cfg(not(windows))]
pub fn set_autostart(_enabled: bool) -> Result<(), String> {
    Err("Disponible uniquement sous Windows".into())
}
