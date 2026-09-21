use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::library::{self, folders, names, GamePatch, LaunchAction, NewManualGame};
use crate::metadata::{self, sgdb};
use crate::{controllers, db, gallery, hardware, launcher, paths, perf, saves, scanners, tracker, tray, AppState};

/// Emitted whenever games, images or statuses change; the UI refetches the library.
pub const LIBRARY_CHANGED: &str = "library-changed";
const SGDB_KEY: &str = "sgdb_api_key";

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn with_db<T>(app: &AppHandle, f: impl FnOnce(&mut rusqlite::Connection) -> rusqlite::Result<T>) -> CmdResult<T> {
    let state = app.state::<AppState>();
    let mut conn = state.db.lock().map_err(err)?;
    f(&mut conn).map_err(err)
}

fn changed(app: &AppHandle) {
    let _ = app.emit(LIBRARY_CHANGED, ());
}

// ── App ─────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: &'static str,
    data_dir: String,
    schema_version: u32,
    /// Hangar was copied onto C:, which the portable setup is meant to avoid.
    on_system_drive: bool,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> CmdResult<AppInfo> {
    let conn = state.db.lock().map_err(err)?;
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        data_dir: state.paths.data.display().to_string(),
        schema_version: db::schema_version(&conn).map_err(err)?,
        on_system_drive: paths::is_on_system_drive(&state.paths.root),
    })
}

// ── Library ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_games(app: AppHandle) -> CmdResult<Vec<library::Game>> {
    with_db(&app, |c| library::list_games(c))
}

#[tauri::command]
pub fn get_game(app: AppHandle, id: i64) -> CmdResult<library::GameDetail> {
    with_db(&app, |c| library::get_game(c, id))?.ok_or_else(|| "Jeu introuvable".into())
}

#[tauri::command]
pub fn update_game(app: AppHandle, id: i64, patch: GamePatch) -> CmdResult<()> {
    with_db(&app, |c| library::update_game(c, id, &patch))?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn update_action(app: AppHandle, action: LaunchAction) -> CmdResult<()> {
    with_db(&app, |c| library::update_action(c, &action))?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn remove_game(app: AppHandle, id: i64) -> CmdResult<()> {
    with_db(&app, |c| library::remove_game(c, id))?;
    let _ = std::fs::remove_dir_all(app.state::<AppState>().paths.cache.join("games").join(id.to_string()));
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn launch_game(app: AppHandle, id: i64, action_id: Option<i64>) -> CmdResult<()> {
    let actions = with_db(&app, |c| library::list_actions(c, id))?;
    let action = match action_id {
        Some(aid) => actions.iter().find(|a| a.id == aid),
        None => actions.iter().find(|a| a.is_default).or(actions.first()),
    }
    .ok_or("Ce jeu n'a aucune action de lancement")?;
    launcher::launch(action)?;
    changed(&app);
    Ok(())
}

/// Syncs Steam and Epic, refreshes manual games' status. Returns the number of new games.
#[tauri::command]
pub async fn scan_platforms(app: AppHandle) -> CmdResult<usize> {
    let (steam, epic) = tauri::async_runtime::spawn_blocking(|| (scanners::steam::scan(), scanners::epic::scan()))
        .await
        .map_err(err)?;
    let added = with_db(&app, |c| {
        let mut added = library::sync_platform_games(c, "steam", &steam)?;
        added.extend(library::sync_platform_games(c, "epic", &epic)?);
        library::refresh_manual_status(c)?;
        Ok(added.len())
    })?;
    changed(&app);
    spawn_artwork_fetch(app);
    Ok(added)
}

/// Emitted at start-up with the number of new games waiting in the watched folders.
pub const NEW_GAMES_FOUND: &str = "new-games-found";

/// Start-up check of the watched folders: the library offers the new games, nothing is added silently.
pub async fn announce_new_games(app: AppHandle) {
    let Ok(plan) = with_db(&app, |c| library::scan_plan(c)) else { return };
    let found = tauri::async_runtime::spawn_blocking(move || library::scan_watched_folders(&plan)).await;
    if let Ok(candidates) = found {
        let count = candidates.iter().filter(|c| c.kind == "game").count();
        if count > 0 {
            let _ = app.emit(NEW_GAMES_FOUND, count);
        }
    }
}

/// Game folders found in the watched folders that aren't in the library yet.
#[tauri::command]
pub async fn scan_folders(app: AppHandle) -> CmdResult<Vec<folders::FolderCandidate>> {
    let plan = with_db(&app, |c| library::scan_plan(c))?;
    tauri::async_runtime::spawn_blocking(move || library::scan_watched_folders(&plan)).await.map_err(err)
}

/// Candidates for dropped / picked paths: executables or folders.
#[tauri::command]
pub async fn analyze_paths(paths: Vec<String>) -> CmdResult<Vec<folders::FolderCandidate>> {
    tauri::async_runtime::spawn_blocking(move || {
        paths
            .iter()
            .map(PathBuf::from)
            .filter_map(|p| {
                if p.is_dir() {
                    folders::analyze(&p)
                } else {
                    match p.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() {
                        Some("exe") => Some(candidate_for_exe(&p, String::new())),
                        Some("lnk") => candidate_for_shortcut(&p, library::shortcut::read_lnk(&p)?),
                        Some("url") => candidate_for_shortcut(&p, library::shortcut::read_url_file(&p)?),
                        _ => None,
                    }
                }
            })
            .collect()
    })
    .await
    .map_err(err)
}

/// A dropped exe: the game folder is its parent, above any Unreal-style `Binaries\Win64`.
fn candidate_for_exe(exe: &Path, args: String) -> folders::FolderCandidate {
    let mut dir = exe.parent().unwrap_or(exe).to_path_buf();
    while dir
        .file_name()
        .is_some_and(|n| ["win64", "win32", "x64", "x86", "bin", "binaries"].contains(&n.to_string_lossy().to_lowercase().as_str()))
    {
        dir.pop();
    }
    // Unreal: <Game>\<Project>\Binaries\Win64 — the uninstaller sits in <Game>.
    if let Some(parent) = dir.parent() {
        if parent.parent().is_some() && parent.join("unins000.exe").exists() {
            dir = parent.to_path_buf();
        }
    }
    let product = library::exe_info::product_name(exe);
    let folder_name = dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let title = if folder_name.is_empty() { product.clone().unwrap_or_default() } else { names::clean_title(&folder_name) };
    folders::FolderCandidate {
        dir: dir.display().to_string(),
        title,
        kind: "game",
        exes: vec![folders::ExeCandidate {
            path: exe.display().to_string(),
            size: exe.metadata().map(|m| m.len()).unwrap_or(0),
            product_name: product,
        }],
        extras: vec![],
        args,
    }
}

/// A dropped shortcut: `.lnk` to an exe behaves like the exe itself; `.url` to `steam://…` keeps the link.
fn candidate_for_shortcut(link: &Path, target: library::shortcut::Target) -> Option<folders::FolderCandidate> {
    match target {
        library::shortcut::Target::Exe { path, args } => {
            let is_exe = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe"));
            (is_exe && path.exists()).then(|| candidate_for_exe(&path, args))
        }
        library::shortcut::Target::Uri(uri) => Some(folders::FolderCandidate {
            dir: String::new(),
            title: library::manage::title_from_shortcut(link),
            kind: "game",
            exes: vec![folders::ExeCandidate { path: uri, size: 0, product_name: None }],
            extras: vec![],
            args: String::new(),
        }),
    }
}

#[tauri::command]
pub fn import_games(app: AppHandle, games: Vec<NewManualGame>) -> CmdResult<Vec<i64>> {
    let ids = with_db(&app, |c| {
        games.iter().map(|g| library::add_manual_game(c, g)).collect::<rusqlite::Result<Vec<_>>>()
    })?;
    changed(&app);
    spawn_artwork_fetch(app);
    Ok(ids)
}

/// Folders the user declined: never offered again.
#[tauri::command]
pub fn ignore_folders(app: AppHandle, dirs: Vec<String>) -> CmdResult<()> {
    with_db(&app, |c| dirs.iter().try_for_each(|d| library::ignore_path(c, d)))
}

// ── Managing a game ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn relocate_game(app: AppHandle, id: i64, dir: String) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let mut conn = state.db.lock().map_err(err)?;
    library::manage::relocate(&mut conn, id, &dir)?;
    drop(conn);
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn uninstall_info(app: AppHandle, id: i64) -> CmdResult<library::manage::UninstallInfo> {
    with_db(&app, |c| library::manage::uninstall_info(c, id))
}

/// `uninstaller`: runs the game's own uninstaller (the game stays listed; it turns "missing" once gone).
/// `recycle`: sends the whole folder to the Recycle Bin and removes the game from the library.
#[tauri::command]
pub async fn uninstall_game(app: AppHandle, id: i64, mode: String) -> CmdResult<()> {
    let info = with_db(&app, |c| library::manage::uninstall_info(c, id))?;
    match mode.as_str() {
        "uninstaller" => {
            let exe = info.uninstaller.ok_or("Ce jeu n'a pas de désinstalleur.")?;
            let dir = Path::new(&exe).parent().map(|p| p.display().to_string());
            launcher::launch(&LaunchAction {
                id: 0,
                name: "Désinstaller".into(),
                target: exe,
                args: String::new(),
                working_dir: dir,
                run_as_admin: false,
                is_default: false,
            })
        }
        "recycle" => {
            let dir = PathBuf::from(info.install_dir.ok_or("Ce jeu n'a pas de dossier.")?);
            if dir.parent().is_none() {
                return Err("Refusé : ce dossier est la racine d'un disque.".into());
            }
            tauri::async_runtime::spawn_blocking(move || library::manage::recycle(&dir)).await.map_err(err)??;
            remove_game(app, id)
        }
        _ => Err("Mode de désinstallation inconnu".into()),
    }
}

#[tauri::command]
pub fn set_game_image_file(app: AppHandle, id: i64, kind: sgdb::ArtKind, path: String) -> CmdResult<()> {
    let cache = app.state::<AppState>().paths.cache.clone();
    let file = library::manage::copy_custom_image(&cache, id, kind.as_str(), Path::new(&path))?;
    with_db(&app, |c| library::set_image(c, id, kind.as_str(), &file, "custom"))?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn add_action(app: AppHandle, game_id: i64, name: String, target: String, args: String) -> CmdResult<i64> {
    let id = with_db(&app, |c| library::manage::add_action(c, game_id, name.trim(), &target, &args))?;
    changed(&app);
    Ok(id)
}

#[tauri::command]
pub fn remove_action(app: AppHandle, id: i64) -> CmdResult<()> {
    with_db(&app, |c| library::manage::remove_action(c, id))?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn set_default_action(app: AppHandle, id: i64) -> CmdResult<()> {
    with_db(&app, |c| library::manage::set_default_action(c, id))?;
    changed(&app);
    Ok(())
}

// ── Collections ─────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_collections(app: AppHandle) -> CmdResult<Vec<library::manage::Collection>> {
    with_db(&app, |c| library::manage::list_collections(c))
}

#[tauri::command]
pub fn create_collection(app: AppHandle, name: String) -> CmdResult<i64> {
    let state = app.state::<AppState>();
    let conn = state.db.lock().map_err(err)?;
    library::manage::create_collection(&conn, &name)
}

#[tauri::command]
pub fn delete_collection(app: AppHandle, id: i64) -> CmdResult<()> {
    with_db(&app, |c| library::manage::delete_collection(c, id))?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn set_game_collections(app: AppHandle, id: i64, collections: Vec<i64>) -> CmdResult<()> {
    with_db(&app, |c| library::manage::set_game_collections(c, id, &collections))?;
    changed(&app);
    Ok(())
}

// ── Play time ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn running_games(state: State<'_, AppState>) -> CmdResult<Vec<i64>> {
    Ok(state.running.lock().map_err(err)?.iter().copied().collect())
}

#[tauri::command]
pub fn game_stats(app: AppHandle, id: i64) -> CmdResult<tracker::stats::GameStats> {
    with_db(&app, |c| tracker::stats::game_stats(c, id))
}

/// Play-time overview for the last `days` days, or all time.
#[tauri::command]
pub fn stats_overview(app: AppHandle, days: Option<i64>) -> CmdResult<tracker::stats::Overview> {
    let since = days.map_or(0, |d| db::now() - d * 86_400);
    with_db(&app, |c| tracker::stats::overview(c, since))
}

// ── Hardware ────────────────────────────────────────────────────────────────

fn with_monitor<T>(state: &AppState, f: impl FnOnce(&mut hardware::Monitor) -> T) -> CmdResult<T> {
    let mut monitor = state.monitor.lock().map_err(err)?;
    let monitor = monitor.get_or_insert_with(|| hardware::Monitor::new(&state.paths.data));
    Ok(f(monitor))
}

#[tauri::command]
pub async fn hardware_info(app: AppHandle) -> CmdResult<hardware::HardwareInfo> {
    tauri::async_runtime::spawn_blocking(move || with_monitor(&app.state::<AppState>(), |m| m.info()))
        .await
        .map_err(err)?
}

#[tauri::command]
pub async fn hardware_snapshot(app: AppHandle) -> CmdResult<hardware::Snapshot> {
    tauri::async_runtime::spawn_blocking(move || with_monitor(&app.state::<AppState>(), |m| m.snapshot()))
        .await
        .map_err(err)?
}

/// Starts `hangar-sensors.exe --install` as administrator (one UAC prompt): it creates the logon
/// task that keeps the CPU temperature sensor running.
#[tauri::command]
pub fn enable_cpu_sensors(state: State<'_, AppState>) -> CmdResult<()> {
    let exe = state.paths.root.join("hangar-sensors.exe");
    if !exe.exists() {
        return Err(format!("{} est introuvable : recopie Hangar avec « npm run build:portable ».", exe.display()));
    }
    if !Path::new(hardware::PAWNIO_LIB).exists() {
        return Err("Le pilote PawnIO n'est pas installé.".into());
    }
    launcher::launch(&LaunchAction {
        id: 0,
        name: "Capteurs".into(),
        target: exe.display().to_string(),
        args: "--install".into(),
        working_dir: Some(state.paths.root.display().to_string()),
        run_as_admin: true,
        is_default: false,
    })
}

// ── Controllers ─────────────────────────────────────────────────────────────

fn send(controllers: &controllers::Controllers, command: controllers::Command) -> CmdResult<()> {
    controllers.commands.lock().map_err(err)?.send(command).map_err(err)
}

#[tauri::command]
pub fn controllers_status(controllers: State<'_, controllers::Controllers>) -> CmdResult<controllers::Status> {
    Ok(controllers.status.lock().map_err(err)?.clone())
}

/// Streams live inputs while the controller test page is open.
#[tauri::command]
pub fn controllers_live(controllers: State<'_, controllers::Controllers>, on: bool) -> CmdResult<()> {
    send(&controllers, controllers::Command::Live(on))
}

/// Half a second on one motor: `strong` is the left (low-frequency) one.
#[tauri::command]
pub fn controller_rumble(controllers: State<'_, controllers::Controllers>, id: u32, strong: bool) -> CmdResult<()> {
    let (low, high) = if strong { (0xC000, 0) } else { (0, 0xC000) };
    send(&controllers, controllers::Command::Rumble { id, low, high, ms: 500 })
}

#[tauri::command]
pub fn controller_motion(controllers: State<'_, controllers::Controllers>, id: u32, enabled: bool) -> CmdResult<()> {
    send(&controllers, controllers::Command::Motion { id, enabled })
}

// ── Start-up and notification area ──────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupSettings {
    autostart: bool,
    hide_on_close: bool,
}

#[tauri::command]
pub fn startup_settings(app: AppHandle) -> CmdResult<StartupSettings> {
    let hide = with_db(&app, |c| db::get_setting(c, tray::HIDE_ON_CLOSE))?;
    Ok(StartupSettings { autostart: tray::autostart_enabled(), hide_on_close: hide.as_deref() != Some("false") })
}

#[tauri::command]
pub fn set_autostart(enabled: bool) -> CmdResult<()> {
    tray::set_autostart(enabled)
}

#[tauri::command]
pub fn set_hide_on_close(app: AppHandle, enabled: bool) -> CmdResult<()> {
    with_db(&app, |c| db::set_setting(c, tray::HIDE_ON_CLOSE, if enabled { "true" } else { "false" }))
}

// ── Performance mode ────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfSettings {
    power: bool,
    priority: bool,
    awake: bool,
}

#[tauri::command]
pub fn perf_settings(app: AppHandle) -> CmdResult<PerfSettings> {
    let o = with_db(&app, |c| perf::options(c))?;
    Ok(PerfSettings { power: o.power, priority: o.priority, awake: o.awake })
}

/// `key` is "power", "priority" or "awake"; the tracker applies the change on its next tick.
#[tauri::command]
pub fn set_perf_setting(app: AppHandle, key: String, enabled: bool) -> CmdResult<()> {
    let setting = match key.as_str() {
        "power" => perf::POWER,
        "priority" => perf::PRIORITY,
        "awake" => perf::AWAKE,
        _ => return Err(format!("réglage inconnu : {key}")),
    };
    with_db(&app, |c| db::set_setting(c, setting, if enabled { "true" } else { "false" }))
}

// ── Screenshot gallery ──────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_screenshots(app: AppHandle) -> CmdResult<Vec<gallery::Shot>> {
    let known = with_db(&app, |c| gallery::Known::load(c))?;
    // The capture folders are read without holding the database.
    let shots = tauri::async_runtime::spawn_blocking(move || gallery::list(&known)).await.map_err(err)?;
    app.state::<AppState>().shots.replace(&shots);
    Ok(shots)
}

fn listed_shot(app: &AppHandle, path: &str) -> CmdResult<PathBuf> {
    let path = PathBuf::from(path);
    if app.state::<AppState>().shots.contains(&path) {
        Ok(path)
    } else {
        Err("Capture inconnue : rouvre la galerie.".into())
    }
}

#[tauri::command]
pub fn reveal_screenshot(app: AppHandle, path: String) -> CmdResult<()> {
    let path = listed_shot(&app, &path)?;
    std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", path.display()))
        .spawn()
        .map(|_| ())
        .map_err(err)
}

#[tauri::command]
pub fn delete_screenshot(app: AppHandle, path: String) -> CmdResult<()> {
    let path = listed_shot(&app, &path)?;
    library::manage::recycle(&path)?;
    app.state::<AppState>().shots.remove(&path);
    Ok(())
}

// ── Save backups ────────────────────────────────────────────────────────────

/// Emitted with a `SavesBackedUp` after an automatic backup at the end of a session.
pub const SAVES_BACKED_UP: &str = "saves-backed-up";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SavesBackedUp {
    pub game_id: i64,
    pub title: String,
    /// `None` when it went fine (or there was nothing to back up).
    pub error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavesSettings {
    installed: bool,
    version: &'static str,
    dir: String,
    auto: bool,
    keep: u8,
}

#[tauri::command]
pub fn saves_settings(app: AppHandle) -> CmdResult<SavesSettings> {
    let state = app.state::<AppState>();
    let s = with_db(&app, |c| saves::settings(c, &state.paths))?;
    Ok(SavesSettings {
        installed: saves::Ludusavi::new(&state.paths).installed(),
        version: saves::VERSION,
        dir: s.dir.display().to_string(),
        auto: s.auto,
        keep: s.keep,
    })
}

#[tauri::command]
pub async fn install_ludusavi(app: AppHandle) -> CmdResult<()> {
    let tool = saves::Ludusavi::new(&app.state::<AppState>().paths);
    tool.install().await
}

/// `key`: "dir" (a folder outside C:), "auto" ("true"/"false") or "keep" (backups kept per game).
#[tauri::command]
pub fn set_saves_setting(app: AppHandle, key: String, value: String) -> CmdResult<()> {
    let (setting, value) = match key.as_str() {
        "dir" => {
            if paths::is_on_system_drive(Path::new(&value)) {
                return Err("Choisis un dossier hors du disque C:.".into());
            }
            (saves::DIR, value)
        }
        "auto" => (saves::AUTO, if value == "false" { "false".into() } else { "true".into() }),
        "keep" => (saves::KEEP, value.parse::<u8>().map_err(err)?.clamp(1, 50).to_string()),
        _ => return Err(format!("réglage inconnu : {key}")),
    };
    with_db(&app, |c| db::set_setting(c, setting, &value))
}

/// Runs Ludusavi: slow, so off the async runtime's threads.
async fn with_saves<T: Send + 'static>(
    app: AppHandle,
    f: impl FnOnce(&paths::AppPaths, &std::sync::Mutex<rusqlite::Connection>) -> Result<T, String> + Send + 'static,
) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        f(&state.paths, &state.db)
    })
    .await
    .map_err(err)?
}

#[tauri::command]
pub async fn game_saves(app: AppHandle, id: i64) -> CmdResult<saves::Overview> {
    with_saves(app, move |p, db| saves::overview(p, db, id)).await
}

/// `true` when files were backed up, `false` when the game has none that Ludusavi can find.
#[tauri::command]
pub async fn backup_game_saves(app: AppHandle, id: i64) -> CmdResult<bool> {
    with_saves(app, move |p, db| saves::backup(p, db, id)).await
}

#[tauri::command]
pub async fn restore_game_saves(app: AppHandle, id: i64, backup: String) -> CmdResult<()> {
    with_saves(app, move |p, db| saves::restore(p, db, id, &backup)).await
}

#[tauri::command]
pub fn set_game_save_dir(app: AppHandle, id: i64, dir: Option<String>) -> CmdResult<()> {
    with_db(&app, |c| saves::set_custom_dir(c, id, dir.as_deref()))
}

// ── Watched folders ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn watched_folders(app: AppHandle) -> CmdResult<Vec<String>> {
    with_db(&app, |c| library::watched_folders(c))
}

#[tauri::command]
pub fn add_watched_folder(app: AppHandle, path: String) -> CmdResult<()> {
    with_db(&app, |c| library::add_watched_folder(c, &path))
}

#[tauri::command]
pub fn remove_watched_folder(app: AppHandle, path: String) -> CmdResult<()> {
    with_db(&app, |c| library::remove_watched_folder(c, &path))
}

// ── SteamGridDB ─────────────────────────────────────────────────────────────

fn sgdb_client(app: &AppHandle) -> CmdResult<sgdb::Client> {
    let key = with_db(app, |c| db::get_setting(c, SGDB_KEY))?;
    sgdb::Client::new(key).map_err(err)
}

#[tauri::command]
pub fn sgdb_key_set(app: AppHandle) -> CmdResult<bool> {
    Ok(with_db(&app, |c| db::get_setting(c, SGDB_KEY))?.is_some_and(|k| !k.is_empty()))
}

/// Checks the key against the API before saving it, then fetches missing artwork.
#[tauri::command]
pub async fn set_sgdb_key(app: AppHandle, key: String) -> CmdResult<()> {
    let client = sgdb::Client::new(Some(key.clone())).map_err(err)?;
    client.search("Hades").await.map_err(err)?;
    with_db(&app, |c| db::set_setting(c, SGDB_KEY, key.trim()))?;
    spawn_artwork_fetch(app);
    Ok(())
}

#[tauri::command]
pub async fn sgdb_search(app: AppHandle, term: String) -> CmdResult<Vec<sgdb::SgdbGame>> {
    sgdb_client(&app)?.search(&term).await.map_err(err)
}

#[tauri::command]
pub async fn sgdb_images(app: AppHandle, sgdb_id: i64, kind: sgdb::ArtKind) -> CmdResult<Vec<sgdb::SgdbImage>> {
    sgdb_client(&app)?.images(kind, sgdb_id).await.map_err(err)
}

/// Links a game to another SteamGridDB entry and replaces all its images.
#[tauri::command]
pub async fn set_game_sgdb(app: AppHandle, id: i64, sgdb_id: i64) -> CmdResult<()> {
    let client = sgdb_client(&app)?;
    with_db(&app, |c| library::set_sgdb_id(c, id, sgdb_id))?;
    let cache = app.state::<AppState>().paths.cache.clone();
    for (kind, url) in metadata::best_images(&client, sgdb_id).await {
        let file = metadata::save_image(&client, &cache, id, kind, &url).await?;
        with_db(&app, |c| library::set_image(c, id, kind.as_str(), &file, "steamgriddb"))?;
    }
    changed(&app);
    Ok(())
}

#[tauri::command]
pub async fn set_game_image(app: AppHandle, id: i64, kind: sgdb::ArtKind, url: String) -> CmdResult<()> {
    let client = sgdb_client(&app)?;
    let cache = app.state::<AppState>().paths.cache.clone();
    let file = metadata::save_image(&client, &cache, id, kind, &url).await?;
    with_db(&app, |c| library::set_image(c, id, kind.as_str(), &file, "steamgriddb"))?;
    changed(&app);
    Ok(())
}

/// Fetches artwork for every game without a cover, one at a time, in the background.
pub fn spawn_artwork_fetch(app: AppHandle) {
    let state = app.state::<AppState>();
    if state.artwork_running.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        if let Err(e) = fetch_missing_artwork(&app).await {
            eprintln!("Hangar : images non récupérées : {e}");
        }
        app.state::<AppState>().artwork_running.store(false, Ordering::SeqCst);
    });
}

async fn fetch_missing_artwork(app: &AppHandle) -> CmdResult<()> {
    let Ok(client) = sgdb_client(app) else { return Ok(()) };
    let cache = app.state::<AppState>().paths.cache.clone();
    let ids = with_db(app, |c| library::games_without_artwork(c))?;
    for id in ids {
        let Some(detail) = with_db(app, |c| library::get_game(c, id))? else { continue };
        let game = detail.game;
        let source_id: String = with_db(app, |c| {
            c.query_row("SELECT source_id FROM games WHERE id = ?1", [id], |r| r.get(0))
        })?;
        let sgdb_id = match game.sgdb_id {
            Some(sid) => sid,
            None => match metadata::resolve_game(&client, &game.source, &source_id, &game.title).await {
                Ok(Some(sid)) => sid,
                Ok(None) => continue,
                Err(sgdb::SgdbError::BadKey) => return Err(sgdb::SgdbError::BadKey.to_string()),
                Err(e) => {
                    eprintln!("Hangar : {} : {e}", game.title);
                    continue;
                }
            },
        };
        with_db(app, |c| library::set_sgdb_id(c, id, sgdb_id))?;
        for (kind, url) in metadata::best_images(&client, sgdb_id).await {
            if let Ok(file) = metadata::save_image(&client, &cache, id, kind, &url).await {
                with_db(app, |c| library::set_image(c, id, kind.as_str(), &file, "steamgriddb"))?;
            }
        }
        changed(app);
    }
    Ok(())
}
