use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::library::{self, folders, names, GamePatch, LaunchAction, NewManualGame};
use crate::metadata::{self, sgdb};
use crate::{db, launcher, paths, scanners, tracker, AppState};

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
    with_db(&app, |c| library::mark_played(c, id))?;
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

/// Game folders found in the watched folders that aren't in the library yet.
#[tauri::command]
pub async fn scan_folders(app: AppHandle) -> CmdResult<Vec<folders::FolderCandidate>> {
    tauri::async_runtime::spawn_blocking(move || with_db(&app, |c| library::scan_watched_folders(c)))
        .await
        .map_err(err)?
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
                } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
                    Some(candidate_for_exe(&p))
                } else {
                    None
                }
            })
            .collect()
    })
    .await
    .map_err(err)
}

/// A dropped exe: the game folder is its parent, above any Unreal-style `Binaries\Win64`.
fn candidate_for_exe(exe: &Path) -> folders::FolderCandidate {
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
