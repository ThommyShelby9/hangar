//! Minimal binding to PawnIOLib.dll (installed with the PawnIO driver) and its IntelMSR module.

use std::ffi::{c_void, CString};
use std::path::Path;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

type OpenFn = unsafe extern "system" fn(*mut HANDLE) -> i32;
type LoadFn = unsafe extern "system" fn(HANDLE, *const u8, usize) -> i32;
type ExecuteFn =
    unsafe extern "system" fn(HANDLE, *const u8, *const u64, usize, *mut u64, usize, *mut usize) -> i32;

pub const PAWNIO_LIB: &str = r"C:\Program Files\PawnIO\PawnIOLib.dll";

pub struct PawnIo {
    handle: HANDLE,
    execute: ExecuteFn,
}

// The handle is only used from the sensor thread.
unsafe impl Send for PawnIo {}

impl PawnIo {
    /// Opens the driver and loads a signed module blob (e.g. IntelMSR.bin).
    pub fn open(module: &Path) -> Result<Self, String> {
        let blob = std::fs::read(module).map_err(|e| format!("module {} illisible : {e}", module.display()))?;
        unsafe {
            let wide: Vec<u16> = PAWNIO_LIB.encode_utf16().chain(std::iter::once(0)).collect();
            let lib: HMODULE = LoadLibraryW(wide.as_ptr());
            if lib.is_null() {
                return Err(format!("PawnIO n'est pas installé ({PAWNIO_LIB} introuvable)"));
            }
            let proc = |name: &str| {
                let name = CString::new(name).unwrap();
                GetProcAddress(lib, name.as_ptr() as *const u8)
            };
            let (Some(open), Some(load), Some(execute)) =
                (proc("pawnio_open"), proc("pawnio_load"), proc("pawnio_execute"))
            else {
                return Err("PawnIOLib.dll incomplète".into());
            };
            let open: OpenFn = std::mem::transmute(open);
            let load: LoadFn = std::mem::transmute(load);
            let execute: ExecuteFn = std::mem::transmute(execute);

            let mut handle: HANDLE = std::ptr::null_mut();
            let hr = open(&mut handle);
            if hr < 0 {
                return Err(format!(
                    "ouverture de PawnIO refusée (0x{hr:08x}) : le capteur doit tourner en administrateur"
                ));
            }
            let hr = load(handle, blob.as_ptr(), blob.len());
            if hr < 0 {
                CloseHandle(handle);
                return Err(format!("chargement du module refusé (0x{hr:08x})"));
            }
            Ok(Self { handle, execute })
        }
    }

    /// Reads a model-specific register on the CPU the calling thread runs on.
    pub fn read_msr(&self, msr: u32) -> Option<u64> {
        let input = [msr as u64];
        let mut output = [0u64; 1];
        let mut written = 0usize;
        let hr = unsafe {
            (self.execute)(
                self.handle,
                c"ioctl_read_msr".as_ptr() as *const u8,
                input.as_ptr(),
                1,
                output.as_mut_ptr(),
                1,
                &mut written,
            )
        };
        (hr >= 0 && written == 1).then_some(output[0])
    }
}

impl Drop for PawnIo {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle as *mut c_void);
        }
    }
}
