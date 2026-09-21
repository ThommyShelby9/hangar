//! Finds games in a folder: picks the real game executable among crash handlers, redistributables,
//! uninstallers and "No DVD" copies, and recognises repack installers.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use super::{exe_info, names};

const MAX_DEPTH: usize = 5;
const MIN_EXE_SIZE: u64 = 100 * 1024;

/// Sub-folders that never hold the game itself.
const SKIP_DIRS: &[&str] = &[
    "redist", "redists", "commonredist", "directx", "dotnet", "vcredist", "nodvd", "crack",
    "battleye", "easyanticheat", "eac", "installer", "__installer", "support", "engine", "prereq",
    "prerequisites", "x360ce", "system volume information", "config.msi", "msdownld.tmp",
    "node_modules",
];

/// Executables that are never the game (matched against the lowercase file stem).
static NOT_A_GAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(unins|uninstall|desinstal|install|setup|crs-|dxsetup|dxwebsetup|vc_?redist|dotnet|ue\d?prereq|quicksfv|x360ce|beservice|eaanticheat|easyanticheat|battleye|7z|unrar|python|redis-)|crash|anticheat|uploader|handler|_be$|patch|cracktro",
    )
    .unwrap()
});

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExeCandidate {
    pub path: String,
    pub size: u64,
    pub product_name: Option<String>,
}

/// A secondary executable offered as an extra launch action ("Launcher", "Configuration"…).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtraAction {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderCandidate {
    pub dir: String,
    pub title: String,
    /// `game`: `exes[0]` is the best guess. `installer`: a repack/setup folder, not an installed game.
    pub kind: &'static str,
    pub exes: Vec<ExeCandidate>,
    pub extras: Vec<ExtraAction>,
}

/// Analyses one game folder. `None` when there is nothing that looks like a game.
pub fn analyze(dir: &Path) -> Option<FolderCandidate> {
    let folder_name = dir.file_name()?.to_string_lossy().into_owned();
    let title = names::clean_title(&folder_name);

    if is_installer_folder(dir, &folder_name) {
        return Some(FolderCandidate {
            dir: dir.display().to_string(),
            title,
            kind: "installer",
            exes: vec![],
            extras: vec![],
        });
    }

    let mut found = Vec::new();
    collect_exes(dir, 0, &mut found);

    let mut games = Vec::new();
    let mut extras: Vec<ExtraAction> = Vec::new();
    for (path, size, depth) in found {
        let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_lowercase();
        if NOT_A_GAME.is_match(&stem) {
            continue;
        }
        if let Some(name) = extra_action_name(&stem) {
            if !extras.iter().any(|e| e.name == name) {
                extras.push(ExtraAction { name: name.into(), path: path.display().to_string() });
            }
            continue;
        }
        if size < MIN_EXE_SIZE {
            continue;
        }
        let product_name = exe_info::product_name(&path);
        if product_name.as_deref().is_some_and(|p| {
            let p = p.to_lowercase();
            p.contains("battleye") || p.contains("anticheat") || p.contains("microsoft® windows®")
        }) {
            continue;
        }
        let score = score(&title, &stem, product_name.as_deref(), size, depth);
        games.push((score, ExeCandidate { path: path.display().to_string(), size, product_name }));
    }
    if games.is_empty() {
        return None;
    }
    games.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.path.len().cmp(&b.1.path.len())));

    Some(FolderCandidate {
        dir: dir.display().to_string(),
        title,
        kind: "game",
        exes: games.into_iter().map(|(_, exe)| exe).collect(),
        extras,
    })
}

/// Every game folder directly inside `root` (e.g. each sub-folder of `H:\Games`).
pub fn scan_root(root: &Path) -> Vec<FolderCandidate> {
    let Ok(entries) = fs::read_dir(root) else { return vec![] };
    entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .filter(|p| !should_skip_dir(p))
        .filter_map(|p| analyze(&p))
        .collect()
}

fn score(title: &str, stem: &str, product_name: Option<&str>, size: u64, depth: usize) -> f32 {
    let name_match = names::similarity(title, stem)
        .max(product_name.map(|p| names::similarity(title, p)).unwrap_or(0.0));
    let size_mb = size as f32 / (1024.0 * 1024.0);
    name_match * 50.0 + (size_mb + 1.0).ln() * 6.0 - depth as f32 * 4.0
}

fn extra_action_name(stem: &str) -> Option<&'static str> {
    if stem.contains("cracktro") {
        None
    } else if stem.contains("launcher") {
        Some("Launcher")
    } else if stem == "settings" || stem.contains("config") {
        Some("Configuration")
    } else {
        None
    }
}

fn is_installer_folder(dir: &Path, folder_name: &str) -> bool {
    let Ok(entries) = fs::read_dir(dir) else { return false };
    let mut has_setup = false;
    let mut has_archives = false;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        has_setup |= name == "setup.exe";
        has_archives |= name.ends_with(".bin") || name.ends_with(".doi");
    }
    has_setup && (has_archives || folder_name.to_lowercase().contains("repack"))
}

fn should_skip_dir(path: &Path) -> bool {
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
    name.starts_with(['_', '$', '.'])
        || SKIP_DIRS.contains(&name.as_str())
        || name.ends_with("venv")
        || path.join("pyvenv.cfg").exists()
}

fn collect_exes(dir: &Path, depth: usize, out: &mut Vec<(PathBuf, u64, usize)>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if depth < MAX_DEPTH && !should_skip_dir(&path) {
                collect_exes(&path, depth + 1, out);
            }
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            out.push((path, size, depth));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_game(files: &[(&str, usize)]) -> PathBuf {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let root = std::env::current_exe().unwrap().parent().unwrap().join(format!(
            "folders-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        for (rel, size) in files {
            let p = root.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, vec![0u8; *size]).unwrap();
        }
        root
    }

    const MB: usize = 1024 * 1024;

    #[test]
    fn picks_game_over_launchers_and_nodvd_copies() {
        let root = fake_game(&[
            ("Red Dead Redemption 2/RDR2.exe", 3 * MB),
            ("Red Dead Redemption 2/Launcher.exe", MB / 2),
            ("Red Dead Redemption 2/unins000.exe", MB),
            ("Red Dead Redemption 2/_Redist/dxwebsetup.exe", MB / 3),
            ("Red Dead Redemption 2/NoDVD/CODEX/RDR2.exe", 3 * MB),
        ]);
        let c = analyze(&root.join("Red Dead Redemption 2")).unwrap();
        assert_eq!(c.kind, "game");
        assert_eq!(c.exes.len(), 1);
        assert!(c.exes[0].path.ends_with(r"Red Dead Redemption 2\RDR2.exe"));
        assert_eq!(c.extras[0].name, "Launcher");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn finds_unreal_binaries_and_skips_patchers() {
        let root = fake_game(&[
            ("Split Fiction/Split/Binaries/Win64/SplitFiction.exe", 2 * MB),
            ("Split Fiction/Split/Binaries/Win64/x360ce.exe", MB),
            ("Split Fiction/unins000.exe", MB),
            ("PES 2017/PES2017.exe", 3 * MB),
            ("PES 2017/NextSeasonPatchV3.exe", MB),
            ("PES 2017/Settings.exe", MB / 2),
        ]);
        let split = analyze(&root.join("Split Fiction")).unwrap();
        assert!(split.exes[0].path.ends_with("SplitFiction.exe"));
        let pes = analyze(&root.join("PES 2017")).unwrap();
        assert!(pes.exes[0].path.ends_with("PES2017.exe"));
        assert_eq!(pes.exes.len(), 1);
        assert_eq!(pes.extras[0].name, "Configuration");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recognises_repack_installers() {
        let root = fake_game(&[
            ("Sekiro [FitGirl Repack]/setup.exe", MB),
            ("Sekiro [FitGirl Repack]/fg-01.bin", 10),
        ]);
        let c = analyze(&root.join("Sekiro [FitGirl Repack]")).unwrap();
        assert_eq!(c.kind, "installer");
        assert_eq!(c.title, "Sekiro");
        fs::remove_dir_all(root).unwrap();
    }

    /// Manual check on this PC's drives: `cargo test real_drives -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_drives() {
        for root in [r"H:\Games", r"N:\"] {
            for c in scan_root(Path::new(root)) {
                println!("{:<9} {:<45} {}", c.kind, c.title, c.exes.first().map_or("", |e| e.path.as_str()));
                for extra in &c.extras {
                    println!("{:<9} {:<45} + {} : {}", "", "", extra.name, extra.path);
                }
            }
        }
    }

    #[test]
    fn empty_folders_are_not_games() {
        let root = fake_game(&[("Films/movie.mkv", 10)]);
        assert!(analyze(&root.join("Films")).is_none());
        fs::remove_dir_all(root).unwrap();
    }
}
