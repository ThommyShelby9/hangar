//! Every path Hangar writes to is computed here, from the folder of the executable.
//!
//! Hangar is portable: nothing may land in %APPDATA% / %LOCALAPPDATA% or anywhere on C:.
//! Never use Tauri's `app_data_dir()` / `app_config_dir()` / `app_cache_dir()` — use `AppPaths`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppPaths {
    /// Folder containing the executable.
    pub root: PathBuf,
    /// `<root>\data` — everything Hangar writes lives below this folder.
    pub data: PathBuf,
    pub database: PathBuf,
    pub cache: PathBuf,
    pub webview: PathBuf,
    pub logs: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum PathsError {
    #[error("impossible de localiser l'exécutable : {0}")]
    ExeNotFound(io::Error),
    #[error("le dossier « {path} » n'est pas accessible en écriture : {source}")]
    NotWritable { path: PathBuf, source: io::Error },
}

impl AppPaths {
    /// Paths next to the running executable.
    pub fn from_current_exe() -> Result<Self, PathsError> {
        let exe = std::env::current_exe().map_err(PathsError::ExeNotFound)?;
        let root = exe
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| PathsError::ExeNotFound(io::Error::other("exe sans dossier parent")))?;
        Ok(Self::from_root(root))
    }

    pub fn from_root(root: PathBuf) -> Self {
        let data = root.join("data");
        Self {
            database: data.join("hangar.db"),
            cache: data.join("cache"),
            webview: data.join("webview"),
            logs: data.join("logs"),
            data,
            root,
        }
    }

    /// Every path Hangar may write to.
    pub fn all(&self) -> [&Path; 5] {
        [&self.data, &self.database, &self.cache, &self.webview, &self.logs]
    }

    /// Creates the data folders and checks we can actually write there.
    /// Fails loudly instead of falling back to %APPDATA%.
    pub fn ensure_writable(&self) -> Result<(), PathsError> {
        let not_writable = |path: &Path| {
            let path = path.to_path_buf();
            move |source| PathsError::NotWritable { path, source }
        };
        for dir in [&self.data, &self.cache, &self.webview, &self.logs] {
            fs::create_dir_all(dir).map_err(not_writable(dir))?;
        }
        let probe = self.data.join(".write-test");
        fs::write(&probe, b"ok").map_err(not_writable(&self.data))?;
        fs::remove_file(&probe).map_err(not_writable(&self.data))?;
        Ok(())
    }
}

/// True when `path` is on the C: drive.
pub fn is_on_system_drive(path: &Path) -> bool {
    let s = path.to_string_lossy().replace('/', "\\");
    let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
    s.len() >= 2 && s[..2].eq_ignore_ascii_case("c:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_path_stays_under_root() {
        let paths = AppPaths::from_root(PathBuf::from(r"O:\Apps\Hangar"));
        for p in paths.all() {
            assert!(p.starts_with(&paths.root), "{} sort du dossier de l'exe", p.display());
        }
    }

    #[test]
    fn no_path_on_c_drive() {
        let paths = AppPaths::from_current_exe().expect("exe introuvable");
        for p in paths.all() {
            assert!(!is_on_system_drive(p), "{} est sur C:", p.display());
        }
    }

    #[test]
    fn detects_system_drive() {
        assert!(is_on_system_drive(Path::new(r"C:\Users\hp\AppData")));
        assert!(is_on_system_drive(Path::new(r"c:/x")));
        assert!(is_on_system_drive(Path::new(r"\\?\C:\x")));
        assert!(!is_on_system_drive(Path::new(r"O:\Apps\Hangar")));
    }
}
