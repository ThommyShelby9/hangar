//! Epic Games: one JSON `.item` manifest per installed game (read only).

use serde::Deserialize;
use std::fs;
use std::path::Path;

use crate::library::PlatformGame;

const MANIFESTS: &str = r"C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests";

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Manifest {
    display_name: String,
    app_name: String,
    install_location: Option<String>,
    catalog_namespace: Option<String>,
    catalog_item_id: Option<String>,
    #[serde(default)]
    app_categories: Vec<String>,
}

pub fn scan() -> Vec<PlatformGame> {
    let Ok(entries) = fs::read_dir(Path::new(MANIFESTS)) else { return vec![] };
    entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("item")))
        .filter_map(|e| parse_manifest(&fs::read_to_string(e.path()).ok()?))
        .collect()
}

fn parse_manifest(text: &str) -> Option<PlatformGame> {
    let m: Manifest = serde_json::from_str(text).ok()?;
    if !m.app_categories.is_empty() && !m.app_categories.iter().any(|c| c == "games") {
        return None;
    }
    let id = match (&m.catalog_namespace, &m.catalog_item_id) {
        (Some(ns), Some(item)) => format!("{ns}%3A{item}%3A{}", m.app_name),
        _ => m.app_name.clone(),
    };
    Some(PlatformGame {
        source: "epic",
        launch_uri: format!("com.epicgames.launcher://apps/{id}?action=launch&silent=true"),
        source_id: m.app_name,
        title: m.display_name,
        install_dir: m.install_location,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_item_manifest() {
        let game = parse_manifest(
            r#"{"DisplayName":"Hades","AppName":"Min","InstallLocation":"D:\\Epic\\Hades",
                "CatalogNamespace":"ns","CatalogItemId":"item","AppCategories":["public","games"]}"#,
        )
        .unwrap();
        assert_eq!(game.title, "Hades");
        assert_eq!(game.launch_uri, "com.epicgames.launcher://apps/ns%3Aitem%3AMin?action=launch&silent=true");
    }

    #[test]
    fn skips_non_games() {
        assert!(parse_manifest(r#"{"DisplayName":"UE","AppName":"UE_5","AppCategories":["engines"]}"#).is_none());
    }
}
