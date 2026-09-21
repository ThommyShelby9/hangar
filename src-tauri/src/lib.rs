mod commands;
mod db;
mod paths;

use std::sync::Mutex;

use paths::AppPaths;
use rusqlite::Connection;
use tauri::utils::config::WindowEffectsConfig;
use tauri::window::Effect;
use tauri::{WebviewUrl, WebviewWindowBuilder};

pub struct AppState {
    pub paths: AppPaths,
    pub db: Mutex<Connection>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = match init_state() {
        Ok(state) => state,
        Err(message) => {
            fatal_error(&message);
            std::process::exit(1);
        }
    };
    let webview_dir = state.paths.webview.clone();

    tauri::Builder::default()
        .manage(state)
        .setup(move |app| {
            // Created here rather than in tauri.conf.json so the WebView2 profile
            // lives in data\webview instead of %LOCALAPPDATA%.
            WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("Hangar")
                .inner_size(1280.0, 800.0)
                .min_inner_size(960.0, 600.0)
                .center()
                .decorations(false)
                .transparent(true)
                .effects(WindowEffectsConfig {
                    effects: vec![Effect::Mica],
                    ..Default::default()
                })
                .data_directory(webview_dir)
                .build()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::app_info])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn init_state() -> Result<AppState, String> {
    let paths = AppPaths::from_current_exe().map_err(|e| e.to_string())?;
    paths.ensure_writable().map_err(|e| e.to_string())?;
    let conn = db::open(&paths.database)
        .map_err(|e| format!("impossible d'ouvrir la base {} : {e}", paths.database.display()))?;
    Ok(AppState { paths, db: Mutex::new(conn) })
}

/// Shown before any window exists, so it goes through a native message box.
fn fatal_error(message: &str) {
    eprintln!("Hangar : {message}");
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
        let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
        let text = wide(&format!("{message}\n\nDéplace Hangar dans un dossier où tu peux écrire."));
        let title = wide("Hangar ne peut pas démarrer");
        unsafe {
            MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR);
        }
    }
}
