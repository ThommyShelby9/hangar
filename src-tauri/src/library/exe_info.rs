//! Reads the "Product name" stored in a Windows executable's version resource.

use std::path::Path;

#[cfg(windows)]
pub fn product_name(exe: &Path) -> Option<String> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW,
    };

    let wide = |s: &std::ffi::OsStr| s.encode_wide().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let path = wide(exe.as_os_str());

    unsafe {
        let size = GetFileVersionInfoSizeW(path.as_ptr(), std::ptr::null_mut());
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        if GetFileVersionInfoW(path.as_ptr(), 0, size, buf.as_mut_ptr() as *mut c_void) == 0 {
            return None;
        }

        // First language/codepage pair, e.g. 040904b0.
        let mut ptr: *mut c_void = std::ptr::null_mut();
        let mut len = 0u32;
        let translation = wide("\\VarFileInfo\\Translation".as_ref());
        if VerQueryValueW(buf.as_ptr() as *const c_void, translation.as_ptr(), &mut ptr, &mut len) == 0
            || len < 4
        {
            return None;
        }
        let lang = *(ptr as *const u16);
        let codepage = *(ptr as *const u16).add(1);

        let key = format!("\\StringFileInfo\\{lang:04x}{codepage:04x}\\ProductName");
        let key = wide(key.as_ref());
        if VerQueryValueW(buf.as_ptr() as *const c_void, key.as_ptr(), &mut ptr, &mut len) == 0 || len == 0 {
            return None;
        }
        let chars = std::slice::from_raw_parts(ptr as *const u16, len as usize);
        // Some executables report a length past the terminator: stop at the first NUL.
        let end = chars.iter().position(|&c| c == 0).unwrap_or(chars.len());
        let name = String::from_utf16_lossy(&chars[..end]).trim().to_string();
        (!name.is_empty()).then_some(name)
    }
}

#[cfg(not(windows))]
pub fn product_name(_exe: &Path) -> Option<String> {
    None
}
