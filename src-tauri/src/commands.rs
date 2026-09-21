use serde::Serialize;
use tauri::State;

use crate::{db, paths, AppState};

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
pub fn app_info(state: State<'_, AppState>) -> Result<AppInfo, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION"),
        data_dir: state.paths.data.display().to_string(),
        schema_version: db::schema_version(&conn).map_err(|e| e.to_string())?,
        on_system_drive: paths::is_on_system_drive(&state.paths.root),
    })
}
