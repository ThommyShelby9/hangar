//! Windows shortcuts dropped on Hangar: `.lnk` (target exe + arguments) and `.url` (web or
//! launcher links such as `steam://rungameid/…`).

use std::path::{Path, PathBuf};

pub enum Target {
    Exe { path: PathBuf, args: String },
    Uri(String),
}

/// `.url` files are INI files with a `URL=` line.
pub fn read_url_file(path: &Path) -> Option<Target> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .find_map(|l| l.trim().strip_prefix("URL="))
        .map(|url| Target::Uri(url.trim().to_string()))
}

#[cfg(windows)]
pub fn read_lnk(path: &Path) -> Option<Target> {
    use windows::core::{Interface, HSTRING};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM_READ,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

    unsafe {
        // May already be initialised on this thread; either way COM is usable afterwards.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        link.cast::<IPersistFile>().ok()?.Load(&HSTRING::from(path.as_os_str()), STGM_READ).ok()?;

        let mut target = [0u16; 1024];
        link.GetPath(&mut target, std::ptr::null_mut(), 0).ok()?;
        let mut args = [0u16; 1024];
        let _ = link.GetArguments(&mut args);

        let text = |buf: &[u16]| {
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            String::from_utf16_lossy(&buf[..len])
        };
        let target = text(&target);
        if target.is_empty() {
            return None;
        }
        Some(Target::Exe { path: PathBuf::from(target), args: text(&args) })
    }
}

#[cfg(not(windows))]
pub fn read_lnk(_path: &Path) -> Option<Target> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_steam_url_shortcut() {
        let dir = std::env::current_exe().unwrap().parent().unwrap().join("shortcut-test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Hades.url");
        std::fs::write(&file, "[InternetShortcut]\r\nURL=steam://rungameid/1145360\r\nIconIndex=0\r\n").unwrap();
        match read_url_file(&file) {
            Some(Target::Uri(url)) => assert_eq!(url, "steam://rungameid/1145360"),
            _ => panic!("URL non lue"),
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
