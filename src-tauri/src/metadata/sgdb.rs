//! SteamGridDB API client (covers, heroes, logos, icons). https://www.steamgriddb.com/api/v2

use serde::{de::DeserializeOwned, Deserialize, Serialize};

const BASE: &str = "https://www.steamgriddb.com/api/v2";

#[derive(Debug, thiserror::Error)]
pub enum SgdbError {
    #[error("aucune clé SteamGridDB : ajoute-la dans les Réglages")]
    NoKey,
    #[error("clé SteamGridDB refusée : vérifie-la dans les Réglages")]
    BadKey,
    #[error("SteamGridDB ne connaît pas ce jeu")]
    NotFound,
    #[error("SteamGridDB injoignable : {0}")]
    Network(#[from] reqwest::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SgdbGame {
    pub id: i64,
    pub name: String,
    #[serde(alias = "release_date")]
    pub release_date: Option<i64>,
    /// Stores the game is sold on ("steam", "egs", "gog", "origin"…); empty for console-only games.
    #[serde(default)]
    pub types: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SgdbImage {
    pub id: i64,
    pub url: String,
    pub thumb: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
}

/// Artwork kinds, as stored in `game_images.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtKind {
    Grid,
    Hero,
    Logo,
    Icon,
}

impl ArtKind {
    pub const ALL: [ArtKind; 4] = [ArtKind::Grid, ArtKind::Hero, ArtKind::Logo, ArtKind::Icon];

    pub fn as_str(self) -> &'static str {
        match self {
            ArtKind::Grid => "grid",
            ArtKind::Hero => "hero",
            ArtKind::Logo => "logo",
            ArtKind::Icon => "icon",
        }
    }

    fn endpoint(self, game_id: i64) -> String {
        match self {
            // Vertical covers, the only shape the library grid uses.
            ArtKind::Grid => format!("grids/game/{game_id}?dimensions=600x900,342x482,660x930&types=static"),
            ArtKind::Hero => format!("heroes/game/{game_id}?types=static"),
            ArtKind::Logo => format!("logos/game/{game_id}?types=static"),
            ArtKind::Icon => format!("icons/game/{game_id}?types=static"),
        }
    }
}

pub struct Client {
    http: reqwest::Client,
    key: String,
}

impl Client {
    pub fn new(key: Option<String>) -> Result<Self, SgdbError> {
        let key = key.filter(|k| !k.trim().is_empty()).ok_or(SgdbError::NoKey)?;
        Ok(Self { http: reqwest::Client::new(), key: key.trim().to_string() })
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, SgdbError> {
        let res = self.http.get(format!("{BASE}/{path}")).bearer_auth(&self.key).send().await?;
        match res.status().as_u16() {
            401 | 403 => Err(SgdbError::BadKey),
            404 => Err(SgdbError::NotFound),
            _ => Ok(res.error_for_status()?.json::<Envelope<T>>().await?.data),
        }
    }

    pub async fn search(&self, term: &str) -> Result<Vec<SgdbGame>, SgdbError> {
        self.get(&format!("search/autocomplete/{}", urlencoding::encode(term))).await
    }

    #[cfg(test)]
    pub async fn game(&self, id: i64) -> Result<SgdbGame, SgdbError> {
        self.get(&format!("games/id/{id}")).await
    }

    pub async fn game_by_steam_id(&self, appid: &str) -> Result<SgdbGame, SgdbError> {
        self.get(&format!("games/steam/{}", urlencoding::encode(appid))).await
    }

    pub async fn images(&self, kind: ArtKind, game_id: i64) -> Result<Vec<SgdbImage>, SgdbError> {
        match self.get(&kind.endpoint(game_id)).await {
            Err(SgdbError::NotFound) => Ok(vec![]),
            other => other,
        }
    }

    pub async fn download(&self, url: &str) -> Result<Vec<u8>, SgdbError> {
        Ok(self.http.get(url).send().await?.error_for_status()?.bytes().await?.to_vec())
    }
}
