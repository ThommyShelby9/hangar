mod commands;
mod controllers;
mod db;
mod gallery;
mod hardware;
mod launcher;
mod library;
mod metadata;
mod paths;
mod perf;
mod scanners;
mod single_instance;
mod tracker;
mod tray;
mod webview_memory;

use std::path::{Component, Path};
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use paths::AppPaths;
use rusqlite::Connection;
use tauri::http::{header::CONTENT_TYPE, Response, StatusCode};
use tauri::utils::config::WindowEffectsConfig;
use tauri::window::Effect;
use tauri::{Manager, Theme, WebviewUrl, WebviewWindowBuilder};

pub struct AppState {
    pub paths: AppPaths,
    pub db: Mutex<Connection>,
    /// True while the background artwork download runs.
    pub artwork_running: AtomicBool,
    /// Games currently running, maintained by the tracker.
    pub running: Mutex<HashSet<i64>>,
    /// Created on first use: the hardware page may never be opened.
    pub monitor: Mutex<Option<hardware::Monitor>>,
    /// Captures the gallery listed: the only files outside the cache the UI may load.
    pub shots: gallery::Allowed,
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
    // A second launch on the same data folder brings the running Hangar back, then quits.
    let Some(instance) = single_instance::acquire(&state.paths.data) else { return };
    let webview_dir = state.paths.webview.clone();
    let cache_dir = state.paths.cache.clone();

    // Started by Windows at log-on: stay in the notification area.
    let start_hidden = std::env::args().any(|a| a == tray::MINIMIZED_ARG);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        // Cached artwork is served as http://hangar.localhost/<path relative to data\cache>, captures
        // as /~shot?p=<path> and their thumbnails as /~thumb?p=<path>. Off the main thread: a
        // thumbnail takes a moment to build the first time.
        .register_asynchronous_uri_scheme_protocol("hangar", move |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let cache_dir = cache_dir.clone();
            let uri = request.uri().clone();
            tauri::async_runtime::spawn_blocking(move || {
                let response = match uri.path() {
                    "/~shot" | "/~thumb" => serve_shot(&app, &cache_dir, &uri),
                    path => serve_cache_file(&cache_dir, path),
                };
                responder.respond(response);
            });
        })
        .setup(move |app| {
            // Created here rather than in tauri.conf.json so the WebView2 profile
            // lives in data\webview instead of %LOCALAPPDATA%.
            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("Hangar")
                .inner_size(1280.0, 800.0)
                .min_inner_size(960.0, 600.0)
                .center()
                .decorations(false)
                .transparent(true)
                .theme(Some(Theme::Dark))
                .effects(WindowEffectsConfig {
                    effects: vec![Effect::Mica],
                    ..Default::default()
                })
                .data_directory(webview_dir)
                .visible(!start_hidden)
                .build()?;
            webview_memory::install(&window);
            tray::install(app.handle(), &window)?;
            let handle = app.handle().clone();
            instance.on_second_launch(move || tray::show(&handle));
            if start_hidden {
                webview_memory::set_low_memory(&window, true);
            }

            tracker::spawn(app.handle().clone());
            app.manage(controllers::spawn(app.handle().clone()));

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = commands::scan_platforms(handle.clone()).await {
                    eprintln!("Hangar : scan des plateformes impossible : {e}");
                }
                commands::announce_new_games(handle).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::list_games,
            commands::get_game,
            commands::update_game,
            commands::update_action,
            commands::remove_game,
            commands::launch_game,
            commands::scan_platforms,
            commands::scan_folders,
            commands::analyze_paths,
            commands::import_games,
            commands::ignore_folders,
            commands::watched_folders,
            commands::add_watched_folder,
            commands::remove_watched_folder,
            commands::sgdb_key_set,
            commands::set_sgdb_key,
            commands::sgdb_search,
            commands::sgdb_images,
            commands::set_game_sgdb,
            commands::set_game_image,
            commands::relocate_game,
            commands::uninstall_info,
            commands::uninstall_game,
            commands::set_game_image_file,
            commands::add_action,
            commands::remove_action,
            commands::set_default_action,
            commands::list_collections,
            commands::create_collection,
            commands::delete_collection,
            commands::set_game_collections,
            commands::running_games,
            commands::game_stats,
            commands::stats_overview,
            commands::hardware_info,
            commands::hardware_snapshot,
            commands::enable_cpu_sensors,
            commands::controllers_status,
            commands::controllers_live,
            commands::controller_rumble,
            commands::controller_motion,
            commands::startup_settings,
            commands::set_autostart,
            commands::set_hide_on_close,
            commands::perf_settings,
            commands::set_perf_setting,
            commands::list_screenshots,
            commands::reveal_screenshot,
            commands::delete_screenshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn init_state() -> Result<AppState, String> {
    let paths = AppPaths::from_current_exe().map_err(|e| e.to_string())?;
    paths.ensure_writable().map_err(|e| e.to_string())?;
    let conn = db::open(&paths.database)
        .map_err(|e| format!("impossible d'ouvrir la base {} : {e}", paths.database.display()))?;
    library::seed_watched_folders(&conn).map_err(|e| e.to_string())?;
    Ok(AppState {
        paths,
        db: Mutex::new(conn),
        artwork_running: AtomicBool::new(false),
        running: Mutex::new(HashSet::new()),
        monitor: Mutex::new(None),
        shots: gallery::Allowed::default(),
    })
}

fn serve_cache_file(cache: &Path, uri_path: &str) -> Response<Vec<u8>> {
    let relative = urlencoding::decode(uri_path.trim_start_matches('/')).unwrap_or_default().into_owned();
    let relative = Path::new(&relative);
    // Only plain names: no "..", no drive letters, no absolute paths.
    let safe = relative.components().all(|c| matches!(c, Component::Normal(_)));
    match safe.then(|| std::fs::read(cache.join(relative)).ok()).flatten() {
        Some(bytes) => image_response(bytes, relative, "max-age=31536000, immutable"),
        None => not_found(),
    }
}

/// A capture or its thumbnail, only for files the gallery listed.
fn serve_shot(app: &tauri::AppHandle, cache: &Path, uri: &tauri::http::Uri) -> Response<Vec<u8>> {
    let encoded = uri.query().and_then(|q| q.strip_prefix("p=")).unwrap_or_default();
    let path = std::path::PathBuf::from(urlencoding::decode(encoded).unwrap_or_default().into_owned());
    if !app.state::<AppState>().shots.contains(&path) {
        return not_found();
    }
    if uri.path() == "/~thumb" {
        match gallery::thumbnail(&cache.join("thumbs"), &path) {
            Ok(jpeg) => image_response(jpeg, Path::new("thumb.jpg"), "max-age=300"),
            Err(e) => {
                eprintln!("Hangar : miniature de {} impossible : {e}", path.display());
                not_found()
            }
        }
    } else {
        match std::fs::read(&path) {
            Ok(bytes) => image_response(bytes, &path, "max-age=300"),
            Err(_) => not_found(),
        }
    }
}

fn image_response(bytes: Vec<u8>, name: &Path, cache_control: &str) -> Response<Vec<u8>> {
    let mime = match name.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        _ => "image/png",
    };
    Response::builder()
        .header(CONTENT_TYPE, mime)
        .header("Cache-Control", cache_control)
        // Lets the UI read cover pixels (dominant colour) from a canvas.
        .header("Access-Control-Allow-Origin", "*")
        .body(bytes)
        .unwrap()
}

fn not_found() -> Response<Vec<u8>> {
    Response::builder().status(StatusCode::NOT_FOUND).body(Vec::new()).unwrap()
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
