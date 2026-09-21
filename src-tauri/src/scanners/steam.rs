//! Steam: library folders from `libraryfolders.vdf`, games from `appmanifest_*.acf`.

use std::fs;
use std::path::{Path, PathBuf};

use super::vdf;
use crate::library::PlatformGame;

/// Tools and runtimes Steam installs alongside games.
const NOT_GAMES: &[&str] = &["228980", "1070560", "1391110", "1628350", "1493710", "2180100"];

pub fn scan() -> Vec<PlatformGame> {
    let Some(steam) = steam_path() else { return vec![] };
    library_folders(&steam).iter().flat_map(|lib| scan_library(lib)).collect()
}

#[cfg(windows)]
pub fn steam_path() -> Option<PathBuf> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Valve\Steam").ok()?;
    let path: String = key.get_value("SteamPath").ok()?;
    let path = PathBuf::from(path.replace('/', "\\"));
    path.is_dir().then_some(path)
}

#[cfg(not(windows))]
pub fn steam_path() -> Option<PathBuf> {
    None
}

fn library_folders(steam: &Path) -> Vec<PathBuf> {
    let mut folders = vec![steam.to_path_buf()];
    if let Ok(text) = fs::read_to_string(steam.join("steamapps").join("libraryfolders.vdf")) {
        let doc = vdf::parse(&text);
        if let Some(root) = doc.get("libraryfolders") {
            for (_, lib) in root.entries() {
                if let Some(path) = lib.str("path") {
                    let path = PathBuf::from(path);
                    if !folders.iter().any(|f| f.as_os_str().eq_ignore_ascii_case(path.as_os_str())) {
                        folders.push(path);
                    }
                }
            }
        }
    }
    folders
}

fn scan_library(library: &Path) -> Vec<PlatformGame> {
    let steamapps = library.join("steamapps");
    let Ok(entries) = fs::read_dir(&steamapps) else { return vec![] };
    entries
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.starts_with("appmanifest_") && name.ends_with(".acf")
        })
        .filter_map(|e| parse_manifest(&fs::read_to_string(e.path()).ok()?, &steamapps))
        .collect()
}

fn parse_manifest(text: &str, steamapps: &Path) -> Option<PlatformGame> {
    let doc = vdf::parse(text);
    let app = doc.get("AppState")?;
    let appid = app.str("appid")?.to_string();
    if NOT_GAMES.contains(&appid.as_str()) {
        return None;
    }
    let install_dir = app.str("installdir").map(|d| steamapps.join("common").join(d).display().to_string());
    Some(PlatformGame {
        source: "steam",
        title: app.str("name").unwrap_or(&appid).to_string(),
        launch_uri: format!("steam://rungameid/{appid}"),
        source_id: appid,
        install_dir,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_app_manifest() {
        let game = parse_manifest(
            r#""AppState" { "appid" "1245620" "name" "ELDEN RING" "installdir" "ELDEN RING" }"#,
            Path::new(r"H:\SteamLibrary\steamapps"),
        )
        .unwrap();
        assert_eq!(game.source_id, "1245620");
        assert_eq!(game.title, "ELDEN RING");
        assert_eq!(game.launch_uri, "steam://rungameid/1245620");
        assert_eq!(game.install_dir.as_deref(), Some(r"H:\SteamLibrary\steamapps\common\ELDEN RING"));
    }

    #[test]
    fn skips_steamworks_redistributables() {
        let text = r#""AppState" { "appid" "228980" "name" "Steamworks Common Redistributables" }"#;
        assert!(parse_manifest(text, Path::new("x")).is_none());
    }
}
