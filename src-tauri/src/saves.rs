//! Save backups through Ludusavi, a portable tool that knows where 10 000+ games keep their saves.
//! It lives in `<Hangar>\tools\ludusavi`, its settings in `data\ludusavi` (never on C:), and backups
//! go to a folder of the user's choice (`data\saves` by default).
//!
//! Games downloaded "here and there" often save through a Steam emulator (CODEX, RUNE, Goldberg…),
//! in folders named after the game's Steam id: Hangar reads that id in the game folder, uses it to
//! find the game in Ludusavi, and adds the emulator folders to the backup. A save folder chosen by
//! hand is added the same way, for games Ludusavi doesn't know.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::Value;

use crate::db;
use crate::paths::AppPaths;

pub const VERSION: &str = "0.31.0";
const MANIFEST_URL: &str = "https://raw.githubusercontent.com/mtkennerly/ludusavi-manifest/master/data/manifest.yaml";

/// Settings.
pub const DIR: &str = "saves_dir";
pub const AUTO: &str = "saves_auto";
pub const KEEP: &str = "saves_keep";
const DEFAULT_KEEP: u8 = 5;

/// One Ludusavi run at a time: each writes its configuration file first.
static RUNNING: Mutex<()> = Mutex::new(());

pub struct Ludusavi {
    exe: PathBuf,
    config: PathBuf,
}

impl Ludusavi {
    pub fn new(paths: &AppPaths) -> Ludusavi {
        Ludusavi {
            exe: paths.root.join("tools").join("ludusavi").join("ludusavi.exe"),
            config: paths.data.join("ludusavi"),
        }
    }

    pub fn installed(&self) -> bool {
        self.exe.is_file()
    }

    /// Downloads the portable Ludusavi and its game list (slow connections included: long timeouts).
    pub async fn install(&self) -> Result<(), String> {
        let dir = self.exe.parent().ok_or("dossier de Ludusavi introuvable")?;
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        fs::create_dir_all(&self.config).map_err(|e| e.to_string())?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30 * 60))
            .build()
            .map_err(|e| e.to_string())?;
        let download = |url: String| {
            let client = client.clone();
            async move {
                let response = client.get(&url).send().await.and_then(|r| r.error_for_status());
                let bytes = response.map_err(|e| format!("téléchargement impossible ({url}) : {e}"))?.bytes().await;
                bytes.map_err(|e| format!("téléchargement interrompu ({url}) : {e}"))
            }
        };

        let zip = dir.join("ludusavi.zip");
        let url = format!("https://github.com/mtkennerly/ludusavi/releases/download/v{VERSION}/ludusavi-v{VERSION}-win64.zip");
        fs::write(&zip, download(url).await?).map_err(|e| e.to_string())?;
        // tar.exe ships with Windows 10 and later and reads zip archives.
        let status = hidden(Command::new("tar")).arg("-xf").arg(&zip).arg("-C").arg(dir).status();
        let _ = fs::remove_file(&zip);
        if !status.is_ok_and(|s| s.success()) || !self.installed() {
            return Err("impossible de décompresser Ludusavi".into());
        }

        let manifest = download(MANIFEST_URL.to_string()).await?;
        fs::write(self.config.join("manifest.yaml"), manifest).map_err(|e| e.to_string())
    }

    /// Runs Ludusavi with its settings in `data\ludusavi` and reads its JSON answer.
    fn run(&self, args: &[&str]) -> Result<Value, String> {
        if !self.installed() {
            return Err("Ludusavi n'est pas installé (Réglages → Sauvegardes des parties).".into());
        }
        let output = hidden(Command::new(&self.exe))
            .arg("--config")
            .arg(&self.config)
            .args(args)
            .output()
            .map_err(|e| format!("Ludusavi ne démarre pas : {e}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Ludusavi : {}", stderr.trim()));
        }
        serde_json::from_str(&stdout).map_err(|e| format!("réponse de Ludusavi illisible : {e}"))
    }

    /// Ludusavi's name for a game: by Steam id, then exact title, then a close title.
    pub fn find(&self, title: &str, steam_id: Option<u32>) -> Result<Option<String>, String> {
        let first = |v: &Value| v["games"].as_object().and_then(|g| g.keys().next().cloned());
        if let Some(id) = steam_id {
            let id = id.to_string();
            if let Some(name) = first(&self.run(&["find", "--api", "--steam-id", &id])?) {
                return Ok(Some(name));
            }
        }
        for mode in [None, Some("--normalized")] {
            let mut args = vec!["find", "--api"];
            args.extend(mode);
            args.push(title);
            if let Some(name) = first(&self.run(&args)?) {
                return Ok(Some(name));
            }
        }
        let fuzzy = self.run(&["find", "--api", "--fuzzy", title])?;
        Ok(fuzzy["games"].as_object().and_then(|games| {
            games
                .iter()
                .filter_map(|(name, g)| Some((name, g["score"].as_f64()?)))
                .filter(|(_, score)| *score >= 0.85)
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(name, _)| name.clone())
        }))
    }

    /// Writes Ludusavi's settings: backups outside C:, the user's game folders as roots (for saves
    /// kept next to the game), and the extra folders of the game about to be handled.
    fn configure(&self, backups: &Path, roots: &[String], extra: Option<(&str, &[PathBuf])>) -> Result<(), String> {
        let file = self.config.join("config.yaml");
        let mut config: serde_yaml_ng::Value = fs::read_to_string(&file)
            .ok()
            .and_then(|text| serde_yaml_ng::from_str(&text).ok())
            .filter(serde_yaml_ng::Value::is_mapping)
            .unwrap_or_else(|| serde_yaml_ng::Value::Mapping(Default::default()));
        let slash = |p: &Path| p.display().to_string().replace('\\', "/");
        let map = config.as_mapping_mut().expect("mapping");
        for section in ["backup", "restore"] {
            let entry = map.entry(section.into()).or_insert_with(|| serde_yaml_ng::Value::Mapping(Default::default()));
            if let Some(section) = entry.as_mapping_mut() {
                section.insert("path".into(), slash(backups).into());
            }
        }
        // Ludusavi's own roots (Steam, Epic… found at its first start) stay; the user's game folders are added.
        let known = map.entry("roots".into()).or_insert_with(|| serde_yaml_ng::Value::Sequence(Vec::new()));
        if let Some(known) = known.as_sequence_mut() {
            for root in roots {
                let path = root.replace('\\', "/");
                let listed = known.iter().any(|r| r["path"].as_str().is_some_and(|p| p.eq_ignore_ascii_case(&path)));
                if !listed {
                    known.push(serde_yaml_ng::to_value(serde_json::json!({ "store": "other", "path": path })).map_err(|e| e.to_string())?);
                }
            }
        }
        let custom: Vec<Value> = extra
            .filter(|(_, dirs)| !dirs.is_empty())
            .map(|(name, dirs)| {
                serde_json::json!({
                    "name": name,
                    "integration": "extend",
                    "files": dirs.iter().map(|d| slash(d)).collect::<Vec<_>>(),
                    "registry": [],
                })
            })
            .into_iter()
            .collect();
        map.insert("customGames".into(), serde_yaml_ng::to_value(custom).map_err(|e| e.to_string())?);
        fs::create_dir_all(&self.config).map_err(|e| e.to_string())?;
        fs::write(&file, serde_yaml_ng::to_string(&config).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
    }
}

#[cfg(windows)]
fn hidden(mut command: Command) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(not(windows))]
fn hidden(command: Command) -> Command {
    command
}

// ── What a game's backup covers ─────────────────────────────────────────────

/// Everything needed to handle one game, read from the database.
pub struct GameSave {
    pub game_id: i64,
    title: String,
    install_dir: Option<PathBuf>,
    steam_app: Option<u32>,
    name: Option<String>,
    custom_dir: Option<PathBuf>,
    last_backup_at: Option<i64>,
    last_error: Option<String>,
}

pub fn game_save(conn: &Connection, game_id: i64) -> rusqlite::Result<GameSave> {
    conn.query_row(
        "SELECT g.title, g.install_dir, g.source, g.source_id, s.ludusavi_name, s.custom_dir, s.last_backup_at, s.last_error
         FROM games g LEFT JOIN game_saves s ON s.game_id = g.id WHERE g.id = ?1",
        [game_id],
        |r| {
            let source: String = r.get(2)?;
            let source_id: String = r.get(3)?;
            Ok(GameSave {
                game_id,
                title: r.get(0)?,
                install_dir: r.get::<_, Option<String>>(1)?.map(PathBuf::from),
                steam_app: (source == "steam").then(|| source_id.parse().ok()).flatten(),
                name: r.get(4)?,
                custom_dir: r.get::<_, Option<String>>(5)?.map(PathBuf::from),
                last_backup_at: r.get(6)?,
                last_error: r.get(7)?,
            })
        },
    )
}

impl GameSave {
    /// Steam id: the platform's, or the one a Steam emulator reads in the game folder.
    fn steam_id(&self) -> Option<u32> {
        self.steam_app.or_else(|| self.install_dir.as_deref().and_then(emulator_app_id))
    }

    /// Folders added to Ludusavi's own knowledge: the hand-picked one and the emulator's.
    fn extra_dirs(&self) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = self.custom_dir.iter().cloned().collect();
        if let Some(id) = self.install_dir.as_deref().and_then(emulator_app_id) {
            dirs.extend(emulator_save_dirs(id));
        }
        dirs
    }

    /// The name backups are filed under in Ludusavi.
    fn backup_name(&self) -> Option<&str> {
        match self.name.as_deref() {
            Some("") | None if self.extra_dirs().is_empty() => None,
            Some("") | None => Some(&self.title),
            Some(name) => Some(name),
        }
    }
}

/// Looks the game up in Ludusavi once, and remembers the answer ('' when unknown).
fn resolve(tool: &Ludusavi, conn: &Mutex<Connection>, save: &mut GameSave) -> Result<(), String> {
    if save.name.is_some() {
        return Ok(());
    }
    let name = tool.find(&save.title, save.steam_id())?.unwrap_or_default();
    let conn = conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO game_saves (game_id, ludusavi_name) VALUES (?1, ?2)
         ON CONFLICT (game_id) DO UPDATE SET ludusavi_name = excluded.ludusavi_name",
        params![save.game_id, name],
    )
    .map_err(|e| e.to_string())?;
    save.name = Some(name);
    Ok(())
}

// ── Settings ────────────────────────────────────────────────────────────────

pub struct Settings {
    pub dir: PathBuf,
    pub auto: bool,
    pub keep: u8,
}

pub fn settings(conn: &Connection, paths: &AppPaths) -> rusqlite::Result<Settings> {
    Ok(Settings {
        dir: db::get_setting(conn, DIR)?.map(PathBuf::from).unwrap_or_else(|| paths.data.join("saves")),
        auto: db::get_setting(conn, AUTO)?.as_deref() != Some("false"),
        keep: db::get_setting(conn, KEEP)?.and_then(|k| k.parse().ok()).unwrap_or(DEFAULT_KEEP).clamp(1, 50),
    })
}

fn roots(conn: &Connection) -> rusqlite::Result<Vec<String>> {
    crate::library::watched_folders(conn)
}

// ── Operations ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    /// Ludusavi's id for this backup, passed back to restore it.
    pub id: String,
    pub when: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub installed: bool,
    /// Ludusavi's name for the game; `None` when it doesn't know it and no folder was chosen.
    pub name: Option<String>,
    pub custom_dir: Option<String>,
    pub emulator_dirs: Vec<String>,
    /// Save files found right now.
    pub files: Vec<String>,
    pub total_bytes: u64,
    pub backups: Vec<Backup>,
    pub last_backup_at: Option<i64>,
    pub last_error: Option<String>,
}

/// State of a game's saves, for its page. Slow (runs Ludusavi): call off the main thread.
pub fn overview(paths: &AppPaths, conn: &Mutex<Connection>, game_id: i64) -> Result<Overview, String> {
    let tool = Ludusavi::new(paths);
    let (mut save, settings, roots) = {
        let c = conn.lock().map_err(|e| e.to_string())?;
        (game_save(&c, game_id).map_err(|e| e.to_string())?, settings(&c, paths).map_err(|e| e.to_string())?, roots(&c).map_err(|e| e.to_string())?)
    };
    let emulator_dirs = save.install_dir.as_deref().and_then(emulator_app_id).map(emulator_save_dirs).unwrap_or_default();
    let mut overview = Overview {
        installed: tool.installed(),
        name: None,
        custom_dir: save.custom_dir.as_ref().map(|d| d.display().to_string()),
        emulator_dirs: emulator_dirs.iter().map(|d| d.display().to_string()).collect(),
        files: vec![],
        total_bytes: 0,
        backups: vec![],
        last_backup_at: save.last_backup_at,
        last_error: save.last_error.clone(),
    };
    if !overview.installed {
        return Ok(overview);
    }

    let _running = RUNNING.lock().map_err(|e| e.to_string())?;
    resolve(&tool, conn, &mut save)?;
    let Some(name) = save.backup_name().map(str::to_string) else { return Ok(overview) };
    tool.configure(&settings.dir, &roots, Some((&name, &save.extra_dirs())))?;

    // Backups are read from Ludusavi's own index (instant); the files are those of the latest one.
    // Only a game never backed up needs Ludusavi to look for its files (a few seconds).
    let stored = stored_backups(&settings.dir, &name);
    if let Some((backups, files)) = stored.filter(|(b, _)| !b.is_empty()) {
        overview.total_bytes = files.iter().map(|(_, size)| size).sum();
        overview.files = files.into_iter().map(|(file, _)| file).collect();
        overview.backups = backups;
    } else {
        let preview = tool.run(&["backup", "--preview", "--api", &name])?;
        if let Some(files) = preview["games"][&name]["files"].as_object() {
            overview.files = files.keys().cloned().collect();
            overview.total_bytes = files.values().filter_map(|f| f["bytes"].as_u64()).sum();
        }
    }
    overview.name = Some(name);
    Ok(overview)
}

/// A game's backups (newest first) and the files of the latest, from the `mapping.yaml` Ludusavi
/// keeps in each game's backup folder.
fn stored_backups(dir: &Path, name: &str) -> Option<(Vec<Backup>, Vec<(String, u64)>)> {
    let mapping = fs::read_dir(dir).ok()?.flatten().find_map(|entry| {
        let text = fs::read_to_string(entry.path().join("mapping.yaml")).ok()?;
        let mapping: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text).ok()?;
        (mapping["name"].as_str() == Some(name)).then_some(mapping)
    })?;
    let listed = mapping["backups"].as_sequence()?;
    let backups = listed
        .iter()
        .rev()
        .filter_map(|b| Some(Backup { id: b["name"].as_str()?.to_string(), when: b["when"].as_str()?.to_string() }))
        .collect();
    let files = listed
        .last()
        .and_then(|b| b["files"].as_mapping())
        .map(|files| {
            files
                .iter()
                .filter_map(|(file, info)| Some((file.as_str()?.to_string(), info["size"].as_u64().unwrap_or(0))))
                .collect()
        })
        .unwrap_or_default();
    Some((backups, files))
}

/// Backs the game's saves up. `Ok(false)`: nothing to back up (unknown game, no save found).
pub fn backup(paths: &AppPaths, conn: &Mutex<Connection>, game_id: i64) -> Result<bool, String> {
    backup_keeping(paths, conn, game_id, 0)
}

/// `extra`: versions kept beyond the setting, so a backup never pushes out the one about to be restored.
fn backup_keeping(paths: &AppPaths, conn: &Mutex<Connection>, game_id: i64, extra: u8) -> Result<bool, String> {
    let tool = Ludusavi::new(paths);
    let (mut save, settings, roots) = {
        let c = conn.lock().map_err(|e| e.to_string())?;
        (game_save(&c, game_id).map_err(|e| e.to_string())?, settings(&c, paths).map_err(|e| e.to_string())?, roots(&c).map_err(|e| e.to_string())?)
    };
    let _running = RUNNING.lock().map_err(|e| e.to_string())?;
    let result = (|| {
        resolve(&tool, conn, &mut save)?;
        let Some(name) = save.backup_name().map(str::to_string) else { return Ok(false) };
        tool.configure(&settings.dir, &roots, Some((&name, &save.extra_dirs())))?;
        let dir = settings.dir.display().to_string();
        let keep = (settings.keep + extra).to_string();
        let answer = tool.run(&[
            "backup", "--force", "--api", "--path", &dir, "--format", "zip", "--full-limit", &keep,
            "--differential-limit", "0", &name,
        ])?;
        if answer["errors"]["someGamesFailed"].as_bool() == Some(true) {
            return Err("certains fichiers n'ont pas pu être sauvegardés".to_string());
        }
        Ok(answer["games"][&name]["files"].as_object().is_some_and(|f| !f.is_empty()))
    })();

    let c = conn.lock().map_err(|e| e.to_string())?;
    let error = result.as_ref().err().cloned();
    let done = matches!(result, Ok(true));
    c.execute(
        "INSERT INTO game_saves (game_id, last_backup_at, last_error) VALUES (?1, CASE WHEN ?2 THEN ?3 END, ?4)
         ON CONFLICT (game_id) DO UPDATE SET
            last_backup_at = CASE WHEN ?2 THEN ?3 ELSE last_backup_at END, last_error = ?4",
        params![game_id, done, db::now(), error],
    )
    .map_err(|e| e.to_string())?;
    result
}

/// Puts a backup back in place. The current saves are backed up first, so a restore can be undone.
pub fn restore(paths: &AppPaths, conn: &Mutex<Connection>, game_id: i64, backup_id: &str) -> Result<(), String> {
    backup_keeping(paths, conn, game_id, 1)?;
    let tool = Ludusavi::new(paths);
    let (save, settings) = {
        let c = conn.lock().map_err(|e| e.to_string())?;
        (game_save(&c, game_id).map_err(|e| e.to_string())?, settings(&c, paths).map_err(|e| e.to_string())?)
    };
    let name = save.backup_name().ok_or("aucune sauvegarde pour ce jeu")?.to_string();
    let _running = RUNNING.lock().map_err(|e| e.to_string())?;
    let dir = settings.dir.display().to_string();
    let answer = tool.run(&["restore", "--force", "--api", "--path", &dir, "--backup", backup_id, &name])?;
    if answer["errors"]["someGamesFailed"].as_bool() == Some(true) {
        return Err("certains fichiers n'ont pas pu être remis en place".into());
    }
    Ok(())
}

/// Ludusavi's name for the game, when it was looked up and found.
pub fn known_name(conn: &Connection, game_id: i64) -> rusqlite::Result<Option<String>> {
    let name: Option<Option<String>> = conn
        .query_row("SELECT ludusavi_name FROM game_saves WHERE game_id = ?1", [game_id], |r| r.get(0))
        .optional()?;
    Ok(name.flatten().filter(|n| !n.is_empty()))
}

pub fn set_custom_dir(conn: &Connection, game_id: i64, dir: Option<&str>) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO game_saves (game_id, custom_dir) VALUES (?1, ?2)
         ON CONFLICT (game_id) DO UPDATE SET custom_dir = excluded.custom_dir",
        params![game_id, dir],
    )?;
    Ok(())
}

// ── Steam emulators ─────────────────────────────────────────────────────────

/// Steam id written by a Steam emulator in the game folder (`steam_emu.ini`, `steam_appid.txt`).
fn emulator_app_id(install_dir: &Path) -> Option<u32> {
    let mut found = None;
    walk(install_dir, 3, &mut |file| {
        let name = file.file_name().and_then(|n| n.to_str()).map(str::to_ascii_lowercase);
        let id = match name.as_deref() {
            Some("steam_emu.ini") => fs::read_to_string(file).ok().and_then(|text| {
                text.lines().find_map(|line| {
                    let (key, value) = line.split_once('=')?;
                    key.trim().eq_ignore_ascii_case("appid").then(|| value.trim().parse().ok()).flatten()
                })
            }),
            Some("steam_appid.txt") => fs::read_to_string(file).ok().and_then(|t| t.trim().parse().ok()),
            _ => None,
        };
        if id.is_some() {
            found = id;
        }
        found.is_some()
    });
    found
}

/// Walks `dir` up to `depth` levels until `visit` returns true.
fn walk(dir: &Path, depth: u32, visit: &mut dyn FnMut(&Path) -> bool) -> bool {
    let Ok(entries) = fs::read_dir(dir) else { return false };
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
        } else if visit(&path) {
            return true;
        }
    }
    depth > 0 && subdirs.iter().any(|d| walk(d, depth - 1, visit))
}

/// Where the usual Steam emulators keep a game's saves and achievements.
fn emulator_save_dirs(app_id: u32) -> Vec<PathBuf> {
    let env = |key: &str| std::env::var_os(key).map(PathBuf::from);
    let id = app_id.to_string();
    let mut candidates = BTreeSet::new();
    if let Some(appdata) = env("APPDATA") {
        for sub in ["Goldberg SteamEmu Saves", "GSE Saves", r"Steam\CODEX", "EMPRESS", "SmartSteamEmu", "CreamAPI"] {
            candidates.insert(appdata.join(sub).join(&id));
        }
    }
    if let Some(public) = env("PUBLIC") {
        for sub in [r"Documents\Steam\CODEX", r"Documents\Steam\RUNE", r"Documents\OnlineFix"] {
            candidates.insert(public.join(sub).join(&id));
        }
    }
    if let Some(local) = env("LOCALAPPDATA") {
        candidates.insert(local.join("SKIDROW").join(&id));
    }
    if let Some(program_data) = env("PROGRAMDATA") {
        candidates.insert(program_data.join("Steam").join("Player").join(&id));
    }
    candidates.into_iter().filter(|d| d.is_dir()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_emulator_app_id() {
        // Inside target: tests write nothing on C:.
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("hangar-saves-test");
        let nested = dir.join("bin").join("x64");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("steam_emu.ini"), "[Settings]\r\n###\r\nAppId = 2322010\r\n").unwrap();
        assert_eq!(emulator_app_id(&dir), Some(2322010));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_backups_from_ludusavi_mapping() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("hangar-saves-mapping");
        let game = dir.join("God of War Ragnarök");
        fs::create_dir_all(&game).unwrap();
        fs::write(
            game.join("mapping.yaml"),
            "---\nname: God of War Ragnarök\nbackups:\n  - name: backup-1.zip\n    when: \"2026-09-20T10:00:00Z\"\n    files:\n      \"C:/a/OLD\":\n        size: 1\n  - name: backup-2.zip\n    when: \"2026-09-21T10:00:00Z\"\n    files:\n      \"C:/a/SAVE0\":\n        size: 10\n      \"C:/a/SAVE1\":\n        size: 5\n",
        )
        .unwrap();
        let (backups, files) = stored_backups(&dir, "God of War Ragnarök").unwrap();
        assert_eq!(backups.iter().map(|b| b.id.as_str()).collect::<Vec<_>>(), ["backup-2.zip", "backup-1.zip"]);
        assert_eq!(files.iter().map(|(_, s)| s).sum::<u64>(), 15);
        assert!(stored_backups(&dir, "Limbo").is_none());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn backup_name_needs_ludusavi_or_a_folder() {
        let save = GameSave {
            game_id: 1,
            title: "Limbo".into(),
            install_dir: None,
            steam_app: None,
            name: Some(String::new()),
            custom_dir: None,
            last_backup_at: None,
            last_error: None,
        };
        assert_eq!(save.backup_name(), None);
        let save = GameSave { custom_dir: Some(PathBuf::from(r"O:\saves\limbo")), ..save };
        assert_eq!(save.backup_name(), Some("Limbo"));
        let save = GameSave { name: Some("LIMBO".into()), ..save };
        assert_eq!(save.backup_name(), Some("LIMBO"));
    }
}
