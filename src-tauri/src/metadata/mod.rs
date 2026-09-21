//! Game identification and artwork.

pub mod sgdb;

use std::fs;
use std::path::Path;

use crate::library::names;
use sgdb::{ArtKind, Client, SgdbError};

/// Finds the SteamGridDB game for a library entry: by Steam appid when known, otherwise by name.
pub async fn resolve_game(
    client: &Client,
    source: &str,
    source_id: &str,
    title: &str,
) -> Result<Option<i64>, SgdbError> {
    if source == "steam" {
        match client.game_by_steam_id(source_id).await {
            Ok(game) => return Ok(Some(game.id)),
            Err(SgdbError::NotFound) => {}
            Err(e) => return Err(e),
        }
    }
    // Folder names often carry extra words ("Ghost of Tsushima DC"): when no result looks like the
    // title, retry with fewer words. No match at all is better than a wrong cover.
    let clean = names::clean_title(title);
    let words: Vec<&str> = clean.split_whitespace().collect();
    if words.is_empty() {
        return Ok(None);
    }
    for n in (2.min(words.len())..=words.len()).rev() {
        // Never drop a sequel number: "Red Dead Redemption 2" must not become "Red Dead Redemption".
        if n < words.len() && words[n].chars().any(|c| c.is_ascii_digit()) {
            break;
        }
        let term = words[..n].join(" ");
        let results = client.search(&term).await?;
        if let Some(id) = best_match(&term, &results) {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

/// The result closest to `term`. On equal names, a game sold on a PC store wins over a console-only
/// one (God of War 2018 over God of War 2005), then SteamGridDB's own ranking.
fn best_match(term: &str, results: &[sgdb::SgdbGame]) -> Option<i64> {
    let score = |g: &sgdb::SgdbGame| {
        names::similarity(term, &g.name) + if g.types.is_empty() { 0.0 } else { 0.05 }
    };
    results
        .iter()
        .rev() // max_by keeps the last of equal scores: reversed, that is SteamGridDB's top hit
        .map(|g| (score(g), g.id))
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .filter(|(score, _)| *score >= 0.6)
        .map(|(_, id)| id)
}

/// Downloads `url` into `<cache>/games/<game_id>/` and returns the path relative to the cache.
/// Each download gets a new name so the webview never shows a stale cached image.
pub async fn save_image(
    client: &Client,
    cache: &Path,
    game_id: i64,
    kind: ArtKind,
    url: &str,
) -> Result<String, String> {
    let bytes = client.download(url).await.map_err(|e| e.to_string())?;
    let ext = url
        .rsplit('.')
        .next()
        .filter(|e| matches!(e.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp" | "ico"))
        .unwrap_or("png");
    let dir = cache.join("games").join(game_id.to_string());
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    // Drop the previous file of this kind.
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with(&format!("{}-", kind.as_str())) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    let name = format!("{}-{}.{ext}", kind.as_str(), crate::db::now());
    fs::write(dir.join(&name), bytes).map_err(|e| e.to_string())?;
    Ok(format!("games/{game_id}/{name}"))
}

/// Best image of each kind for a SteamGridDB game (SteamGridDB returns them sorted by score).
pub async fn best_images(client: &Client, sgdb_id: i64) -> Vec<(ArtKind, String)> {
    let mut found = Vec::new();
    for kind in ArtKind::ALL {
        if let Ok(images) = client.images(kind, sgdb_id).await {
            if let Some(best) = images.into_iter().next() {
                found.push((kind, best.url));
            }
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live API check: `HANGAR_SGDB_KEY=… cargo test live_sgdb -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn live_sgdb() {
        let client = Client::new(std::env::var("HANGAR_SGDB_KEY").ok()).unwrap();
        tauri::async_runtime::block_on(async {
            for title in [
                "God of War - Ragnarok",
                "God of War",
                "Red Dead Redemption 2",
                "Ghost of Tsushima DC",
                "PES 2017 NextSeasonPatchV3",
                "Limbo",
                "Split Fiction",
            ] {
                match resolve_game(&client, "manual", "", title).await.unwrap() {
                    Some(id) => {
                        let name = client.game(id).await.unwrap().name;
                        let images = best_images(&client, id).await.len();
                        println!("{title:<28} → {name} (#{id}), {images} types d'images");
                    }
                    None => println!("{title:<28} → introuvable"),
                }
            }
        });
    }
}
