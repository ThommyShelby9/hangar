//! Starts a launch action: an executable (optionally elevated) or a URI such as `steam://…`.

use crate::library::LaunchAction;

#[cfg(windows)]
pub fn launch(action: &LaunchAction) -> Result<(), String> {
    use std::path::Path;
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    let is_uri = action.target.contains("://");
    if !is_uri && !Path::new(&action.target).exists() {
        return Err(format!("Fichier introuvable : {}", action.target));
    }

    let wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let verb = wide(if action.run_as_admin && !is_uri { "runas" } else { "open" });
    let file = wide(&action.target);
    let args = wide(&action.args);
    let dir = action.working_dir.as_deref().map(wide);

    // ShellExecute returns a value > 32 on success.
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            if action.args.is_empty() { std::ptr::null() } else { args.as_ptr() },
            dir.as_ref().map_or(std::ptr::null(), |d| d.as_ptr()),
            SW_SHOWNORMAL,
        )
    } as isize;

    match result {
        r if r > 32 => Ok(()),
        5 => Err("Lancement refusé (droits administrateur refusés ?)".into()),
        31 => Err(format!("Aucune application n'ouvre ce lien : {}", action.target)),
        r => Err(format!("Impossible de lancer {} (code {r})", action.target)),
    }
}

#[cfg(not(windows))]
pub fn launch(_action: &LaunchAction) -> Result<(), String> {
    Err("Lancement disponible uniquement sous Windows".into())
}
